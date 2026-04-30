pub mod domain;
pub mod application;
pub mod infrastructure;
pub mod interfaces;

/// Default port for LanSync P2P communication (per PROTOCOL.md)
pub const DEFAULT_PORT: u16 = 53317;

use std::sync::Arc;
use std::path::PathBuf;
use application::{identity_service::DeviceAppService, transfer_service::TransferAppService, share_service::ShareAppService, indexer_service::IndexerService};
use domain::service::{chunking::SizeBasedChunking, policy_enforcer::PolicyEnforcer};
use infrastructure::{
    events::{in_process_bus::InProcessEventBus, audit_handler::AuditEventHandler, cleanup_handler::CascadeCleanupHandler},
    network::discovery::{composite::CompositeDiscovery, mdns::MdnsStrategy},
    persistence::{
        sqlite_audit_repo::SqliteAuditLogRepository,
        sqlite_device_repo::SqliteDeviceRepository,
        sqlite_transfer_repo::SqliteTransferRepository,
        sqlite_share_repo::SqliteShareRepository,
        sqlite_file_index_repo::SqliteFileIndexRepository,
    },
    system::notify_watcher::NotifyWatcherAdapter,
    security::keystore::{FileKeyStore, generate_self_signed_cert},
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Basic setup for DI
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();

    let db_path = "lansync.db";
    let keystore_dir = PathBuf::from(".keystore");

    // 1. Repositories
    let device_repo = Arc::new(SqliteDeviceRepository::new(db_path).expect("Failed to init device repo"));
    let audit_repo = Arc::new(SqliteAuditLogRepository::new(db_path).expect("Failed to init audit repo"));
    let transfer_repo = Arc::new(SqliteTransferRepository::new(db_path).expect("Failed to init transfer repo"));
    let share_repo = Arc::new(SqliteShareRepository::new(db_path).expect("Failed to init share repo"));
    let file_index_repo = Arc::new(SqliteFileIndexRepository::new(db_path).expect("Failed to init file index repo"));

    // 2. EventBus
    let event_bus = Arc::new(InProcessEventBus::new());

    // 3. Security
    let key_store = Arc::new(FileKeyStore::new(keystore_dir.clone()));

    // 4. Discovery
    let discovery = Arc::new(CompositeDiscovery::new(vec![
        Box::new(MdnsStrategy::new()),
    ]));

    // 5. Chunking Strategy
    let chunking_strategy = Arc::new(SizeBasedChunking::new());

    // 6. Policy Enforcer
    let policy_enforcer = Arc::new(PolicyEnforcer::new(device_repo.clone(), share_repo.clone()));

    // File Watcher
    let file_watcher = Arc::new(NotifyWatcherAdapter::new());

    // 7. App Services
    // Load persisted device ID, or generate and save a new one
    let device_id_path = keystore_dir.join("device_id");
    let local_device_id = if device_id_path.exists() {
        let id_str = std::fs::read_to_string(&device_id_path)
            .expect("Failed to read device_id file");
        domain::model::device::DeviceId(id_str.trim().to_string())
    } else {
        let id = domain::model::device::DeviceId(uuid::Uuid::new_v4().to_string());
        std::fs::write(&device_id_path, &id.0)
            .expect("Failed to persist device_id");
        id
    };
    
    // Load or generate TLS cert (keyed by persistent device ID)
    let cert_path = keystore_dir.join(format!("{}.crt", local_device_id.0));
    let key_path = keystore_dir.join(format!("{}.key", local_device_id.0));
    
    let (cert_pem_bytes, key_pem_bytes) = if cert_path.exists() && key_path.exists() {
        let cert = std::fs::read(&cert_path)
            .expect("Failed to read TLS cert file");
        let key = std::fs::read(&key_path)
            .expect("Failed to read TLS key file");
        (cert, key)
    } else {
        let (cert_pem, pk_pem) = generate_self_signed_cert()
            .expect("Failed to generate self-signed cert");
        std::fs::write(&cert_path, cert_pem.as_bytes())
            .expect("Failed to write TLS cert file");
        std::fs::write(&key_path, pk_pem.as_bytes())
            .expect("Failed to write TLS key file");
        (cert_pem.into_bytes(), pk_pem.into_bytes())
    };
    
    // Network Client
    let network_client = Arc::new(ReqwestNetworkClient::new().expect("Failed to init NetworkClient"));

    let identity_service = Arc::new(DeviceAppService::new(
        local_device_id.clone(),
        device_repo.clone(),
        discovery,
        key_store,
        network_client.clone(),
        event_bus.clone(),
    ));

    let transfer_service = Arc::new(TransferAppService::new(
        transfer_repo.clone(),
        device_repo.clone(),
        event_bus.clone(),
        chunking_strategy,
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
    ));

    // 8. Event Handlers
    let audit_handler = AuditEventHandler::new(audit_repo.clone());
    let cleanup_handler = CascadeCleanupHandler::new();
    
    let rx_audit = event_bus.subscribe();
    let rx_cleanup = event_bus.subscribe();

    rt.spawn(async move {
        audit_handler.start(rx_audit).await;
    });
    rt.spawn(async move {
        cleanup_handler.start(rx_cleanup).await;
    });

    // Start HTTPS Server
    let server_config = HttpServerConfig {
        port: DEFAULT_PORT,
        cert_pem: cert_pem_bytes,
        key_pem: key_pem_bytes,
    };
    let server_device_id = local_device_id.clone();
    let server_device_repo = device_repo.clone();
    let server_file_index_repo = file_index_repo.clone();
    let server_share_repo = share_repo.clone();
    rt.spawn(async move {
        if let Err(e) = start_server(
            server_config,
            server_device_id,
            server_device_repo,
            server_file_index_repo,
            server_share_repo,
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
