pub mod domain;
pub mod application;
pub mod infrastructure;
pub mod interfaces;

/// Default port for LanSync P2P communication (per PROTOCOL.md)
pub const DEFAULT_PORT: u16 = 53317;

use std::sync::Arc;
use std::path::PathBuf;
use application::{identity_service::DeviceAppService, transfer_service::TransferAppService, share_service::ShareAppService, indexer_service::IndexerService};
use domain::model::transfer::TransferState;
use domain::port::transfer_repo::TransferRepository;
use domain::port::audit_repo::AuditLogRepository;

use domain::service::{chunking::SizeBasedChunking, policy_enforcer::PolicyEnforcer};
use infrastructure::{
    events::{in_process_bus::InProcessEventBus, audit_handler::AuditEventHandler, cleanup_handler::CascadeCleanupHandler},
    network::discovery::{composite::CompositeDiscovery, mdns::MdnsStrategy},
    persistence::{
        init_database,
        sqlite_audit_repo::SqliteAuditLogRepository,
        sqlite_device_repo::SqliteDeviceRepository,
        sqlite_transfer_repo::SqliteTransferRepository,
        sqlite_share_repo::SqliteShareRepository,
        sqlite_file_index_repo::SqliteFileIndexRepository,
    },
    system::notify_watcher::NotifyWatcherAdapter,
    security::keystore::{FileKeyStore, generate_self_signed_cert},
    security::platform_keystore::PlatformKeyStore,
    network::{http_client::ReqwestNetworkClient, http_server::{start_server, HttpServerConfig}},
};
use interfaces::tauri_cmds::{
    AppState, discover_devices, request_pairing, confirm_pairing,
    send_files, accept_transfer, pause_transfer, resume_transfer, cancel_transfer,
    create_share, invite_to_share, remove_share_member, start_watching_share
};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

fn resolve_data_dir() -> PathBuf {
    let base = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("com.luokai.kfilesync")
}

fn get_hostname() -> String {
    gethostname::gethostname()
    .to_string_lossy()
    .to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Basic setup for DI
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();

    let data_dir = resolve_data_dir();
    std::fs::create_dir_all(&data_dir).expect("Failed to create app data path");

    let db_path = data_dir.join("kfilesync.db");
    let db_path_str = db_path.to_string_lossy().to_string();
    let keystore_dir = data_dir.join("keystore");

    //1. Shared database coonection with consistent pragams
    let db_conn = init_database(&db_path_str).expect("Failed to init database");
    
    //2. Repositories (all share the same connection)
    let device_repo = Arc::new(SqliteDeviceRepository::new(db_conn.clone()));
    let audit_repo = Arc::new(SqliteAuditLogRepository::new(db_conn.clone()));
    let transfer_repo = Arc::new(SqliteTransferRepository::new(db_conn.clone()));
    let share_repo = Arc::new(SqliteShareRepository::new(db_conn.clone()));
    let file_index_repo = Arc::new(SqliteFileIndexRepository::new(db_conn.clone()));

    // 3. EventBus
    let event_bus = Arc::new(InProcessEventBus::new());

    // 4. Security
    let key_store: Arc<dyn crate::domain::port::key_store::KeyStore> = if PlatformKeyStore::is_available() {
        Arc::new(PlatformKeyStore::new())
    } else {
        eprintln!("[Boot]: Platform keystore not available, falling back to file-based keystore");
        Arc::new(FileKeyStore::new(keystore_dir.clone()))
    };

    // 5. Discovery
    let discovery = Arc::new(CompositeDiscovery::new(vec![
        Box::new(MdnsStrategy::new()),
    ]));

    // 6. Chunking Strategy
    let chunking_strategy: Arc<dyn domain::service::chunking::ChunkingStrategy> = Arc::new(SizeBasedChunking::new());

    // 7. Policy Enforcer
    let policy_enforcer = Arc::new(PolicyEnforcer::new(device_repo.clone(), share_repo.clone()));

    // 8. File Watcher
    let file_watcher = Arc::new(NotifyWatcherAdapter::new());

    // 9. Load or generate TLS cert (keyed by persistent device ID)
    std::fs::create_dir_all(&keystore_dir).expect("Failed to create keystore directory");

    let cert_pem_path =keystore_dir.join("device.crt");
    let key_pem_path = keystore_dir.join("device.key");
    let cert_der_path = keystore_dir.join("device.der");

    let (cert_pem_bytes, key_pem_bytes) = if cert_pem_path.exists() && key_pem_path.exists() {
        let cert = std::fs::read(&cert_pem_path)
            .expect("Failed to read TLS cer file");
        let key = std::fs::read(&key_pem_path)
            .expect("Failed to read TLS key file");
        (cert, key)
    } else {
        let (cert_pem, pk_pem, cert_der) = generate_self_signed_cert()
            .expect("Failed to generate self-signed cert");
        std::fs::write(&cert_pem_path, cert_pem.as_bytes())
            .expect("Failed to write TLS cer file");
        std::fs::write(&key_pem_path, pk_pem.as_bytes())
            .expect("Failed to write TLS key file");
        std::fs::write(&cert_der_path, &cert_der)
            .expect("Failed to write TLS DER file");
        (cert_pem.into_bytes(), pk_pem.into_bytes())
    };

    // 10. Deriver DeviceId = SHA-256(cert_der)
    let cert_der = std::fs::read(&cert_der_path)
        .expect("Failed to read cert DER file");
    let local_device_id = domain::model::device::DeviceId(
        infrastructure::security::keystore::device_id_from_cert_der(&cert_der)
    );

    // 11. Compute local alias from hostname
    let local_alias = get_hostname();

    // 12.  Network Client
    let network_client = Arc::new(ReqwestNetworkClient::new().expect("Failed to init NetworkClient"));

    // 13. Crash Recovery: recover interrupted transfers job

    {
        let transfer_repo_ref = transfer_repo.clone();
        let audit_repo_ref = audit_repo.clone();
        
        rt.block_on(async {
            match transfer_repo_ref.find_incomplete_jobs().await {
                Ok(jobs) => {
                   let mut recovered = 0u32;
                   let mut cancelled = 0u32;
                   let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();

                    for job in jobs {
                        let jid=job.job_id.clone();
                        let updated = match &job.state{
                            TransferState::Active{ .. } | TransferState::Verifying =>{
                                recovered+=1;
                                job.pause(None)
                            }
                            TransferState::Pending if job.created_at + 3600 < now =>{ // Pending for over an hour -> cancel
                                cancelled +=1;
                                job.cancel()
                            }
                            _ => continue,
                        };
                        if let Ok(updated_job) = updated {
                            if let Err(e) = transfer_repo_ref.save( updated_job).await {
                                eprintln!("[CrashRecovery] Failed to save job {}: {}", jid.0, e);
                                
                            }
                        }
                    }
                    if recovered > 0 || cancelled > 0 {
                        println!("[CrashRecovery] Recovered {} jobs, cancelled {} stale jobs", recovered, cancelled);
                        let entry = domain::port::audit_repo::AuditEntry {
                            id: uuid::Uuid::new_v4().to_string(),
                            timestamp: now,
                            event_type: "CrashRecovery".to_string(),
                            aggregate_id: "system".to_string(),
                            details: format!("recovered={}, cancelled={}", recovered, cancelled)
                        };
                        let _ = audit_repo_ref.append(&entry).await;
                    }
                },
                Err(e) => {
                    eprintln!("[CrashRecovery] Failed to query imcomplete jobs: {}", e);
                }
            }
        });
    }

    // 14. App Services
    let identity_service = Arc::new(DeviceAppService::new(
        local_device_id.clone(),
        local_alias.clone(),
        device_repo.clone(),
        discovery,
        key_store,
        network_client.clone(),
        event_bus.clone(),
    ));

    let transfer_service = Arc::new(TransferAppService::new(
        local_device_id.clone(),
        transfer_repo.clone(),
        device_repo.clone(),
        event_bus.clone(),
        chunking_strategy.clone(),
        network_client.clone(),
    ));

    let share_service = Arc::new(ShareAppService::new(
        local_device_id.clone(),
        share_repo.clone(),
        device_repo.clone(),
        network_client.clone(),
        policy_enforcer.clone(),
        event_bus.clone(),
    ));

    let indexer_service = Arc::new(IndexerService::new(
        file_index_repo.clone(),
        share_repo.clone(),
        file_watcher.clone(),
        local_device_id.clone(),
        chunking_strategy,
    ));

    // 15. Event Handlers
    let audit_handler = AuditEventHandler::new(audit_repo.clone());
    let cleanup_handler = CascadeCleanupHandler::new(
        transfer_repo.clone(),
        share_repo.clone(),
    );
    
    let rx_audit = event_bus.subscribe();
    let rx_cleanup = event_bus.subscribe();

    rt.spawn(async move {
        audit_handler.start(rx_audit).await;
    });
    rt.spawn(async move {
        cleanup_handler.start(rx_cleanup).await;
    });

    // 16. Tombstone Cleanup Scheduler: every 6 hours, purge tombstones older than 30 days
    {
        let file_index_repo_ref = file_index_repo.clone();
        rt.spawn(async move {
            const CLEANUP_INTERVAL: std::time::Duration = std::time::Duration::from_secs(6 * 3600);
            const TOMBSTONE_TTL_SECS: u64 = 30 * 24 * 3600;

            loop {
                tokio::time::sleep(CLEANUP_INTERVAL).await;

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                let cutoff = now.saturating_sub(TOMBSTONE_TTL_SECS);

                match file_index_repo_ref.cleanup_expired_tombstones(cutoff as i64).await {
                    Ok(count) if count > 0 => {
                        println!("[TombstoneCleanup] Purged {} expired tombstones", count);
                    }
                    Err(e) => {
                        eprintln!("[TombstoneCleanup] Error: {:?}", e);
                    }
                    _ => {}
                }
            }
        });
    }

    // 17. Start HTTPS Server
    let server_config = HttpServerConfig {
        port: DEFAULT_PORT,
        cert_pem: cert_pem_bytes,
        key_pem: key_pem_bytes,
    };
    let server_device_id = local_device_id.clone();
    let server_alias = local_alias;
    let server_device_repo = device_repo.clone();
    let server_file_index_repo = file_index_repo.clone();
    let server_share_repo = share_repo.clone();
    let server_transfer_repo = transfer_repo.clone();
    rt.spawn(async move {
        if let Err(e) = start_server(
            server_config,
            server_device_id,
            server_alias,
            server_device_repo,
            server_file_index_repo,
            server_share_repo,
            server_transfer_repo,
        ).await {
            eprintln!("Failed to start HTTPS server: {}", e);
        }
    });
    
    let app_state = AppState {
        identity_service,
        transfer_service,
        share_service,
        indexer_service,
    };

    tauri::Builder::default()
        .manage(app_state)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            discover_devices,
            request_pairing,
            confirm_pairing,
            send_files,
            accept_transfer,
            pause_transfer,
            resume_transfer,
            cancel_transfer,
            create_share,
            invite_to_share,
            remove_share_member,
            start_watching_share
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
