//! HTTPS server adapter.
//!
//! Sprint 4 desktop upgrade: routes, header names and most wire DTOs now
//! come from `kfilesync_core::protocol` (see ADR-014) instead of a
//! hand-rolled `dto.rs`. Anti-replay (`timestamp`/`nonce`) moved from JSON
//! body fields to HTTP headers (`X-Device-Id` / `X-Timestamp` / `X-Nonce`).
//!
//! Sprint 5 desktop upgrade: the ad-hoc per-handler
//! `extract_anti_replay_headers` + `NonceGuard` checks are replaced by a
//! single `trust_check` helper built on
//! `kfilesync_core::trust::trust_evaluator::evaluate_inbound` (ADR-011) —
//! the central zero-trust gate, covering anti-replay AND paired-device
//! checks in one call. Pairing itself now speaks core's real dual-PIN OOB
//! protocol (`kfilesync_core::trust::pairing_state`, ADR-010) via the new
//! `PAIR_CONFIRM` route, replacing the old single-PIN/manual-cert-PEM flow.
//!
//! Handlers that had no anti-replay check before this sprint
//! (`handle_transfer_cancel`, `handle_chunk_download`, `handle_chunk_upload`)
//! are deliberately left as-is — closing those gaps is out of scope for this
//! sprint's diff (see Sprint 5 report).

use axum::{
    body::Bytes,
    extract::{ConnectInfo, Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use axum_server::tls_rustls::RustlsConfig;
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tower_http::cors::CorsLayer;

use kfilesync_core::domain::SecretPin;
use kfilesync_core::protocol::{conventions, dto as core_dto, headers, routes};
use kfilesync_core::service::nonce_window::NonceWindowState;
use kfilesync_core::trust::pairing_state::{self, PinVerdict};
use kfilesync_core::trust::trust_evaluator::{
    evaluate_inbound, PairedDeviceEntry, PairedDeviceSet, RequestMeta, TrustDecision,
};

use crate::domain::event::identity::{PairingCompleted, PairingRequestReceived};
use crate::domain::model::device::{Certificate, Device, DeviceId, DeviceState, DiscoveredData};
use crate::domain::model::file_entry::{BlockInfo, EntryType, FileEntry};
use crate::domain::model::pairing::PairingSessionStore;
use crate::domain::model::transfer::{
    ChunkInfo, ChunkManifest, FileId, JobId, TransferItem, TransferJob, TransferState, TransferType,
};
use crate::domain::port::event_bus::EventBus;
use crate::domain::port::file_index_repo::FileIndexRepository;
use crate::domain::port::network::NetworkClient;
use crate::domain::port::repository::DeviceRepository;
use crate::domain::port::share_repo::ShareRepository;
use crate::domain::port::transfer_repo::TransferRepository;
use crate::domain::service::policy_enforcer::{PolicyEnforcer, SyncDirection};

pub struct HttpServerConfig {
    pub port: u16,
    pub cert_pem: Vec<u8>,
    pub key_pem: Vec<u8>,
}

#[derive(Clone)]
#[allow(dead_code)]
struct ServerAppState {
    local_device_id: DeviceId,
    local_alias: String,
    /// This device's own certificate, PEM-encoded — sent back to a peer in
    /// `PairResultDto.peer_certificate_pem` when their `/pair/confirm`
    /// succeeds, so they can complete pairing in the same round trip.
    local_cert_pem: String,
    port: u16,
    device_repo: Arc<dyn DeviceRepository>,
    file_index_repo: Arc<dyn FileIndexRepository>,
    share_repo: Arc<dyn ShareRepository>,
    transfer_repo: Arc<dyn TransferRepository>,
    network_client: Arc<dyn NetworkClient>,
    event_bus: Arc<dyn EventBus>,
    policy_enforcer: Arc<PolicyEnforcer>,
    /// Shared with `DeviceAppService` — a pairing session created by either
    /// role (initiator via `start_outbound`, responder via
    /// `receive_incoming`) must be reachable regardless of which side
    /// created it.
    sessions: Arc<PairingSessionStore>,
    nonce_state: Arc<Mutex<NonceWindowState>>,
}

#[allow(clippy::too_many_arguments)]
pub async fn start_server(
    config: HttpServerConfig,
    local_device_id: DeviceId,
    local_alias: String,
    local_cert_pem: String,
    device_repo: Arc<dyn DeviceRepository>,
    file_index_repo: Arc<dyn FileIndexRepository>,
    share_repo: Arc<dyn ShareRepository>,
    transfer_repo: Arc<dyn TransferRepository>,
    network_client: Arc<dyn NetworkClient>,
    event_bus: Arc<dyn EventBus>,
    sessions: Arc<PairingSessionStore>,
    policy_enforcer: Arc<PolicyEnforcer>,
) -> Result<(), String> {
    let state = Arc::new(ServerAppState {
        local_device_id,
        local_alias,
        local_cert_pem,
        port: config.port,
        device_repo,
        file_index_repo,
        share_repo,
        transfer_repo,
        network_client,
        event_bus,
        sessions,
        policy_enforcer,
        nonce_state: Arc::new(Mutex::new(NonceWindowState::default())),
    });

    let app = Router::new()
        .route(routes::INFO, get(handle_info))
        .route(routes::HEALTHZ, get(handle_healthz))
        .route(routes::PAIR_REQUEST, post(handle_pair_request))
        .route(routes::PAIR_CONFIRM, post(handle_pair_confirm))
        .route(routes::PAIR_REVOKE, post(handle_pair_revoke))
        // Deprecated alias, kept for one version cycle (plan doc §Sprint 4).
        .route("/api/lansync/v1/pair/reject", post(handle_pair_revoke))
        .route(routes::SHARE_INVITE, post(handle_share_invite))
        .route(routes::SHARE_LEAVE, post(handle_share_leave))
        // Deprecated alias, kept for one version cycle (plan doc §Sprint 4).
        .route("/api/lansync/v1/share/cancel", post(handle_share_leave))
        .route(routes::SYNC_INDEX, get(handle_sync_index))
        .route(routes::TRANSFER_REQUEST, post(handle_transfer_request))
        .route(routes::TRANSFER_CANCEL, post(handle_transfer_cancel))
        .route(
            "/api/lansync/v1/transfer/{job_id}/chunk/{file_id}/{chunk_index}",
            get(handle_chunk_download).post(handle_chunk_upload),
        )
        .layer(
            CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
                .allow_headers(tower_http::cors::Any),
        )
        .layer(tower_http::limit::RequestBodyLimitLayer::new(
            64 * 1024 * 1024,
        ))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));

    let tls_config = RustlsConfig::from_pem(config.cert_pem, config.key_pem)
        .await
        .map_err(|e| format!("TLS Config Error: {}", e))?;

    println!("Starting HTTPS server on {}", addr);

    tokio::spawn(async move {
        if let Err(e) = axum_server::bind_rustls(addr, tls_config)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
        {
            eprintln!("HTTP server error: {}", e);
        }
    });

    Ok(())
}

// ---- Zero-trust gate (Sprint 5, ADR-011) ----

/// 5-minute acceptance window and 5-minute clock-skew tolerance — matches
/// the previous `NonceGuard::default()`.
const ANTI_REPLAY_WINDOW_MS: i64 = 5 * 60 * 1000;
const ANTI_REPLAY_CLOCK_SKEW_MS: i64 = 5 * 60 * 1000;

const PAIRING_MAX_ATTEMPTS: u32 = 3;
const PAIRING_PIN_TTL_MS: i64 = 5 * 60 * 1000;

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn generate_pin() -> String {
    use rand::Rng;
    let mut rng = rand::rngs::OsRng;
    format!("{:06}", rng.gen_range(0..1_000_000u32))
}

fn build_paired_device_set(devices: Vec<Device>) -> PairedDeviceSet {
    PairedDeviceSet::new(
        devices
            .into_iter()
            .filter_map(|d| match d.state {
                // Desktop's device_id IS the SHA-256 hex of its own cert DER
                // (`keystore::device_id_from_cert_der`), so it doubles as
                // `cert_fingerprint_hex` here (Sprint 4 convention).
                DeviceState::Paired(_) => Some(PairedDeviceEntry {
                    cert_fingerprint_hex: d.id.0.clone(),
                    device_id: d.id.0,
                }),
                _ => None,
            })
            .collect(),
    )
}

/// The central zero-trust gate (ADR-011): anti-replay headers, timestamp
/// window, nonce replay, and (for non-bootstrap routes) paired-device
/// membership, all in one call. Returns the HTTP status core assigned to a
/// rejection, so callers can just `?` this inside a handler returning
/// `Result<_, StatusCode>`.
async fn trust_check(
    state: &ServerAppState,
    route: &str,
    headers: &HeaderMap,
) -> Result<TrustDecision, StatusCode> {
    let get = |name: &str| headers.get(name)?.to_str().ok().map(|s| s.to_string());
    let meta = RequestMeta {
        route: route.to_string(),
        device_id_header: get(headers::HEADER_DEVICE_ID),
        timestamp_ms_header: get(headers::HEADER_TIMESTAMP).and_then(|s| s.parse().ok()),
        nonce_header: get(headers::HEADER_NONCE),
        fingerprint_header: get(headers::HEADER_FINGERPRINT),
    };

    let paired = state
        .device_repo
        .find_paired()
        .await
        .map(build_paired_device_set)
        .unwrap_or_default();

    let mut nonce_state = state.nonce_state.lock().unwrap();
    let decision = evaluate_inbound(
        &meta,
        &paired,
        &mut nonce_state,
        now_millis(),
        ANTI_REPLAY_WINDOW_MS,
        ANTI_REPLAY_CLOCK_SKEW_MS,
    );

    match decision {
        TrustDecision::Reject { http_status, .. } => {
            Err(StatusCode::from_u16(http_status).unwrap_or(StatusCode::UNAUTHORIZED))
        }
        allowed => Ok(allowed),
    }
}

async fn handle_healthz() -> StatusCode {
    StatusCode::OK
}

async fn handle_info(State(state): State<Arc<ServerAppState>>) -> Json<core_dto::DeviceInfoDto> {
    Json(core_dto::DeviceInfoDto {
        protocol: conventions::PROTOCOL_NAME.to_string(),
        version: conventions::PROTOCOL_VERSION.to_string(),
        device_id: state.local_device_id.0.clone(),
        alias: state.local_alias.clone(),
        device_type: "desktop".to_string(),
        platform: std::env::consts::OS.to_string(),
        // Desktop's device_id IS the SHA-256 hex of its cert DER (see
        // `keystore::device_id_from_cert_der`), so it doubles as the
        // fingerprint the wire format expects here.
        fingerprint: state.local_device_id.0.clone(),
        port: state.port,
        // mDNS announcement is always active alongside the HTTPS server in
        // this build; there is no per-device opt-out yet.
        announce: true,
    })
}

// ---- Pairing (Sprint 5: dual-PIN OOB ceremony, ADR-010) ----

async fn handle_pair_request(
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<ServerAppState>>,
    headers: HeaderMap,
    Json(req): Json<core_dto::PairRequestDto>,
) -> Result<StatusCode, StatusCode> {
    trust_check(&state, routes::PAIR_REQUEST, &headers).await?;

    println!(
        "[Server] Received pair request {} from {} (IP: {})",
        req.request_id,
        req.from_device_id,
        peer_addr.ip()
    );

    // Save requesting device as Discovered (if not already known) or update its address.
    let peer_id = DeviceId(req.from_device_id.clone());
    let peer_ip = peer_addr.ip().to_string();
    match state.device_repo.find_by_id(peer_id.clone()).await {
        Ok(None) => {
            let device = Device {
                id: peer_id,
                state: DeviceState::Discovered(DiscoveredData {
                    alias: req.from_alias.clone(),
                    address: peer_ip,
                }),
            };
            let _ = state.device_repo.save(device).await;
        }
        Ok(Some(mut existing)) => {
            if let DeviceState::Discovered(ref mut data) = existing.state {
                if data.address != peer_ip || data.alias != req.from_alias {
                    data.address = peer_ip;
                    data.alias = req.from_alias.clone();
                    let _ = state.device_repo.save(existing).await;
                }
            }
        }
        Err(_) => {}
    }

    let our_pin = generate_pin();
    let session = pairing_state::receive_incoming(
        &req,
        SecretPin::new(our_pin.clone()),
        PAIRING_MAX_ATTEMPTS,
        now_millis(),
        PAIRING_PIN_TTL_MS,
    );
    state.sessions.insert(session);

    // Surfaced to the UI so the local user can be shown `our_pin` (to read
    // aloud/compare) and prompted for the peer's own PIN.
    state.event_bus.publish(Box::new(PairingRequestReceived {
        request_id: req.request_id,
        from_device_id: DeviceId(req.from_device_id),
        from_alias: req.from_alias,
        our_pin,
    }));

    Ok(StatusCode::OK)
}

async fn handle_pair_confirm(
    State(state): State<Arc<ServerAppState>>,
    headers: HeaderMap,
    Json(req): Json<core_dto::PairConfirmDto>,
) -> Result<Json<core_dto::PairResultDto>, StatusCode> {
    trust_check(&state, routes::PAIR_CONFIRM, &headers).await?;

    let reject = |reason: &str| {
        Ok(Json(core_dto::PairResultDto {
            request_id: req.request_id.clone(),
            accepted: false,
            peer_certificate_pem: None,
            reason: Some(reason.to_string()),
        }))
    };

    let now_ms = now_millis();
    let verdict = state
        .sessions
        .with_mut(&req.request_id, |session| {
            pairing_state::verify_peer_pin(session, &req.pin, now_ms)
        });

    let verdict = match verdict {
        Some(v) => v,
        None => return reject("no such pairing session"),
    };

    match verdict {
        PinVerdict::Wrong => reject("wrong pin"),
        PinVerdict::Expired => {
            state.sessions.remove(&req.request_id);
            reject("pairing session expired")
        }
        PinVerdict::MaxAttemptsExceeded => {
            state.sessions.remove(&req.request_id);
            reject("maximum pin attempts exceeded")
        }
        PinVerdict::Accepted => {
            let session = state.sessions.remove(&req.request_id);
            let target_device_id = match session {
                Some(s) => s.target_device_id,
                None => return reject("pairing session vanished mid-confirm"),
            };
            let peer_id = DeviceId(target_device_id.clone());

            let device = match state.device_repo.find_by_id(peer_id.clone()).await {
                Ok(Some(d)) => d,
                _ => return reject("peer device not registered"),
            };

            let cert = match Certificate::from_pem(req.certificate_pem.clone()) {
                Ok(c) => c,
                Err(_) => return reject("invalid certificate"),
            };

            let timestamp = now_secs();
            let updated_state = match device.state.confirm_pairing(cert, timestamp) {
                Ok(s) => s,
                Err(e) => return reject(&format!("{}", e)),
            };

            let updated_device = Device {
                id: device.id.clone(),
                state: updated_state,
            };
            if let Err(e) = state.device_repo.save(updated_device).await {
                return reject(&format!("{}", e));
            }

            state.network_client.trust_fingerprint(&target_device_id);

            state.event_bus.publish(Box::new(PairingCompleted {
                local_device: state.local_device_id.clone(),
                peer_device: peer_id,
                paired_at: timestamp,
            }));

            Ok(Json(core_dto::PairResultDto {
                request_id: req.request_id,
                accepted: true,
                peer_certificate_pem: Some(state.local_cert_pem.clone()),
                reason: None,
            }))
        }
    }
}

#[derive(Deserialize)]
struct PairRevokeBody {
    #[allow(dead_code)]
    device_id: String,
}

async fn handle_pair_revoke(
    State(state): State<Arc<ServerAppState>>,
    headers: HeaderMap,
    Json(_req): Json<PairRevokeBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let decision = trust_check(&state, routes::PAIR_REVOKE, &headers).await?;
    // The revoked device is always the authenticated caller (verified by
    // `trust_check`), never the body's self-reported `device_id` - otherwise
    // any paired device could revoke any other paired device's pairing.
    let peer_id = match decision {
        TrustDecision::Allow { peer_device_id, .. } => DeviceId(peer_device_id),
        _ => return Err(StatusCode::FORBIDDEN),
    };
    println!("[Server] Received pair revoke from {}", peer_id.0);

    if let Ok(Some(device)) = state.device_repo.find_by_id(peer_id.clone()).await {
        if let Ok(revoked_state) = device.state.revoke(
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        ) {
            let updated = Device { id: device.id, state: revoked_state };
            let _ = state.device_repo.save(updated).await;
        }
    }

    state.network_client.untrust_fingerprint(&peer_id.0);

    Ok(Json(serde_json::json!({"status": "ok"})))
}

// ---- Shares ----

/// Handle incoming share invitation from a remote device.
async fn handle_share_invite(
    State(state): State<Arc<ServerAppState>>,
    headers: HeaderMap,
    Json(req): Json<core_dto::ShareInviteDto>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let decision = trust_check(&state, routes::SHARE_INVITE, &headers).await?;
    let sender_id = match decision {
        TrustDecision::Allow { peer_device_id, .. } => DeviceId(peer_device_id),
        _ => return Err(StatusCode::FORBIDDEN),
    };

    println!(
        "[Server] Received share invite: {} from {}",
        req.share_name, sender_id.0
    );

    match state.device_repo.find_by_id(sender_id.clone()).await {
        Ok(Some(device)) => {
            if !matches!(device.state, DeviceState::Paired(_)) {
                return Ok(Json(
                    serde_json::json!({"status": "rejected", "reason": "Sender is not Paired"}),
                ));
            }
        }
        Ok(None) => {
            return Ok(Json(
                serde_json::json!({"status": "rejected", "reason": "Sender not found"}),
            ));
        }
        Err(e) => {
            return Ok(Json(
                serde_json::json!({"status": "error", "reason": format!("{}", e)}),
            ));
        }
    }

    let permission = match req.default_permission.as_str() {
        "read_only" => crate::domain::model::share::SharePermission::ReadOnly,
        "send_only" => crate::domain::model::share::SharePermission::SendOnly,
        "receive_only" => crate::domain::model::share::SharePermission::ReceiveOnly,
        _ => crate::domain::model::share::SharePermission::ReadWrite,
    };

    let sync_mode = match req.sync_mode.as_str() {
        "send_only" => crate::domain::model::share::SyncMode::SendOnly,
        "receive_only" => crate::domain::model::share::SyncMode::ReceiveOnly,
        _ => crate::domain::model::share::SyncMode::TwoWay,
    };

    let share_id = crate::domain::model::share::ShareId(req.share_id.clone());

    match state.share_repo.find_by_id(&share_id).await {
        Ok(Some(existing)) => {
            if existing.has_member(&state.local_device_id) {
                return Ok(Json(
                    serde_json::json!({"status": "accepted", "reason": "Already a member"}),
                ));
            }
            match existing.authorize_member(state.local_device_id.clone(), permission, sender_id) {
                Ok(updated) => {
                    if let Err(e) = state.share_repo.save(&updated).await {
                        return Ok(Json(
                            serde_json::json!({"status": "error", "reason": format!("{}", e)}),
                        ));
                    }
                }
                Err(e) => {
                    return Ok(Json(
                        serde_json::json!({"status": "error", "reason": format!("{}", e)}),
                    ));
                }
            }
        }
        Ok(None) => {
            let safe_share_name = std::path::Path::new(&req.share_name)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .filter(|n| !n.is_empty() && !n.contains(['/', '\\']))
                .unwrap_or_else(|| format!("share-{}", req.share_id));

            let local_path = dirs::download_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(&safe_share_name)
                .to_string_lossy()
                .to_string();

            let share = crate::domain::model::share::Share::create(
                share_id,
                req.share_name.clone(),
                local_path,
                sync_mode,
                sender_id.clone(),
            );

            let share = match share.authorize_member(
                state.local_device_id.clone(),
                permission,
                sender_id,
            ) {
                Ok(updated) => updated,
                Err(e) => {
                    return Ok(Json(
                        serde_json::json!({"status": "error", "reason": format!("{}", e)}),
                    ));
                }
            };

            if let Err(e) = state.share_repo.save(&share).await {
                return Ok(Json(serde_json::json!({"status": "error", "reason": format!("{}", e)})));
            }
        }
        Err(e) => {
            return Ok(Json(serde_json::json!({"status": "error", "reason": format!("{}", e)})));
        }
    }

    // For MVP, auto-acknowledge, Real app would prompt the user.
    Ok(Json(serde_json::json!({"status": "accepted"})))
}

async fn handle_share_leave(
    State(state): State<Arc<ServerAppState>>,
    headers: HeaderMap,
    Json(req): Json<core_dto::ShareLeaveDto>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let decision = trust_check(&state, routes::SHARE_LEAVE, &headers).await?;
    let device_id = match decision {
        TrustDecision::Allow { peer_device_id, .. } => DeviceId(peer_device_id),
        _ => return Err(StatusCode::FORBIDDEN),
    };

    println!(
        "[Server] Received share leave: share={} from device={}",
        req.share_id, device_id.0
    );

    let share_id = crate::domain::model::share::ShareId(req.share_id);

    match state.share_repo.find_by_id(&share_id).await {
        Ok(Some(share)) => match share.remove_member(&device_id) {
            Ok(updated) => {
                if let Err(e) = state.share_repo.save(&updated).await {
                    return Ok(Json(
                        serde_json::json!({"status": "error", "reason": format!("{}", e)}),
                    ));
                }
            }
            Err(e) => {
                return Ok(Json(serde_json::json!({"status": "error", "reason": format!("{}", e)})));
            }
        },
        Ok(None) => {
            return Ok(Json(
                serde_json::json!({"status": "ok", "message": "Share not found, nothing to leave"}),
            ));
        }
        Err(e) => {
            return Ok(Json(serde_json::json!({"status": "error", "reason": format!("{}", e)})));
        }
    }

    Ok(Json(serde_json::json!({"status": "ok"})))
}

// ---- Sync index ----

fn entry_type_to_wire(t: &EntryType) -> String {
    match t {
        EntryType::File => "file".to_string(),
        EntryType::Directory => "directory".to_string(),
        // ADR-008: The wire format has no "symlink" variant.
        EntryType::Symlink => "file".to_string(),
    }
}

fn block_to_dto(b: &BlockInfo) -> core_dto::BlockInfoDto {
    core_dto::BlockInfoDto {
        index: b.index,
        size: b.size,
        hash: b.hash.clone(),
    }
}

fn entry_to_dto(e: &FileEntry) -> core_dto::FileEntryDto {
    core_dto::FileEntryDto {
        share_id: e.share_id.0.clone(),
        path: e.path.clone(),
        entry_type: entry_type_to_wire(&e.entry_type),
        size: e.size,
        modified_at_ms: e.modified_at as i64,
        modified_by: e.modified_by.0.clone(),
        version_vector: core_dto::VersionVectorMap(e.version.iter().map(|(k, v)| (k.to_string(), v)).collect()),
        sha256: e.sha256.clone(),
        blocks: e.blocks.0.iter().map(block_to_dto).collect(),
        deleted: e.deleted,
        deleted_at_ms: e.deleted_at.map(|v| v as i64),
    }
}

#[derive(Deserialize)]
struct SyncIndexQuery {
    share_id: String,
    #[allow(dead_code)]
    since_version: Option<u64>,
}

async fn handle_sync_index(
    State(state): State<Arc<ServerAppState>>,
    headers: HeaderMap,
    Query(query): Query<SyncIndexQuery>,
) -> Result<Json<core_dto::IndexResponseDto>, StatusCode> {
    let decision = trust_check(&state, routes::SYNC_INDEX, &headers).await?;
    let caller_id = match decision {
        TrustDecision::Allow { peer_device_id, .. } => DeviceId(peer_device_id),
        _ => return Err(StatusCode::FORBIDDEN),
    };

    let share_id = crate::domain::model::share::ShareId(query.share_id.clone());

    // Caller must be a member of the requested share
    match state.share_repo.find_by_id(&share_id).await {
        Ok(Some(share)) => {
            if !share.has_member(&caller_id) {
                return Err(StatusCode::FORBIDDEN);
            }
        }
        _ => return Err(StatusCode::NOT_FOUND),
    }

    let entries = state
        .file_index_repo
        .find_all_by_share(&share_id)
        .await
        .unwrap_or_default();

    let index_version = entries.iter().map(|e| e.modified_at).max().unwrap_or(0);

    Ok(Json(core_dto::IndexResponseDto {
        share_id: query.share_id,
        index_version,
        entries: entries.iter().map(entry_to_dto).collect(),
    }))
}

// ---- Transfer ----

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

async fn handle_transfer_request(
    State(state): State<Arc<ServerAppState>>,
    headers: HeaderMap,
    Json(req): Json<core_dto::TransferRequestDto>,
) -> Json<core_dto::TransferAcceptDto> {
    println!(
        "[Server] Transfer request from {} with {} files",
        req.from_device_id,
        req.files.len()
    );

    let reject = |reason: &str| core_dto::TransferAcceptDto {
        session_id: req.session_id.clone(),
        job_id: req.job_id.clone(),
        accepted: false,
        reason: Some(reason.to_string()),
        skip_chunks: Default::default(),
    };

    // The sender's identity is whatever `trust_check` verified against the
    // paired-device set - never the request body's self-reported
    // `from_device_id`, which a paired device could set to impersonate any
    // other paired device.
    let send_id = match trust_check(&state, routes::TRANSFER_REQUEST, &headers).await {
        Ok(TrustDecision::Allow { peer_device_id, .. }) => DeviceId(peer_device_id),
        _ => return Json(reject("anti-replay or trust check failed")),
    };

    // sender must be a Paired device
    match state.device_repo.find_by_id(send_id.clone()).await {
        Ok(Some(device)) if matches!(device.state, DeviceState::Paired(_)) => {}
        _ => {
            eprintln!("[Server] Reject transfer: sender {} not paired", send_id.0);
            return Json(reject("sender not paired"));
        }
    }

    // Sender must be pushing into a share it is actually a member of, with a
    // permission and `SyncMode` that allow inbound writes - without this,
    // any paired device (member or not) could push files into any share.
    let share_id = match &req.share_id {
        Some(id) => crate::domain::model::share::ShareId(id.clone()),
        None => {
            eprintln!("[Server] Reject transfer: no share_id from {}", send_id.0);
            return Json(reject("share_id required"));
        }
    };
    if let Err(e) = state
        .policy_enforcer
        .check_sync(&send_id, &share_id, SyncDirection::Push)
        .await
    {
        eprintln!("[Server] Reject transfer: policy denied for {}: {}", send_id.0, e);
        return Json(reject("permission denied"));
    }

    // resolve receive base dir and refuse paths that escape it
    let base_dir = dirs::download_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let recv_dir = base_dir.join("KFileSync").join(&send_id.0);

    if let Err(e) = std::fs::create_dir_all(&recv_dir) {
        eprintln!("[Server] Fail to create receive dir: {}", e);
        return Json(reject("storage error"));
    }

    let canonical_recv = match recv_dir.canonicalize() {
        Ok(p) => p,
        Err(_) => recv_dir.clone(),
    };

    let items: Vec<TransferItem> = req
        .files
        .iter()
        .map(|item| {
            // Sanitize path; take only the base name and place it under recv_dir.
            // This prevents directory traversal regardless of what the sender claims.
            let safe_name = std::path::Path::new(&item.path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| format!("file-{}", item.file_id));
            let target = canonical_recv.join(&safe_name);
            let target_str = target.to_string_lossy().to_string();

            let mut chunks = Vec::new();
            let cs = item.chunk_size;

            if cs == 0 {
                chunks.push(ChunkInfo {
                    index: 0,
                    offset: 0,
                    size: item.size as u32,
                    hash: item.chunk_hashes.first().cloned().unwrap_or_default(),
                });
            } else {
                let mut offset = 0u64;
                let mut idx = 0u32;
                while offset < item.size {
                    let sz = std::cmp::min(cs as u64, item.size - offset) as u32;
                    chunks.push(ChunkInfo {
                        index: idx,
                        offset,
                        size: sz,
                        // Sprint 4 fix: the sender now sends real per-chunk
                        // BLAKE3 hashes in `chunk_hashes`, so upload-time
                        // verification is no longer a no-op.
                        hash: item.chunk_hashes.get(idx as usize).cloned().unwrap_or_default(),
                    });
                    offset += sz as u64;
                    idx += 1;
                }
            }

            TransferItem {
                file_id: crate::domain::model::transfer::FileId(item.file_id.clone()),
                file_path: target_str,
                file_size: item.size,
                sha256: item.sha256.clone(),
                status: crate::domain::model::transfer::TransferItemStatus::Pending,
                chunk_manifest: ChunkManifest {
                    chunks,
                    chunk_size: cs,
                },
                chunks_done: 0,
                temp_path: None,
            }
        })
        .collect();

    let job = TransferJob {
        job_id: JobId(req.job_id.clone()),
        session_id: req.session_id.clone(),
        job_type: TransferType::Receive,
        peer_device_id: send_id.clone(),
        share_id: req.share_id.clone(),
        state: TransferState::Active {
            started_at: now_secs(),
        },
        items,
        created_at: now_secs(),
    };

    if let Err(e) = state.transfer_repo.save(job).await {
        eprintln!("[Server] Failed to save receive job: {}", e);
        return Json(reject("failed to persist job"));
    }

    Json(core_dto::TransferAcceptDto {
        session_id: req.session_id,
        job_id: req.job_id,
        accepted: true,
        reason: None,
        skip_chunks: Default::default(),
    })
}

#[derive(Deserialize)]
struct TransferCancelBody {
    job_id: String,
}

async fn handle_transfer_cancel(
    State(state): State<Arc<ServerAppState>>,
    Json(req): Json<TransferCancelBody>,
) -> Json<serde_json::Value> {
    if let Ok(Some(job)) = state.transfer_repo.find_by_id(&JobId(req.job_id)).await {
        if let Ok(cancelled) = job.cancel() {
            let _ = state.transfer_repo.save(cancelled).await;
        }
    }
    Json(serde_json::json!({"status": "ok"}))
}

async fn handle_chunk_download(
    State(state): State<Arc<ServerAppState>>,
    AxumPath((job_id, file_id, chunk_index)): AxumPath<(String, String, u32)>,
) -> Result<Bytes, StatusCode> {
    let job = state
        .transfer_repo
        .find_by_id(&JobId(job_id))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let item = job
        .items
        .iter()
        .find(|i| i.file_id.0 == file_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    let chunk = item
        .chunk_manifest
        .chunks
        .iter()
        .find(|c| c.index == chunk_index)
        .ok_or(StatusCode::NOT_FOUND)?;

    let file_path = item.file_path.clone();
    let offset = chunk.offset;
    let size = chunk.size;

    let data = tokio::task::spawn_blocking(move || {
        crate::infrastructure::security::chunk_hasher::ChunkHasher::read_chunk(
            std::path::Path::new(&file_path),
            offset,
            size as u64,
        )
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Bytes::from(data))
}

/// `POST /transfer/{job_id}/chunk/{file_id}/{chunk_index}` — Sprint 4:
/// unified with the download route's path-param shape (was previously
/// `/transfer/{job_id}/chunk?file_id=..&chunk_index=..`). Body is raw
/// bytes; the expected hash travels via `X-Chunk-Hash`, verified
/// constant-time via `kfilesync_core::crypto::chunk_hasher::verify_chunk`.
async fn handle_chunk_upload(
    State(state): State<Arc<ServerAppState>>,
    AxumPath((job_id, file_id, chunk_index)): AxumPath<(String, String, u32)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<core_dto::TransferChunkAckDto>, StatusCode> {
    let job = state
        .transfer_repo
        .find_by_id(&JobId(job_id.clone()))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let item = job
        .items
        .iter()
        .find(|i| i.file_id.0 == file_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    let chunk = item
        .chunk_manifest
        .chunks
        .iter()
        .find(|c| c.index == chunk_index)
        .ok_or(StatusCode::NOT_FOUND)?;

    let header_hash = headers
        .get(headers::HEADER_CHUNK_HASH)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let verified = kfilesync_core::crypto::chunk_hasher::verify_chunk(&body, header_hash)
        && (chunk.hash.is_empty() || chunk.hash == header_hash);

    if !verified {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    let file_path = item.file_path.clone();
    let offset = chunk.offset;
    let data = body.to_vec();

    tokio::task::spawn_blocking(move || {
        use std::io::{Seek, SeekFrom, Write};
        let parent = std::path::Path::new(&file_path).parent();
        if let Some(p) = parent {
            let _ = std::fs::create_dir_all(p);
        }

        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false) // chunk seek-write: preserve existing file content
            .open(&file_path)
            .map_err(|e| e.to_string())?;

        f.seek(SeekFrom::Start(offset)).map_err(|e| e.to_string())?;
        f.write_all(&data).map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let file_id = FileId(file_id);
    let updated = job
        .record_chunk_done(&file_id, chunk_index)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let is_complete = matches!(updated.state, TransferState::Verifying);
    let updated = if is_complete {
        updated
            .complete()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        updated
    };

    state
        .transfer_repo
        .save(updated)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(core_dto::TransferChunkAckDto {
        job_id,
        file_id: file_id.0,
        chunk_index,
        verified: true,
    }))
}