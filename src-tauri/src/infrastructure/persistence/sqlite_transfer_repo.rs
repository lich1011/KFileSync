use rusqlite::{Connection, Result as SqlResult, OptionalExtension};
use std::sync::Mutex;
use async_trait::async_trait;
use crate::domain::model::transfer::{JobId, TransferJob, TransferItem, TransferType, TransferState};
use crate::domain::model::device::DeviceId;
use crate::domain::port::transfer_repo::TransferRepository;

pub struct SqliteTransferRepository {
    conn: Mutex<Connection>,
}

impl SqliteTransferRepository {
    pub fn new(db_path: &str) -> SqlResult<Self> {
        let conn = Connection::open(db_path)?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS transfer_jobs (
                job_id            TEXT PRIMARY KEY,
                session_id        TEXT NOT NULL,
                job_type          TEXT NOT NULL,
                peer_device_id    TEXT NOT NULL,
                share_id          TEXT,
                state_json        TEXT NOT NULL,
                status            TEXT NOT NULL,
                created_at        INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS transfer_items (
                job_id          TEXT NOT NULL,
                file_id         TEXT NOT NULL,
                item_json       TEXT NOT NULL,
                PRIMARY KEY (job_id, file_id),
                FOREIGN KEY (job_id) REFERENCES transfer_jobs(job_id) ON DELETE CASCADE
            )",
            [],
        )?;

        // 索引：按状态快速检索未完成任务
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_transfer_jobs_status ON transfer_jobs(status)",
            [],
        )?;

        Ok(Self { conn: Mutex::new(conn) })
    }
}

#[async_trait]
impl TransferRepository for SqliteTransferRepository {
    async fn find_by_id(&self, job_id: &JobId) -> Result<Option<TransferJob>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        
        let mut stmt = conn.prepare("SELECT session_id, job_type, peer_device_id, share_id, state_json, created_at FROM transfer_jobs WHERE job_id = ?1").map_err(|e| e.to_string())?;
        
        let job_row = stmt.query_row([&job_id.0], |row| {
            let session_id: String = row.get(0)?;
            let job_type_str: String = row.get(1)?;
            let peer_device_id_str: String = row.get(2)?;
            let share_id: Option<String> = row.get(3)?;
            let state_json: String = row.get(4)?;
            let created_at: u64 = row.get(5)?;
            Ok((session_id, job_type_str, peer_device_id_str, share_id, state_json, created_at))
        }).optional().map_err(|e| e.to_string())?;

        let (session_id, job_type_str, peer_device_id_str, share_id, state_json, created_at) = match job_row {
            Some(row) => row,
            None => return Ok(None),
        };

        let mut items_stmt = conn.prepare("SELECT item_json FROM transfer_items WHERE job_id = ?1").map_err(|e| e.to_string())?;
        let items_iter = items_stmt.query_map([&job_id.0], |row| {
            let item_json: String = row.get(0)?;
            Ok(item_json)
        }).map_err(|e| e.to_string())?;

        let mut items = Vec::new();
        for item_res in items_iter {
            let item_json = item_res.map_err(|e| e.to_string())?;
            let item: TransferItem = serde_json::from_str(&item_json).map_err(|e| e.to_string())?;
            items.push(item);
        }

        let state = serde_json::from_str(&state_json).map_err(|e| e.to_string())?;
        let job_type = match job_type_str.as_str() {
            "Send" => TransferType::Send,
            "Receive" => TransferType::Receive,
            "SyncPull" => TransferType::SyncPull,
            "SyncPush" => TransferType::SyncPush,
            _ => TransferType::Send,
        };

        Ok(Some(TransferJob {
            job_id: job_id.clone(),
            session_id,
            job_type,
            peer_device_id: DeviceId(peer_device_id_str),
            share_id,
            state,
            items,
            created_at,
        }))
    }

    async fn save(&self, job: TransferJob) -> Result<(), String> {
        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        
        let job_type_str = match job.job_type {
            TransferType::Send => "Send",
            TransferType::Receive => "Receive",
            TransferType::SyncPull => "SyncPull",
            TransferType::SyncPush => "SyncPush",
        };
        let state_json = serde_json::to_string(&job.state).map_err(|e| e.to_string())?;
        
        let status = match &job.state {
            TransferState::Pending => "pending",
            TransferState::Active { .. } => "active",
            TransferState::Paused { .. } => "paused",
            TransferState::Verifying => "verifying",
            TransferState::Completed { .. } => "completed",
            TransferState::Failed { .. } => "failed",
            TransferState::Cancelled => "cancelled",
        };

        tx.execute(
            "INSERT OR REPLACE INTO transfer_jobs (job_id, session_id, job_type, peer_device_id, share_id, state_json, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![job.job_id.0, job.session_id, job_type_str, job.peer_device_id.0, job.share_id, state_json, status, job.created_at],
        ).map_err(|e| e.to_string())?;

        tx.execute("DELETE FROM transfer_items WHERE job_id = ?1", [&job.job_id.0]).map_err(|e| e.to_string())?;

        for item in job.items {
            let item_json = serde_json::to_string(&item).map_err(|e| e.to_string())?;
            tx.execute(
                "INSERT INTO transfer_items (job_id, file_id, item_json) VALUES (?1, ?2, ?3)",
                [&job.job_id.0, &item.file_id.0, &item_json],
            ).map_err(|e| e.to_string())?;
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn find_incomplete_jobs(&self) -> Result<Vec<TransferJob>, String> {
        let job_ids = {
            let conn = self.conn.lock().map_err(|e| e.to_string())?;
            
            let mut stmt = conn.prepare("SELECT job_id FROM transfer_jobs WHERE status NOT IN ('completed', 'failed', 'cancelled')").map_err(|e| e.to_string())?;
            let rows = stmt.query_map([], |row| {
                let id: String = row.get(0)?;
                Ok(id)
            }).map_err(|e| e.to_string())?;

            let mut ids = Vec::new();
            for id in rows {
                ids.push(JobId(id.map_err(|e| e.to_string())?));
            }
            ids
        };

        let mut incomplete = Vec::new();
        for id in job_ids {
            if let Some(job) = self.find_by_id(&id).await? {
                incomplete.push(job);
            }
        }
        Ok(incomplete)
    }
}
