use async_trait::async_trait;
use crate::domain::port::audit_repo::{AuditLogRepository, AuditEntry};
use crate::domain::error::DomainError;
use super::Dbpool;

pub struct SqliteAuditLogRepository {
    pool: Dbpool,
}

impl SqliteAuditLogRepository {
    pub fn new(pool: Dbpool) -> Self {
        Self { pool }
    }
}

fn db_err(e: impl std::fmt::Display) -> DomainError {
    DomainError::Persistence(e.to_string())
}

#[async_trait]
impl AuditLogRepository for SqliteAuditLogRepository {
    async fn append(&self, entry: &AuditEntry) -> Result<(), DomainError> {
        let pool = self.pool.clone();
        let id = entry.id.clone();
        let timestamp = entry.timestamp as i64;
        let event_type = entry.event_type.clone();
        let aggregate_id = entry.aggregate_id.clone();
        let details = entry.details.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(db_err)?;
            conn.execute(
                "INSERT INTO audit_logs (id, timestamp, event_type, aggregate_id, details) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![id, timestamp, event_type, aggregate_id, details],
            ).map_err(db_err)?;
            Ok(())
        }).await.map_err(db_err)?
    }
}