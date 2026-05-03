use async_trait::async_trait;
use rusqlite::{params, OptionalExtension, Row};
use std::sync::{Arc, Mutex};
use crate::domain::error::DomainError;
use crate::domain::model::device::DeviceId;
use crate::domain::model::file_entry::{BlockList, ConflictResolution, EntryType, FileEntry, SyncConflict, VersionVector};
use crate::domain::model::share::ShareId;
use crate::domain::port::file_index_repo::{FileIndexRepository, LocalBlockCopy};

fn db_err(e: impl std::fmt::Display) -> DomainError {
    DomainError::Persistence(e.to_string())
}   

pub struct SqliteFileIndexRepository {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl SqliteFileIndexRepository {
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self { conn }
    }

    fn row_to_file_entry(row: &Row) -> rusqlite::Result<FileEntry> {
        let entry_type_str: String = row.get("entry_type")?;
        let entry_type = match entry_type_str.as_str() {
            "directory" => EntryType::Directory,
            "symlink" => EntryType::Symlink,
            _ => EntryType::File,
        };

        let version_str: String = row.get("version_vector")?;
        let version: VersionVector = serde_json::from_str(&version_str).unwrap_or_default();

        let blocks_str: Option<String> = row.get("blocks")?;
        let blocks = if let Some(s) = blocks_str {
            serde_json::from_str(&s).unwrap_or_default()
        } else {
            BlockList::default()
        };

        let deleted_val: i32 = row.get("deleted")?;

        let modified_by_str: Option<String> = row.get("modified_by")?;
        let modified_by = DeviceId(modified_by_str.unwrap_or_default());

        Ok(FileEntry {
            share_id: ShareId(row.get("share_id")?),
            path: row.get("path")?,
            entry_type,
            size: row.get("size")?,
            modified_at: row.get("modified_at")?,
            modified_by,
            version,
            sha256: row.get("sha256")?,
            blocks,
            deleted: deleted_val == 1,
            deleted_at: row.get("deleted_at")?,
        })
    }

    fn serialize_resolution(resolution: &ConflictResolution) -> String {
        match resolution {
            ConflictResolution::Pending => serde_json::json!({"type": "pending"}).to_string(),
            ConflictResolution::KeepLocal => serde_json::json!({"type": "keep_local"}).to_string(),
            ConflictResolution::KeepRemote => serde_json::json!({"type": "keep_remote"}).to_string(),
            ConflictResolution::KeepBoth { conflict_copy_path } => {
                serde_json::json!({
                    "type": "keep_both", 
                    "conflict_copy_path": conflict_copy_path
                }).to_string()
            }
        }
    }

    fn deserialize_resolution(s: &str) -> ConflictResolution {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
            match v.get("type").and_then(|t| t.as_str()) {
                Some("keep_local") => ConflictResolution::KeepLocal,
                Some("keep_remote") => ConflictResolution::KeepRemote,
                Some("keep_both") => {
                    let path = v.get("conflict_copy_path")
                        .and_then(|p| p.as_str())
                        .unwrap_or("")
                        .to_string();
                    ConflictResolution::KeepBoth { conflict_copy_path: path }
                }
                _ => ConflictResolution::Pending,
            }
        } else {
            match s {
                "keep_local" => ConflictResolution::KeepLocal,
                "keep_remote" => ConflictResolution::KeepRemote,
                _ => ConflictResolution::Pending,
            }
        }
    }
}

#[async_trait]
impl FileIndexRepository for SqliteFileIndexRepository {
    async fn save(&self, entry: &FileEntry) -> Result<(), DomainError> {
        let conn = self.conn.clone();
        let entry = entry.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(db_err)?;

            let entry_type_str = match entry.entry_type {
                EntryType::File => "file",
                EntryType::Directory => "directory",
                EntryType::Symlink => "symlink",
            };
        
            let version_json = serde_json::to_string(&entry.version).map_err(db_err)?;
            
            let blocks_json = serde_json::to_string(&entry.blocks).map_err(db_err)?;

            conn.execute(
                "INSERT INTO file_entries (
                    share_id, path, entry_type, size, modified_at, modified_by,
                    version_vector, sha256, blocks, deleted, deleted_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, strftime('%s', 'now'))
                ON CONFLICT(share_id, path) DO UPDATE SET
                    entry_type = excluded.entry_type,
                    size = excluded.size,
                    modified_at = excluded.modified_at,
                    modified_by = excluded.modified_by,
                    version_vector = excluded.version_vector,
                    sha256 = excluded.sha256,
                    blocks = excluded.blocks,
                    deleted = excluded.deleted,
                    deleted_at = excluded.deleted_at,
                    updated_at = strftime('%s', 'now')",
                params![
                    entry.share_id.0,
                    entry.path,
                    entry_type_str,
                    entry.size,
                    entry.modified_at,
                    entry.modified_by.0,
                    version_json,
                    entry.sha256,
                    blocks_json,
                    if entry.deleted { 1 } else { 0 },
                    entry.deleted_at,
                ],
            ).map_err(db_err)?;

            Ok(())
        }).await.map_err(db_err)?
    }

    async fn find_by_path(&self, share_id: &ShareId, path: &str) -> Result<Option<FileEntry>, DomainError> {
        let conn = self.conn.clone();
        let share_id = share_id.clone();
        let path = path.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(db_err)?;
            let mut stmt = conn.prepare("SELECT * FROM file_entries WHERE share_id = ?1 AND path = ?2")
                .map_err(db_err)?;
            
            let result = stmt.query_row(params![share_id.0, path], Self::row_to_file_entry)
                .optional()
                .map_err(db_err)?;

            Ok(result)
        }).await.map_err(db_err)?
    }

    async fn find_all_by_share(&self, share_id: &ShareId) -> Result<Vec<FileEntry>, DomainError> {
        let conn = self.conn.clone();
        let share_id = share_id.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(db_err)?;
            let mut stmt = conn.prepare("SELECT * FROM file_entries WHERE share_id = ?1")
                .map_err(db_err)?;  
        
            let mut rows = stmt.query(params![share_id.0])
                .map_err(db_err)?;

            let mut entries = Vec::new();
            while let Some(row) = rows.next().map_err(db_err)? {
                entries.push(Self::row_to_file_entry(row).map_err(db_err)?);
            }

            Ok(entries)
        }).await.map_err(db_err)?
    }

    async fn find_blocks_by_hash(&self, share_id: &ShareId, hash: &str) -> Result<Vec<LocalBlockCopy>, DomainError> {
        let conn = self.conn.clone();
        let share_id = share_id.clone();
        let hash = hash.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(db_err)?;
            
        // This is a slow operation in SQLite unless we create a virtual table or extract blocks into a separate table.
        // For MVP, since `blocks` is stored as JSON, doing a LIKE query is a hack but works for deduplication fallback.
        // A better approach would be normalizing `blocks` into a `file_blocks` table, but we will follow the provided schema.
        
            let like_query = format!("%\"hash\":\"{}\"%", hash);
            let mut stmt = conn.prepare("SELECT path, blocks FROM file_entries WHERE share_id = ?1 AND blocks LIKE ?2 AND deleted = 0")
                .map_err(db_err)?;

            let mut rows = stmt.query(params![share_id.0, like_query])
                .map_err(db_err)?;

            let mut copies = Vec::new();
            while let Some(row) = rows.next().map_err(db_err)? {
                let path: String = row.get(0).unwrap_or_default();
                let blocks_str: String = row.get(1).unwrap_or_default();
                
                if let Ok(block_list) = serde_json::from_str::<BlockList>(&blocks_str) {
                    let mut current_offset: u64 = 0;
                    for b in block_list.0 {
                        if b.hash == hash {
                            copies.push(LocalBlockCopy {
                                hash: hash.to_string(),
                                source_path: path.clone(),
                                source_offset: current_offset,
                            });
                    }
                    current_offset += b.size as u64;
                }
            }
        }

        Ok(copies)
        }).await.map_err(db_err)?
    }

    async fn save_conflict(&self, conflict: &SyncConflict) -> Result<(), DomainError> {
        let conn = self.conn.clone();
        let conflict_path = conflict.path.clone();
        let local_json = serde_json::to_string(&conflict.local).map_err(db_err)?;
        let remote_json = serde_json::to_string(&conflict.remote).map_err(db_err)?;
        let resolution_str = Self::serialize_resolution(&conflict.resolution);
        let share_id = conflict.local.share_id.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(db_err)?;
            let conflict_id = uuid::Uuid::new_v4().to_string();

            conn.execute(
                "INSERT INTO sync_conflicts (
                    conflict_id, share_id, file_path, local_entry, remote_entry, resolution
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    conflict_id,
                    share_id.0,
                    conflict_path,
                    local_json,
                    remote_json,
                    resolution_str
                ],
            ).map_err(db_err)?;

            Ok(())
        }).await.map_err(db_err)?
    }

    async fn find_conflicts_by_share(&self, share_id: &ShareId) -> Result<Vec<SyncConflict>, DomainError> {
        let conn=self.conn.clone();
        let share_id=share_id.clone();
        
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(db_err)?;
            let mut stmt = conn.prepare("SELECT * FROM sync_conflicts WHERE share_id = ?1")
                .map_err(db_err)?;
            
            let mut rows = stmt.query(params![share_id.0]).map_err(db_err)?;

            let mut conflicts = Vec::new();
            while let Some(row) = rows.next().map_err(db_err)? {
                let local_str: String = row.get("local_entry").unwrap_or_default();
                let remote_str: String = row.get("remote_entry").unwrap_or_default();
                
                let local: FileEntry = serde_json::from_str(&local_str).map_err(db_err)?;
                let remote: FileEntry = serde_json::from_str(&remote_str).map_err(db_err)?;
            
                let resolution_str: String = row.get("resolution").unwrap_or_default();
                let resolution = Self::deserialize_resolution(&resolution_str);

                conflicts.push(SyncConflict {
                    path: row.get("file_path").unwrap_or_default(),
                    local,
                    remote,
                    resolution,
                });
            }

            Ok(conflicts)
        }).await.map_err(db_err)?   
    }

    async fn delete_conflict(&self, conflict_id: &str) -> Result<(), DomainError> {
        let conn=self.conn.clone(); 
        let conflict_id=conflict_id.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(db_err)?;
            conn.execute("DELETE FROM sync_conflicts WHERE conflict_id = ?1", params![conflict_id])
                .map_err(db_err)?;
            Ok(())
        }).await.map_err(db_err)?
    }
}

impl SqliteFileIndexRepository {
    pub async fn cleanup_expired_tombstones(&self, before_timestamp: i64) -> Result<usize, DomainError> {
        let conn=self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(db_err)?;
            let mut stmt = conn.prepare("DELETE FROM file_index WHERE status = 'tombstone' AND last_modified < ?1").map_err(db_err)?;
            
            stmt.execute([&before_timestamp]).map_err(db_err)
        }).await.map_err(db_err)?
    }
}
