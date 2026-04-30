use crate::domain::port::event_bus::DomainEvent;
use crate::domain::port::audit_repo::{AuditLogRepository, AuditEntry};
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;
use uuid::Uuid;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct AuditEventHandler {
    repo: Arc<dyn AuditLogRepository>,
}

impl AuditEventHandler {
    pub fn new(repo: Arc<dyn AuditLogRepository>) -> Self {
        Self { repo }
    }

    /// Runs the event loop. This method blocks (awaits) until the receiver is closed.
    /// Must be spawned inside a tokio::spawn in the caller (lib.rs already does this).
    pub async fn start(&self, mut rx: Receiver<Arc<dyn DomainEvent>>) {
        while let Ok(event) = rx.recv().await {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let entry = AuditEntry {
                id: Uuid::new_v4().to_string(),
                timestamp,
                event_type: event.event_type().to_string(),
                aggregate_id: event.aggregate_id().to_string(),
                details: format!("Event: {}", event.event_type()),
            };

            if let Err(e) = self.repo.append(&entry).await {
                eprintln!("[AuditEventHandler] Failed to write audit log: {}", e);
            }
        }
    }
}
