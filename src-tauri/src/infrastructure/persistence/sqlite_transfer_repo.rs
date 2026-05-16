use rusqlite::OptionalExtension;
use async_trait::async_trait;
use crate::domain::model::transfer::{JobId, TransferJob, TransferItem, TransferType, TransferState};
use crate::domain::model::device::DeviceId;
use crate::domain::port::transfer_repo::TransferRepository;
use crate::domain::error::DomainError;
use super::Dbpool;

fn db_err(e: impl std::fmt::Display) -> DomainError {
    DomainError::Persistence(e.to_string())
}  

pub struct SqliteTransferRepository {
    pool: Dbpool,
}

impl SqliteTransferRepository {
    pub fn new(pool: Dbpool) -> Self {
        Self { pool } 
    }
}

#[async_trait]
impl TransferRepository for SqliteTransferRepository {
    async fn find_by_id(&self, job_id: &JobId) -> Result<Option<TransferJob>,   DomainError> {
        let pool = self.pool.clone();
        let job_id = job_id.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(db_err)?;
        
            let mut stmt = conn.prepare("SELECT session_id, job_type, peer_device_id, share_id, state_json, created_at FROM transfer_jobs WHERE job_id = ?1").map_err(db_err)?;
        
            let job_row = stmt.query_row([&job_id.0], |row| {
                let session_id: String = row.get(0)?;
                let job_type_str: String = row.get(1)?;
                let peer_device_id_str: String = row.get(2)?;
                let share_id: Option<String> = row.get(3)?;
                let state_json: String = row.get(4)?;
                let created_at: u64 = row.get(5)?;
                Ok((session_id, job_type_str, peer_device_id_str, share_id, state_json, created_at))
            }).optional().map_err(db_err)?;

            let (session_id, job_type_str, peer_device_id_str, share_id, state_json, created_at) = match job_row {
                Some(row) => row,
                None => return Ok(None),
            };

            let mut items_stmt = conn.prepare("SELECT item_json FROM transfer_items WHERE job_id = ?1").map_err(db_err)?;
            let items_iter = items_stmt.query_map([&job_id.0], |row| {
                let item_json: String = row.get(0)?;
                Ok(item_json)
            }).map_err(db_err)?;

            let mut items = Vec::new();
            for item_res in items_iter {
                let item_json = item_res.map_err(db_err)?;
                let item: TransferItem = serde_json::from_str(&item_json).map_err(db_err)?;
                items.push(item);
            }

            let state = serde_json::from_str(&state_json).map_err(db_err)?;
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
        }).await.map_err(db_err)?
    }

    async fn save(&self, job: TransferJob) -> Result<(), DomainError> {
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get().map_err(db_err)?;
            let tx = conn.transaction().map_err(db_err)?;
        
            let job_type_str = match job.job_type {
                TransferType::Send => "Send",
                TransferType::Receive => "Receive",
                TransferType::SyncPull => "SyncPull",
                TransferType::SyncPush => "SyncPush",
            };
            let state_json = serde_json::to_string(&job.state).map_err(db_err)?;
        
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
            ).map_err(db_err)?;

            tx.execute("DELETE FROM transfer_items WHERE job_id = ?1", [&job.job_id.0]).map_err(db_err)?;

            for item in job.items {
                let item_json = serde_json::to_string(&item).map_err(db_err)?;
                tx.execute(
                    "INSERT INTO transfer_items (job_id, file_id, item_json) VALUES (?1, ?2, ?3)",
                    [&job.job_id.0, &item.file_id.0, &item_json],
                ).map_err(db_err)?;
            }

            tx.commit().map_err(db_err)?;
            Ok(())
        }).await.map_err(db_err)?
    }

    async fn find_incomplete_jobs(&self) -> Result<Vec<TransferJob>, DomainError> {
        let pool =self.pool.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(db_err)?;
            
            let mut stmt = conn.prepare(
                "SELECT j.job_id, j.session_id, j.job_type, j.peer_device_id, j.share_id, j.state_json, j.status, j.created_at 
                 FROM transfer_jobs j
                 WHERE j.status NOT IN ('completed', 'failed', 'cancelled')"
            ).map_err(db_err)?;

            let job_rows = stmt.query_map([], |row| {
                let job_id: String = row.get(0)?;
                let session_id: String = row.get(1)?;
                let job_type_str: String = row.get(2)?;
                let peer_device_id: String = row.get(3)?;
                let share_id: Option<String> = row.get(4)?;
                let state_json: String = row.get(5)?;
                let status: String = row.get(6)?;
                let created_at: u64 = row.get(7)?;
                Ok((job_id, session_id, job_type_str, peer_device_id, share_id, state_json, status, created_at))
            }).map_err(db_err)?;

            let mut jobs = Vec::new();
            for job_row in job_rows {
                let (job_id_str, session_id, job_type_str, peer_device_id_str, share_id, state_json, _status, created_at) = job_row.map_err(db_err)?;
                
                let mut items_stmt = conn.prepare(
                    "SELECT item_json FROM transfer_items WHERE job_id = ?1"
                ).map_err(db_err)?;

                let items_iter = items_stmt.query_map([&job_id_str], |row| {
                    let item_json: String = row.get(0)?;
                    Ok(item_json)
                }).map_err(db_err)?;

                let mut items = Vec::new();
                for item_res in items_iter {
                    let item_json = item_res.map_err(db_err)?;
                    let item: TransferItem = serde_json::from_str(&item_json).map_err(db_err)?;
                    items.push(item);
                }   

                let state = serde_json::from_str(&state_json).map_err(db_err)?;
                let job_type = match job_type_str.as_str() {
                    "Send" => TransferType::Send,
                    "Receive" => TransferType::Receive,
                    "SyncPull" => TransferType::SyncPull,
                    "SyncPush" => TransferType::SyncPush,
                    _ => TransferType::Send,
                };

                jobs.push(TransferJob {
                    job_id: JobId(job_id_str),
                    session_id,
                    job_type,
                    peer_device_id: DeviceId(peer_device_id_str),
                    share_id,
                    state,
                    items,
                    created_at,
                });
            }   
            Ok(jobs)
        }).await.map_err(db_err)?
    }

    async fn find_actions_by_peer(&self, device_id: &DeviceId) -> Result<Vec<TransferJob>, DomainError> {
        let pool = self.pool.clone();
        let device_id = device_id.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(db_err)?;

            let mut stmt = conn.prepare(
                "SELECT j.job_id, j.session_id, j.job_type, j.peer_device_id, j.share_id, j.state_json, j.created_at 
                FROM transfer_jobs j 
                WHERE j.peer_device_id = ?1 AND j.status IN ('pending', 'active', 'paused')"
            ).map_err(db_err)?;
            
            let job_rows = stmt.query_map([&device_id.0], |row| {
                let job_id: String = row.get(0)?;
                let session_id: String = row.get(1)?;
                let job_type_str: String = row.get(2)?;
                let peer_device_id: String = row.get(3)?;
                let share_id: Option<String> = row.get(4)?;
                let state_json: String = row.get(5)?;
                let created_at: u64 = row.get(6)?;
                Ok((job_id, session_id, job_type_str, peer_device_id, share_id, state_json, created_at))
            }).map_err(db_err)?;

            let mut jobs = Vec::new();
            for job_row in job_rows {
                let (job_id, session_id, job_type_str, peer_device_id, share_id, state_json, created_at) = job_row.map_err(db_err)?;
                
                let mut items_stmt = conn.prepare(
                    "SELECT item_json FROM transfer_items WHERE job_id = ?1"
                ).map_err(db_err)?;
                let items: Vec<TransferItem> = items_stmt.query_map([&job_id], |row| {
                    let item_json: String = row.get(0)?;
                    Ok(item_json)
                }).map_err(db_err)?
                .filter_map(|r| r.ok())
                .filter_map(|json| serde_json::from_str(&json).ok())
                .collect();

                let state = serde_json::from_str(&state_json).map_err(db_err)?;
                let job_type = match job_type_str.as_str() {
                    "Send" => TransferType::Send,
                    "Receive" => TransferType::Receive,
                    "SyncPull" => TransferType::SyncPull,
                    "SyncPush" => TransferType::SyncPush,
                    _ => TransferType::Send,
                };

                jobs.push(TransferJob {
                    job_id: JobId(job_id),
                    session_id,
                    job_type,
                    peer_device_id: DeviceId(peer_device_id),
                    share_id,
                    state,
                    items,
                    created_at,
                });
            }   
            Ok(jobs)
        }).await.map_err(db_err)?
    }
}
