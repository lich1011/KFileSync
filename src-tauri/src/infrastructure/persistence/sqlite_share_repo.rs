use async_trait::async_trait;
use rusqlite::{Connection, Result as SqlResult, OptionalExtension, Transaction};
use std::sync::Mutex;

use crate::domain::model::device::DeviceId;
use crate::domain::model::share::{
    Share, ShareId, ShareMember, SharePermission, ShareStatus, SyncMode,
};
use crate::domain::port::share_repo::ShareRepository;

pub struct SqliteShareRepository {
    conn: Mutex<Connection>,
}

impl SqliteShareRepository {
    pub fn new(db_path: &str) -> SqlResult<Self> {
        let conn = Connection::open(db_path)?;
        
        // Ensure foreign keys are enabled
        conn.execute("PRAGMA foreign_keys = ON;", [])?;
        
        // Create shares table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS shares (
                share_id        TEXT PRIMARY KEY,
                share_name      TEXT NOT NULL,
                local_path      TEXT NOT NULL,
                sync_mode       TEXT NOT NULL DEFAULT 'two_way',
                status          TEXT NOT NULL DEFAULT 'active',
                created_by      TEXT NOT NULL,
                created_at      INTEGER NOT NULL
            )",
            [],
        )?;
        
        // Create share_members table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS share_members (
                share_id        TEXT NOT NULL,
                device_id       TEXT NOT NULL,
                permission      TEXT NOT NULL DEFAULT 'read_write',
                authorized_by   TEXT NOT NULL,
                authorized_at   INTEGER NOT NULL,
                PRIMARY KEY (share_id, device_id),
                FOREIGN KEY (share_id) REFERENCES shares(share_id) ON DELETE CASCADE
            )",
            [],
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn serialize_sync_mode(mode: &SyncMode) -> &'static str {
        match mode {
            SyncMode::TwoWay => "two_way",
            SyncMode::SendOnly => "send_only",
            SyncMode::ReceiveOnly => "receive_only",
        }
    }

    fn deserialize_sync_mode(s: &str) -> SyncMode {
        match s {
            "send_only" => SyncMode::SendOnly,
            "receive_only" => SyncMode::ReceiveOnly,
            _ => SyncMode::TwoWay,
        }
    }

    fn serialize_status(status: &ShareStatus) -> String {
        match status {
            ShareStatus::Active => "active".to_string(),
            ShareStatus::Paused => "paused".to_string(),
            ShareStatus::Error(e) => format!("error:{}", e),
        }
    }

    fn deserialize_status(s: &str) -> ShareStatus {
        if s == "active" {
            ShareStatus::Active
        } else if s == "paused" {
            ShareStatus::Paused
        } else if let Some(e) = s.strip_prefix("error:") {
            ShareStatus::Error(e.to_string())
        } else {
            ShareStatus::Error("Unknown status".to_string())
        }
    }

    fn serialize_permission(p: &SharePermission) -> &'static str {
        match p {
            SharePermission::ReadOnly => "read_only",
            SharePermission::ReadWrite => "read_write",
            SharePermission::SendOnly => "send_only",
            SharePermission::ReceiveOnly => "receive_only",
        }
    }

    fn deserialize_permission(s: &str) -> SharePermission {
        match s {
            "read_only" => SharePermission::ReadOnly,
            "send_only" => SharePermission::SendOnly,
            "receive_only" => SharePermission::ReceiveOnly,
            _ => SharePermission::ReadWrite,
        }
    }

    fn load_members(tx: &Transaction, share_id: &str) -> SqlResult<Vec<ShareMember>> {
        let mut stmt = tx.prepare(
            "SELECT device_id, permission, authorized_by, authorized_at FROM share_members WHERE share_id = ?1"
        )?;
        
        let members = stmt.query_map([share_id], |row| {
            let device_id: String = row.get(0)?;
            let permission_str: String = row.get(1)?;
            let authorized_by: String = row.get(2)?;
            let authorized_at: u64 = row.get(3)?;
            
            Ok(ShareMember {
                device_id: DeviceId(device_id),
                permission: Self::deserialize_permission(&permission_str),
                authorized_by: DeviceId(authorized_by),
                authorized_at,
            })
        })?.collect::<SqlResult<Vec<_>>>()?;
        
        Ok(members)
    }

    fn row_to_share(tx: &Transaction, row: &rusqlite::Row) -> SqlResult<Share> {
        let share_id: String = row.get(0)?;
        let share_name: String = row.get(1)?;
        let local_path: String = row.get(2)?;
        let sync_mode_str: String = row.get(3)?;
        let status_str: String = row.get(4)?;
        let created_by: String = row.get(5)?;
        let created_at: u64 = row.get(6)?;

        let members = Self::load_members(tx, &share_id)?;

        Ok(Share {
            share_id: ShareId(share_id),
            share_name,
            local_path,
            sync_mode: Self::deserialize_sync_mode(&sync_mode_str),
            status: Self::deserialize_status(&status_str),
            members,
            created_by: DeviceId(created_by),
            created_at,
        })
    }
}

#[async_trait]
impl ShareRepository for SqliteShareRepository {
    async fn save(&self, share: &Share) -> Result<(), String> {
        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        // 1. Insert or Replace the Share
        tx.execute(
            "INSERT OR REPLACE INTO shares 
            (share_id, share_name, local_path, sync_mode, status, created_by, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                share.share_id.0,
                share.share_name,
                share.local_path,
                Self::serialize_sync_mode(&share.sync_mode),
                Self::serialize_status(&share.status),
                share.created_by.0,
                share.created_at,
            ],
        ).map_err(|e| e.to_string())?;

        // 2. Delete existing members for this share
        tx.execute(
            "DELETE FROM share_members WHERE share_id = ?1",
            [&share.share_id.0],
        ).map_err(|e| e.to_string())?;

        // 3. Insert all members
        let mut stmt = tx.prepare(
            "INSERT INTO share_members 
            (share_id, device_id, permission, authorized_by, authorized_at)
            VALUES (?1, ?2, ?3, ?4, ?5)"
        ).map_err(|e| e.to_string())?;

        for member in &share.members {
            stmt.execute(rusqlite::params![
                share.share_id.0,
                member.device_id.0,
                Self::serialize_permission(&member.permission),
                member.authorized_by.0,
                member.authorized_at,
            ]).map_err(|e| e.to_string())?;
        }
        
        drop(stmt);
        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn find_by_id(&self, id: &ShareId) -> Result<Option<Share>, String> {
        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        let mut stmt = tx.prepare(
            "SELECT share_id, share_name, local_path, sync_mode, status, created_by, created_at 
             FROM shares WHERE share_id = ?1"
        ).map_err(|e| e.to_string())?;

        let share = stmt.query_row([&id.0], |row| Self::row_to_share(&tx, row))
            .optional()
            .map_err(|e| e.to_string())?;

        Ok(share)
    }

    async fn find_by_member(&self, device_id: &DeviceId) -> Result<Vec<Share>, String> {
        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        let mut stmt = tx.prepare(
            "SELECT s.share_id, s.share_name, s.local_path, s.sync_mode, s.status, s.created_by, s.created_at 
             FROM shares s
             JOIN share_members sm ON s.share_id = sm.share_id
             WHERE sm.device_id = ?1"
        ).map_err(|e| e.to_string())?;

        let shares = stmt.query_map([&device_id.0], |row| Self::row_to_share(&tx, row))
            .map_err(|e| e.to_string())?
            .collect::<SqlResult<Vec<_>>>()
            .map_err(|e| e.to_string())?;

        Ok(shares)
    }

    async fn find_all(&self) -> Result<Vec<Share>, String> {
        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        let mut stmt = tx.prepare(
            "SELECT share_id, share_name, local_path, sync_mode, status, created_by, created_at 
             FROM shares"
        ).map_err(|e| e.to_string())?;

        let shares = stmt.query_map([], |row| Self::row_to_share(&tx, row))
            .map_err(|e| e.to_string())?
            .collect::<SqlResult<Vec<_>>>()
            .map_err(|e| e.to_string())?;

        Ok(shares)
    }
}
