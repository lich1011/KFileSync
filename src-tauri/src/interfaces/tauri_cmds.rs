use crate::application::identity_service::DeviceAppService;
use crate::application::transfer_service::TransferAppService;
use crate::application::indexer_service::IndexerService;
use crate::application::http_sync_flow::HttpSyncFlow;
use crate::domain::model::device::DeviceId;
use crate::domain::model::transfer::{FileRequest, JobId};
use std::sync::Arc;
use crate::domain::port::repository::DeviceRepository;
use crate::domain::port::file_index_repo::FileIndexRepository;
use tauri::State;
use serde::{Serialize, Deserialize};

// We wrap the service in Arc<Mutex<...>> or just Arc, depending on how it's injected
pub struct AppState {
    pub identity_service: Arc<DeviceAppService>,
    pub transfer_service: Arc<TransferAppService>,
    pub share_service: Arc<crate::application::share_service::ShareAppService>,
    pub indexer_service: Arc<IndexerService>,
    pub sync_flow: Arc<HttpSyncFlow>,
    pub device_repo: Arc<dyn DeviceRepository>,
    pub file_index_repo: Arc<dyn FileIndexRepository>,
    pub share_repo: Arc<dyn crate::domain::port::share_repo::ShareRepository>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredDeviceDto {
    pub id: String,
    pub alias: String,
    pub address: String,
}

#[tauri::command]
pub async fn discover_devices(state: State<'_, AppState>) -> Result<Vec<DiscoveredDeviceDto>, String> {
    let devices = state.identity_service.discover_devices().await.map_err(|e| e.to_string())?;
    let dtos = devices.into_iter().map(|d| DiscoveredDeviceDto {
        id: d.device_id.0,
        alias: d.alias,
        address: d.address,
    }).collect();
    Ok(dtos)
}

#[tauri::command]
pub async fn request_pairing(target_id: String, state: State<'_, AppState>) -> Result<String, String> {
    let session = state.identity_service.initiate_pairing(&DeviceId(target_id)).await.map_err(|e| e.to_string())?;
    // Return the session id and pin code to the UI; UI must pass session_id back on confirm
    Ok(session.pin_code)
}

#[tauri::command]
pub async fn confirm_pairing(
    target_id: String,
    pin_code: String,
    cert_pem: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.identity_service.confirm_pairing(&DeviceId(target_id), &pin_code, cert_pem).await.map_err(|e| e.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRequestDto {
    pub file_path: String,
    pub file_size: u64,
    pub sha256: String,
}

#[tauri::command]
pub async fn send_files(
    peer_id: String,
    files: Vec<FileRequestDto>,
    state: State<'_, AppState>,
) -> Result<String, String> {

    //The frontend often cannot stat files (especially under Tarui's sandbox), so it sends
    //file_size= 0 / shan256 = "". We backfill these from disk here so the transfer job is
    //create with accurate metadata before chunking.

    let mut requests = Vec::with_capacity(files.len());
    for file in files{
        let mut req = FileRequest{
            file_path: file.file_path,
            file_size: file.file_size,
            sha256: file.sha256
        };
        if req.file_size == 0{
            match std::fs::metadata(&req.file_path) {
                Ok(meta) => req.file_size = meta.len(),
                Err(e) => return Err(format!("Cannot read file {}: {}",req.file_path, e))
            }
        }
        requests.push(req);
    }
    let job_id = state.transfer_service.clone().send_files(DeviceId(peer_id), requests).await.map_err(|e| e.to_string())?;  
    Ok(job_id.0)
}

#[tauri::command]
pub async fn accept_transfer(job_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.transfer_service.accept_transfer(&JobId(job_id)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pause_transfer(job_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.transfer_service.pause_transfer(&JobId(job_id)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn resume_transfer(job_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.transfer_service.resume_transfer(&JobId(job_id)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cancel_transfer(job_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.transfer_service.cancel_transfer(&JobId(job_id)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_share(
    share_name: String,
    local_path: String,
    sync_mode_str: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let sync_mode = match sync_mode_str.as_str() {
        "send_only" => crate::domain::model::share::SyncMode::SendOnly,
        "receive_only" => crate::domain::model::share::SyncMode::ReceiveOnly,
        _ => crate::domain::model::share::SyncMode::TwoWay,
    };
    
    let share_id = uuid::Uuid::new_v4().to_string();
    let id = state.share_service.create_share(share_id, share_name, local_path, sync_mode).await.map_err(|e| e.to_string())?;
    Ok(id.0)
}

#[tauri::command]
pub async fn invite_to_share(
    share_id: String,
    peer_id: String,
    permission_str: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let permission = match permission_str.as_str() {
        "read_only" => crate::domain::model::share::SharePermission::ReadOnly,
        "send_only" => crate::domain::model::share::SharePermission::SendOnly,
        "receive_only" => crate::domain::model::share::SharePermission::ReceiveOnly,
        _ => crate::domain::model::share::SharePermission::ReadWrite,
    };
    
    state.share_service.invite_device(
        &crate::domain::model::share::ShareId(share_id),
        &DeviceId(peer_id),
        permission
    ).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_share_member(
    share_id: String,
    peer_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.share_service.remove_member(
        &crate::domain::model::share::ShareId(share_id),
        &DeviceId(peer_id)
    ).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_watching_share(share_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.indexer_service.start_watching(&crate::domain::model::share::ShareId(share_id)).await.map_err(|e| e.to_string())
}


#[tauri::command]
pub async fn reject_pairing(target_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.identity_service.reject_pairing(&DeviceId(target_id)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_manual_device(ip: String, _state: State<'_, AppState>) -> Result<DiscoveredDeviceDto, String> {
    let port = crate::DEFAULT_PORT;
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("https://{}:{}/api/lansync/v1/info", ip, port);
    let resp = client.get(&url).send().await.map_err(|e| format!("Cannot reach {}: {}", ip, e))?;

    #[derive(serde::Deserialize)]
    struct InfoResp { device_id: String, alias: String }

    let info: InfoResp = resp.json().await.map_err(|e| format!("Invalid response: {}", e))?;
    
    Ok(DiscoveredDeviceDto {
        id: info.device_id,
        alias: info.alias,
        address: ip,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairedDeviceDto {
    pub id: String,
    pub alias: String,
    pub address: String,
    pub paired_at: u64,
    pub last_seen_at: Option<u64>,
    pub online: bool,
}

#[tauri::command]
pub async fn get_paired_devices(state: State<'_, AppState>) -> Result<Vec<PairedDeviceDto>, String> {
    let devices = state.device_repo.find_paired().await.map_err(|e| e.to_string())?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let dtos = devices.into_iter().filter_map(|d| {
        match d.state {
            crate::domain::model::device::DeviceState::Paired(data) => {
                let online = data.last_seen_at
                    .map(|t| now.saturating_sub(t) < 90)
                    .unwrap_or(false);
                
                Some(PairedDeviceDto {
                    id: d.id.0,
                    alias: data.alias,
                    address: data.address,
                    paired_at: data.paired_at,
                    last_seen_at: data.last_seen_at,
                    online,
                })
            }
            _ => None,
        }
    }).collect();

    Ok(dtos)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatusDto {
    pub share_id: String,
    pub total_files: u32,
    pub conflicts: u32,
}

#[tauri::command]
pub async fn get_sync_status(share_id: String, state: State<'_, AppState>) -> Result<SyncStatusDto, String> {
    let sid = crate::domain::model::share::ShareId(share_id.clone());
    let entries = state.file_index_repo.find_all_by_share(&sid).await.map_err(|e| e.to_string())?;
    let conflicts = state.file_index_repo.find_conflicts_by_share(&sid).await.map_err(|e| e.to_string())?;

    Ok(SyncStatusDto {
        share_id,
        total_files: entries.len() as u32,
        conflicts: conflicts.len() as u32,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConflictDto {
    pub conflict_id: String,
    pub share_id: String,
    pub file_path: String,
    pub resolution: String,
}

#[tauri::command]
pub async fn get_conflicts(share_id: String, state: State<'_, AppState>) -> Result<Vec<SyncConflictDto>, String> {
    let sid = crate::domain::model::share::ShareId(share_id);
    let conflicts = state.file_index_repo.find_conflicts_by_share(&sid).await.map_err(|e| e.to_string())?;
    
    let dtos = conflicts.into_iter().map(|c| SyncConflictDto {
        conflict_id: c.conflict_id,
        share_id: c.local.share_id.0.clone(),
        file_path: c.path,
        resolution: format!("{:?}", c.resolution),
    }).collect();

    Ok(dtos)
}

#[tauri::command]
pub async fn resolve_conflict(conflict_id: String, resolution: String, state: State<'_, AppState>) -> Result<(), String> {
    
    match resolution.as_str() {
        "delete" | "keep_local" | "keep_remote" | "keep_both" => {
            state.file_index_repo.delete_conflict(&conflict_id).await.map_err(|e| e.to_string())
        }
        other => Err(
            format!("Unsupported resolution '{}'. Use one of: delete | keep_local | keep_both | keep_remote", other)
        )
    }
}

#[tauri::command]
pub async fn trigger_sync(share_id: String, peer_id: String, state: State<'_, AppState>) -> Result<String, String> {
    let sid = crate::domain::model::share::ShareId(share_id);
    let pid = DeviceId(peer_id);
    let plan = state.sync_flow.execute(&sid, &pid).await.map_err(|e| e.to_string())?;
    
    Ok(format!("Synced: {} pull, {} push, {} conflicts", plan.to_pull.len(), plan.to_push.len(), plan.conflicts.len()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareMemberDto {
    pub device_id: String,
    pub permission: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareInfoDto {
    pub share_id: String,
    pub share_name: String,
    pub local_path: String,
    pub sync_mode: String,
    pub status: String,
    pub created_by: String,
    pub created_at: u64,
    pub members: Vec<ShareMemberDto>,
}

#[tauri::command]
pub async fn list_shares(state: State<'_, AppState>) -> Result<Vec<ShareInfoDto>, String> {
    use crate::domain::model::share::{SharePermission, SyncMode, ShareStatus};
    let shares = state.share_repo.find_all().await.map_err(|e| e.to_string())?;
    let dtos = shares.into_iter().map(|s| ShareInfoDto {
        share_id: s.share_id.0,
        share_name: s.share_name,
        local_path: s.local_path,
        sync_mode: match s.sync_mode {
            SyncMode::TwoWay => "two_way".into(),
            SyncMode::SendOnly => "send_only".into(),
            SyncMode::ReceiveOnly => "receive_only".into(),
        },
        status: match s.status {
            ShareStatus::Active => "active".into(),
            ShareStatus::Paused => "paused".into(),
            ShareStatus::Error(msg) => format!("error: {}", msg),
        },
        created_by: s.created_by.0,
        created_at: s.created_at,
        members: s.members.into_iter().map(|m| ShareMemberDto {
            device_id: m.device_id.0,
            permission: match m.permission {
                SharePermission::ReadOnly => "read_only".into(),
                SharePermission::ReadWrite => "read_write".into(),
                SharePermission::SendOnly => "send_only".into(),
                SharePermission::ReceiveOnly => "receive_only".into(),
            },
        }).collect(),
    }).collect();
    
    Ok(dtos)
}