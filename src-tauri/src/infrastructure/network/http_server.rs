use axum::{
    routing::{get, post},
    Router, Json, extract::{Query, State, Path as AxumPath, ConnectInfo},
    body::Bytes,
    http::StatusCode,   
};
use serde::Deserialize;
use std::sync::Arc;
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};
use tower_http::cors::CorsLayer;
use axum_server::tls_rustls::RustlsConfig;

use crate::domain::model::device::{Device, DeviceId, DeviceState, DiscoveredData};
use crate::domain::model::transfer::{
    TransferJob, TransferType, TransferState, TransferItem,
    ChunkManifest, ChunkInfo, FileId, JobId
};  
use crate::domain::port::repository::DeviceRepository;
use crate::domain::port::file_index_repo::FileIndexRepository;
use crate::domain::port::share_repo::ShareRepository;
use crate::domain::port::transfer_repo::TransferRepository;
use crate::infrastructure::security::chunk_hasher::ChunkHasher;
use crate::infrastructure::security::keystore::fingerprint_short;
use crate::infrastructure::security::nonce_validator::NonceValidator;
use super::dto::{PairRequestDto, PairResponseDto, ShareInviteDto, SyncIndexResponseDto,
    TransferRequestDto, TransferResponseDto, SkipChunkDto, ShareCancelDto};

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
    device_repo: Arc<dyn DeviceRepository>,
    file_index_repo: Arc<dyn FileIndexRepository>,
    share_repo: Arc<dyn ShareRepository>,
    transfer_repo: Arc<dyn TransferRepository>,
    nonce_validator: Arc<NonceValidator>,
}

pub async fn start_server(
    config: HttpServerConfig,
    local_device_id: DeviceId,
    local_alias: String,
    device_repo: Arc<dyn DeviceRepository>,
    file_index_repo: Arc<dyn FileIndexRepository>,
    share_repo: Arc<dyn ShareRepository>,
    transfer_repo: Arc<dyn TransferRepository>,
) -> Result<(), String> {
    let state = Arc::new(ServerAppState {
        local_device_id,
        local_alias,
        device_repo,
        file_index_repo,
        share_repo,
        transfer_repo,
        nonce_validator: std::sync::Arc::new(NonceValidator::new(300)),
    });

    let app = Router::new()
        .route("/api/lansync/v1/pair/request", post(handle_pair_request))
        .route("/api/lansync/v1/share/invite", post(handle_share_invite))
        .route("/api/lansync/v1/share/cancel", post(handle_share_cancel))
        .route("/api/lansync/v1/sync/index", get(handle_sync_index))
        .route("/api/lansync/v1/transfer/request", post(handle_transfer_request))
        .route("/api/lansync/v1/transfer/{job_id}/chunk/{file_id}/{chunk_index}", get(handle_chunk_download))
        .route("/api/lansync/v1/transfer/{job_id}/chunk", post(handle_chunk_upload))
        .layer(CorsLayer::permissive())
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

/// Handle incoming pair request from a remote device.
/// Saves the requesting device as Discovered and responds with local device info.
async fn handle_pair_request(
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<ServerAppState>>,
    Json(req): Json<PairRequestDto>,
) -> Json<PairResponseDto> {
    println!("[Server] Received pair request from {} (IP: {})", req.device_id, peer_addr.ip());

    if let Err(e) = state.nonce_validator.validate(&req.nonce, req.timestamp) {
        eprintln!("[Server] Invalid pair request: {}", e);
    }

    // Save requesting device as Discovered (if not already known)
    let peer_id = DeviceId(req.device_id.clone());
    if let Ok(None) = state.device_repo.find_by_id(peer_id.clone()).await {
        let device = Device {
            id: peer_id,
            state: DeviceState::Discovered(DiscoveredData {
                alias: req.alias.clone(),
                address: peer_addr.ip().to_string(),
            }),
        };
        let _ = state.device_repo.save(device).await;
    }

    Json(PairResponseDto {
        status: "accepted".to_string(),
        device_id: state.local_device_id.0.clone(),
        alias: state.local_alias.clone(),
        platform: std::env::consts::OS.to_string(),
        fingerprint_short: fingerprint_short(&state.local_device_id.0), 
    })
}

/// Handle incoming share invitation from a remote device.
async fn handle_share_invite(
    State(state): State<Arc<ServerAppState>>,
    Json(req): Json<ShareInviteDto>,
) -> Json<serde_json::Value> {
    println!("[Server] Received share invite: {} from {}", req.share_name, req.invited_by);

    if let Err(e) = state.nonce_validator.validate(&req.nonce, req.timestamp) {
        return Json(serde_json::json!({"status": "rejected", "reason": format!("{}", e)}));
    }

    let sender_id = DeviceId(req.invited_by.clone());
    match state.device_repo.find_by_id(sender_id.clone()).await {
        Ok(Some(device)) => {
            if !matches!(
                device.state, 
                DeviceState::Paired(_),
            ) {
                return Json(serde_json::json!({"status": "rejected", "reason": "Sender is not Paired"}));
            }   
        },
        Ok(None)=> {
            return Json(serde_json::json!({"status": "rejected", "reason": "Sender not found"}));
        }
        Err(e) => {
            return Json(serde_json::json!({"status": "error", "reason": format!("{}", e)}));
        }
    }
    
    let permission = match req.permission.as_str() {
        "read_only" => crate::domain::model::share::SharePermission::ReadOnly,
        "send_only" => crate::domain::model::share::SharePermission::SendOnly,
        "receive_only" => crate::domain::model::share::SharePermission::ReceiveOnly,
        _ => crate::domain::model::share::SharePermission::ReadWrite,
    };

    let share_id = crate::domain::model::share::ShareId(req.share_id.clone());
    
    match state.share_repo.find_by_id(&share_id).await {
        Ok(Some(existing)) => {
            if existing.has_member(&state.local_device_id) {
                return Json(serde_json::json!({"status": "accepted", "reason": "Already a member"}));
            }
            match existing.authorize_member(state.local_device_id.clone(), permission, sender_id) {
                Ok(updated) => {
                    if let Err(e) = state.share_repo.save(&updated).await {
                        return Json(serde_json::json!({"status": "error", "reason": format!("{}", e)}));
                    }
                }
                Err(e) => {
                    return Json(serde_json::json!({"status": "error", "reason": format!("{}", e)}));
                }
            }   
        },
        Ok(None)=> {
            let local_path = dirs::download_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(&req.share_name)
                .to_string_lossy()
                .to_string();

            let share = crate::domain::model::share::Share::create(
                share_id, req.share_name.clone(), 
                local_path, crate::domain::model::share::SyncMode::TwoWay, sender_id.clone());

            let share = match share.authorize_member(state.local_device_id.clone(), permission, sender_id) {
                Ok(updated) => updated,
                Err(e) => {
                    return Json(serde_json::json!({"status": "error", "reason": format!("{}", e)}));
                }
            };
            
            if let Err(e) = state.share_repo.save(&share).await {
                return Json(serde_json::json!({"status": "error", "reason": format!("{}", e)}));
            }

        }
        Err(e) => {
            return Json(serde_json::json!({"status": "error", "reason": format!("{}", e)}));
        }
    }
    
    // For MVP, auto-acknowledge. Real app would prompt the user.
    Json(serde_json::json!({"status": "accepted"}))
}

async fn handle_share_cancel(
    State(state): State<Arc<ServerAppState>>,
    Json(req): Json<ShareCancelDto>,
) -> Json<serde_json::Value> {
    println!("[Server] Received share cancel: share={} from device={}", req.share_id, req.device_id);

    if let Err(e) = state.nonce_validator.validate(&req.nonce, req.timestamp) {
        return Json(serde_json::json!({"status": "rejected", "reason": format!("{}", e)}));
    }

    let share_id = crate::domain::model::share::ShareId(req.share_id);
    let device_id = DeviceId(req.device_id);

    match state.share_repo.find_by_id(&share_id).await {
        Ok(Some(share)) => {
            match share.remove_member(&device_id) {
                Ok(updated) => {
                    if let Err(e) = state.share_repo.save(&updated).await {
                        return Json(serde_json::json!({"status": "error", "reason": format!("{}", e)}));
                    }
                }
                Err(e) => {
                    return Json(serde_json::json!({"status": "error", "reason": format!("{}", e)}));
                }
            }
        }
        Ok(None) => {
            return Json(serde_json::json!({"status": "ok", "message": "Share not found, nothing to cancel"}));
        }
        Err(e) => {
            return Json(serde_json::json!({"status": "error", "reason": format!("{}", e)}));
        }
    }

    Json(serde_json::json!({"status": "ok"}))
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

    let index_version = entries.iter()
        .map(|e| e.modified_at)
        .max()
        .unwrap_or(0);

    Json(SyncIndexResponseDto {
        share_id: query.share_id,
        index_version,
        entries,
    })
}

// ---- Transfer ----

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

async fn handle_transfer_request(
    State(state): State<Arc<ServerAppState>>, 
    Json(req): Json<TransferRequestDto>
) -> Json<TransferResponseDto> {
    println!("[Server] Transfer request from {} with {} files", req.sender_device_id, req.items.len());
    
    if let Err(e) = state.nonce_validator.validate(&req.nonce, req.timestamp) {
        eprintln!("[Server] Transfer request nonce validation failed: {}", e);
        return Json(TransferResponseDto { 
            status: "rejected".to_string(), 
            skip_chunks: vec![] 
        });
    }

    let items: Vec<TransferItem> = req.items.iter().map(|item| {
        let mut chunks = Vec::new();
        let cs = item.chunk_size;
        
        if cs == 0 {
            chunks.push(ChunkInfo { index: 0, offset: 0, size: item.file_size as u32, hash: String::new() });
        } else {
            let mut offset = 0u64;
            let mut idx = 0u32;
            while offset < item.file_size {
                let sz = std::cmp::min(cs as u64, item.file_size - offset) as u32;
                chunks.push(ChunkInfo { index: idx, offset, size: sz, hash: String::new() });
                offset += sz as u64;
                idx += 1;
            }
        }

        TransferItem {
            file_id: crate::domain::model::transfer::FileId(item.file_id.clone()),
            file_path: item.file_path.clone(),
            file_size: item.file_size,
            sha256: item.sha256.clone(),
            status: crate::domain::model::transfer::TransferItemStatus::Pending,
            chunk_manifest: ChunkManifest { chunks, chunk_size: cs },
            chunks_done: 0,
            temp_path: None,
        }
    }).collect();

    let skip_chunks: Vec<SkipChunkDto> = vec![];

    let job = TransferJob {
        job_id: JobId(req.job_id.clone()),
        session_id: req.session_id.clone(),
        job_type: TransferType::Receive,
        peer_device_id: DeviceId(req.sender_device_id.clone()),
        share_id: None,
        state: TransferState::Active { started_at: now_secs() },
        items,
        created_at: now_secs(),
    };

    if let Err(e) = state.transfer_repo.save(job).await {
        eprintln!("[Server] Failed to save receive job: {}", e);
        return Json(TransferResponseDto { 
            status: "rejected".to_string(), 
            skip_chunks: vec![] 
        });
    }

    Json(TransferResponseDto { 
        status: "accepted".to_string(), 
        skip_chunks 
    })
}

async fn handle_chunk_download(
    State(state): State<Arc<ServerAppState>>,
    AxumPath((job_id, file_id, chunk_index)): AxumPath<(String, String, u32)>,
) -> Result<Bytes, StatusCode> {
    let job = state.transfer_repo.find_by_id(&JobId(job_id)).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let item = job.items.iter()
        .find(|i| i.file_id.0 == file_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    let chunk = item.chunk_manifest.chunks.iter()
        .find(|c| c.index == chunk_index)
        .ok_or(StatusCode::NOT_FOUND)?;

    let file_path = item.file_path.clone();
    let offset = chunk.offset;
    let size = chunk.size;

    let data = tokio::task::spawn_blocking(move || {
        ChunkHasher::read_chunk(std::path::Path::new(&file_path), offset, size as u64)
    }).await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Bytes::from(data))
}

#[derive(Deserialize)]
struct ChunkUploadQuery {
    file_id: String,
    chunk_index: u32,
}

async fn handle_chunk_upload(
    State(state): State<Arc<ServerAppState>>,
    AxumPath(job_id): AxumPath<String>,
    Query(params): Query<ChunkUploadQuery>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let job = state.transfer_repo.find_by_id(&JobId(job_id)).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let item = job.items.iter()
    .find(|i| i.file_id.0 == params.file_id)
    .ok_or(StatusCode::NOT_FOUND)?;

    let chunk = item.chunk_manifest.chunks.iter()
    .find(|c| c.index == params.chunk_index)
    .ok_or(StatusCode::NOT_FOUND)?;
    
    if !chunk.hash.is_empty() && !ChunkHasher::verify_chunk(&body, &chunk.hash) {
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
        .truncate(false)  // chunk seek-write: preserve existing file content
        .open(&file_path)
        .map_err(|e| e.to_string())?;

        f.seek(SeekFrom::Start(offset)).map_err(|e| e.to_string())?;
        f.write_all(&data).map_err(|e| e.to_string())

    }).await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let file_id = FileId(params.file_id);
    let updated = job.record_chunk_done(&file_id, params.chunk_index)
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let is_complete = matches!(updated.state, TransferState::Verifying);
    let updated = if is_complete {
        updated.complete().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        updated
    };

    state.transfer_repo.save(updated).await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({"status": "ok"})))
}