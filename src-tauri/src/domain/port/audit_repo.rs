use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub timestamp: u64,
    pub event_type: String,
    pub aggregate_id: String,
    pub details: String,
}

#[async_trait]
pub trait AuditLogRepository: Send + Sync {
    async fn append(&self, entry: &AuditEntry) -> Result<(), String>;
    // async fn query(&self, filter: &AuditFilter) -> Result<Vec<AuditEntry>, String>;
}
