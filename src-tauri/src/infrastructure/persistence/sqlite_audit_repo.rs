use rusqlite::{Connection, Result as SqlResult};
use std::sync::Mutex;
use async_trait::async_trait;
use crate::domain::port::audit_repo::{AuditLogRepository, AuditEntry};

pub struct SqliteAuditLogRepository {
    conn: Mutex<Connection>,
}

impl SqliteAuditLogRepository {
    pub fn new(db_path: &str) -> SqlResult<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS audit_logs (
                id TEXT PRIMARY KEY,
                timestamp INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                aggregate_id TEXT NOT NULL,
                details TEXT NOT NULL
            )",
            [],
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }
}

#[async_trait]
impl AuditLogRepository for SqliteAuditLogRepository {
    async fn append(&self, entry: &AuditEntry) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO audit_logs (id, timestamp, event_type, aggregate_id, details) VALUES (?1, ?2, ?3, ?4, ?5)",
            [&entry.id, &entry.timestamp.to_string(), &entry.event_type, &entry.aggregate_id, &entry.details],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }
}
