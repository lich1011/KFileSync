use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use crate::domain::model::device::{Device, DeviceId, DeviceState};
use crate::domain::port::repository::DeviceRepository;
use crate::domain::error::DomainError;
use async_trait::async_trait;

pub struct SqliteDeviceRepository {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteDeviceRepository {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }
}

fn db_err(e: impl std::fmt::Display) -> DomainError {
    DomainError::Persistence(e.to_string())
}   

#[async_trait]
impl DeviceRepository for SqliteDeviceRepository {
    async fn find_by_id(&self, id: DeviceId) -> Result<Option<Device>, DomainError> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(db_err)?;
            let mut stmt = conn.prepare("SELECT state_json FROM devices WHERE id = ?1").map_err(db_err)?;
            let mut rows = stmt.query([&id.0]).map_err(db_err)?;
            if let Some(row) = rows.next().map_err(db_err)? {
                let state_json: String = row.get(0).map_err(db_err)?;
                let state = serde_json::from_str(&state_json).map_err(db_err)?;
                Ok(Some(Device { id: id.clone(), state }))
            } else {
                Ok(None)
            }
        }).await.map_err(db_err)?
    }

    async fn find_paired(&self) -> Result<Vec<Device>, DomainError> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(db_err)?;
            let mut stmt = conn.prepare("SELECT id, state_json FROM devices").map_err(db_err)?;
            let rows = stmt.query_map([], |row| {
                let id: String = row.get(0)?;
                let state_json: String = row.get(1)?;
                Ok((id, state_json))
            }).map_err(db_err)?;

            let mut paired_devices = Vec::new();
            for row in rows {
                let (id, state_json) = row.map_err(db_err)?;
                let state: DeviceState = serde_json::from_str(&state_json).map_err(db_err)?;
                if let DeviceState::Paired(_) = state {
                    paired_devices.push(Device { id: DeviceId(id), state });
                }
            }
            Ok(paired_devices)
        }).await.map_err(db_err)?
    }

    async fn save(&self, device: Device) -> Result<(), DomainError> {
        let conn = self.conn.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(db_err)?;
            let state_json = serde_json::to_string(&device.state).map_err(db_err)?;
            conn.execute(
                "INSERT OR REPLACE INTO devices (id, state_json) VALUES (?1, ?2)",
                [&device.id.0, &state_json],
            ).map_err(db_err)?;
            Ok::<(), DomainError>(())
        }).await.map_err(db_err)?;
        Ok(())
    }

    async fn update_trust_status(&self, id: DeviceId, status: DeviceState) -> Result<(), DomainError> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(db_err)?;
            // Check whether the device exists first
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM devices WHERE id = ?1",
                [&id.0],
                |row| row.get(0),
            ).map_err(db_err)?;

            if count == 0 {
                return Err(DomainError::DeviceNotFound(id.0.clone()));
            }

            let state_json = serde_json::to_string(&status).map_err(db_err)?;
            conn.execute(
                "UPDATE devices SET state_json = ?1 WHERE id = ?2",
                [&state_json, &id.0],
            ).map_err(db_err)?;
            Ok(())
        }).await.map_err(db_err)?
    }
}
