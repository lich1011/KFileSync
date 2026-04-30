use rusqlite::{Connection, Result as SqlResult};
use std::sync::Mutex;
use crate::domain::model::device::{Device, DeviceId, DeviceState};
use crate::domain::port::repository::DeviceRepository;

pub struct SqliteDeviceRepository {
    conn: Mutex<Connection>,
}

impl SqliteDeviceRepository {
    pub fn new(db_path: &str) -> SqlResult<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS devices (
                id TEXT PRIMARY KEY,
                state_json TEXT NOT NULL
            )",
            [],
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }
}

use async_trait::async_trait;

#[async_trait]
impl DeviceRepository for SqliteDeviceRepository {
    async fn find_by_id(&self, id: DeviceId) -> Result<Option<Device>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare("SELECT state_json FROM devices WHERE id = ?1").map_err(|e| e.to_string())?;
        let mut rows = stmt.query([&id.0]).map_err(|e| e.to_string())?;

        if let Some(row) = rows.next().map_err(|e| e.to_string())? {
            let state_json: String = row.get(0).map_err(|e| e.to_string())?;
            let state = serde_json::from_str(&state_json).map_err(|e| e.to_string())?;
            Ok(Some(Device { id: id.clone(), state }))
        } else {
            Ok(None)
        }
    }

    async fn find_paired(&self) -> Result<Vec<Device>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare("SELECT id, state_json FROM devices").map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let state_json: String = row.get(1)?;
            Ok((id, state_json))
        }).map_err(|e| e.to_string())?;

        let mut paired_devices = Vec::new();
        for row in rows {
            let (id, state_json) = row.map_err(|e| e.to_string())?;
            let state: DeviceState = serde_json::from_str(&state_json).map_err(|e| e.to_string())?;
            if let DeviceState::Paired(_) = state {
                paired_devices.push(Device { id: DeviceId(id), state });
            }
        }
        Ok(paired_devices)
    }

    async fn save(&self, device: Device) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let state_json = serde_json::to_string(&device.state).map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO devices (id, state_json) VALUES (?1, ?2)",
            [&device.id.0, &state_json],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn update_trust_status(&self, id: DeviceId, status: DeviceState) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        // Check whether the device exists first
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM devices WHERE id = ?1",
            [&id.0],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;

        if count == 0 {
            return Err(format!("Device {} not found", id.0));
        }

        let state_json = serde_json::to_string(&status).map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE devices SET state_json = ?1 WHERE id = ?2",
            [&state_json, &id.0],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }
}
