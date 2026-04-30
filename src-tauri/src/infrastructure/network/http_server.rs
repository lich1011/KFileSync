use axum::{
    routing::{get, post},
    Router, Json, extract::{Query, State},
};
use serde::Deserialize;
use std::sync::Arc;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use axum_server::tls_rustls::RustlsConfig;

use crate::domain::model::device::{Device, DeviceId, DeviceState, DiscoveredData};
use crate::domain::port::repository::DeviceRepository;
use crate::domain::port::file_index_repo::FileIndexRepository;
use crate::domain::port::share_repo::ShareRepository;
use super::dto::{PairRequestDto, PairResponseDto, ShareInviteDto, SyncIndexResponseDto};

pub struct HttpServerConfig {
    pub port: u16,
    pub cert_pem: Vec<u8>,
    pub key_pem: Vec<u8>,
}

#[derive(Clone)]
#[allow(dead_code)]
struct ServerAppState {
    local_device_id: DeviceId,
    device_repo: Arc<dyn DeviceRepository>,
    file_index_repo: Arc<dyn FileIndexRepository>,
    share_repo: Arc<dyn ShareRepository>,
}

pub async fn start_server(
    config: HttpServerConfig,
    local_device_id: DeviceId,
    device_repo: Arc<dyn DeviceRepository>,
    file_index_repo: Arc<dyn FileIndexRepository>,
    share_repo: Arc<dyn ShareRepository>,
) -> Result<(), String> {
    let state = Arc::new(ServerAppState {
        local_device_id,
        device_repo,
        file_index_repo,
        share_repo,
    });

    let app = Router::new()
        .route("/api/lansync/v1/pair/request", post(handle_pair_request))
        .route("/api/lansync/v1/share/invite", post(handle_share_invite))
        .route("/api/lansync/v1/sync/index", get(handle_sync_index))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));

    let tls_config = RustlsConfig::from_pem(config.cert_pem, config.key_pem)
        .await
        .map_err(|e| format!("TLS Config Error: {}", e))?;

    println!("Starting HTTPS server on {}", addr);

    tokio::spawn(async move {
        if let Err(e) = axum_server::bind_rustls(addr, tls_config)
            .serve(app.into_make_service())
            .await
        {
            eprintln!("HTTP server error: {}", e);
        }
    });

    Ok(())
}

/// Handle incoming pair request from a remote device.
/// Saves the requesting device as Discovered and responds with local device info.
async fn handle_pair_request(
    State(state): State<Arc<ServerAppState>>,
    Json(req): Json<PairRequestDto>,
) -> Json<PairResponseDto> {
    println!("[Server] Received pair request from {}", req.device_id);

    // Save requesting device as Discovered (if not already known)
    let peer_id = DeviceId(req.device_id.clone());
    if let Ok(None) = state.device_repo.find_by_id(peer_id.clone()).await {
        let device = Device {
            id: peer_id,
            state: DeviceState::Discovered(DiscoveredData {
                alias: req.alias.clone(),
                address: String::new(), // We don't know caller's IP in this handler yet
            }),
        };
        let _ = state.device_repo.save(device).await;
    }

    Json(PairResponseDto {
        status: "accepted".to_string(),
        device_id: state.local_device_id.0.clone(),
        alias: "LanSync Device".to_string(),
        platform: std::env::consts::OS.to_string(),
        fingerprint_short: "0000-0000".to_string(), // TODO: generate from real cert
    })
}

/// Handle incoming share invitation from a remote device.
async fn handle_share_invite(
    State(_state): State<Arc<ServerAppState>>,
    Json(req): Json<ShareInviteDto>,
) -> Json<serde_json::Value> {
    println!("[Server] Received share invite: {} from {}", req.share_name, req.invited_by);
    // For MVP, auto-acknowledge. Real app would prompt the user.
    Json(serde_json::json!({"status": "accepted"}))
}

/// Serve local file index for a given share_id.
#[derive(Deserialize)]
struct SyncIndexQuery {
    share_id: String,
    #[allow(dead_code)]
    since_version: Option<u64>,
}

async fn handle_sync_index(
    State(state): State<Arc<ServerAppState>>,
    Query(query): Query<SyncIndexQuery>,
) -> Json<SyncIndexResponseDto> {
    let share_id = crate::domain::model::share::ShareId(query.share_id.clone());

    let entries = state.file_index_repo.find_all_by_share(&share_id).await.unwrap_or_default();

    Json(SyncIndexResponseDto {
        share_id: query.share_id,
        index_version: entries.len() as u64,
        entries,
    })
}
