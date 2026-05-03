use async_trait::async_trait;
use rusqlite::{Connection, Result as SqlResult, OptionalExtension};
use std::sync::{Arc, Mutex};

use crate::domain::model::device::DeviceId;
use crate::domain::model::share::{
    Share, ShareId, ShareMember, SharePermission, ShareStatus, SyncMode,
};
use crate::domain::port::share_repo::ShareRepository;
use crate::domain::error::DomainError;

fn db_err(e: impl std::fmt::Display) -> DomainError {
    DomainError::Persistence(e.to_string())
}   

pub struct SqliteShareRepository {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteShareRepository {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
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

    fn load_members_sync(conn: &Connection, share_id: &str) -> Result<Vec<ShareMember>, DomainError> {
        let mut stmt = conn.prepare(
            "SELECT device_id, permission, authorized_by, authorized_at FROM share_members WHERE share_id = ?1"
        ).map_err(db_err)?;
        
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
        }).map_err(db_err)? 
        .collect::<SqlResult<Vec<_>>>()
        .map_err(db_err)?;
        
        Ok(members)
    }

    fn assemble_share(
        conn: &Connection, 
        share_id: String,
        share_name: String,
        local_path: String,
        sync_mode_str: String,
        status_str: String,
        created_by: String,
        created_at: u64,
    ) -> Result<Share, DomainError> {
        let members = Self::load_members_sync(conn, &share_id)?;

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
    async fn save(&self, share: &Share) -> Result<(), DomainError> {
        let conn = self.conn.clone();
        let share = share.clone();

        tokio::task::spawn_blocking(move || {
            let mut conn = conn.lock().map_err(db_err)?;
            let tx = conn.transaction().map_err(db_err)?;

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
            ).map_err(db_err)?;

            // 2. Delete existing members for this share
            tx.execute(
                "DELETE FROM share_members WHERE share_id = ?1",
                [&share.share_id.0],
            ).map_err(db_err)?;

            // 3. Insert all members
            let mut stmt = tx.prepare(
                "INSERT INTO share_members 
                (share_id, device_id, permission, authorized_by, authorized_at)
                VALUES (?1, ?2, ?3, ?4, ?5)",
            ).map_err(db_err)?;

            for member in &share.members {
                stmt.execute(rusqlite::params![
                    share.share_id.0,
                    member.device_id.0,
                    Self::serialize_permission(&member.permission),
                    member.authorized_by.0,
                    member.authorized_at,
                ]).map_err(db_err)?;
            }
            
            drop(stmt);
            tx.commit().map_err(db_err)?;
            Ok(())
        }).await.map_err(db_err)?
    }

    async fn find_by_id(&self, id: &ShareId) -> Result<Option<Share>, DomainError> {
        let conn = self.conn.clone();
        let id = id.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(db_err)?;

            let mut stmt = conn.prepare(
                "SELECT share_id, share_name, local_path, sync_mode, status, created_by, created_at 
                 FROM shares WHERE share_id = ?1"
            ).map_err(db_err)?;

            let row_opt = stmt.query_row([&id.0], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, u64>(6)?,
                ))
            }).optional()
            .map_err(db_err)?;

            match row_opt {
                Some((id, name, path, mode, status, created_by, created_at)) => {
                    let share = Self::assemble_share(&conn, id, name, path, mode, status, created_by, created_at)?;
                    Ok(Some(share))
                }
                None => Ok(None),
            }
        }).await.map_err(db_err)?
    }

    async fn find_by_member(&self, device_id: &DeviceId) -> Result<Vec<Share>, DomainError> {
        let conn = self.conn.clone();
        let device_id = device_id.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(db_err)?;

            let mut stmt = conn.prepare(
                "SELECT share_id, share_name, local_path, sync_mode, status, created_by, created_at 
                 FROM shares s 
                 JOIN share_members m ON s.share_id = m.share_id 
                 WHERE m.device_id = ?1"
            ).map_err(db_err)?;

            let rows = stmt.query_map([&device_id.0], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, u64>(6)?,
                ))
            })
            .map_err(db_err)?;

            let mut shares = Vec::new();
            for row in rows {
                let (id, name, path, mode, status, created_by, created_at) = row.map_err(db_err)?;
                let share = Self::assemble_share(&conn, id, name, path, mode, status, created_by, created_at)?;
                shares.push(share);
            }

            Ok(shares)
        }).await.map_err(db_err)?
    }

    async fn find_all(&self) -> Result<Vec<Share>, DomainError> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(db_err)?;

            let mut stmt = conn.prepare(
                "SELECT share_id, share_name, local_path, sync_mode, status, created_by, created_at 
                 FROM shares"
            ).map_err(db_err)?;

            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, u64>(6)?,
                ))
            })
            .map_err(db_err)?;

            let mut shares = Vec::new();
            for row in rows {
                let (id, name, path, mode, status, created_by, created_at) = row.map_err(db_err)?;
                let share = Self::assemble_share(&conn, id, name, path, mode, status, created_by, created_at)?;
                shares.push(share);
            }

            Ok(shares)
        }).await.map_err(db_err)?
    }
}
