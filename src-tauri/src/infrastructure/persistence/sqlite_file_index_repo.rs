use async_trait::async_trait;
use rusqlite::{params, OptionalExtension, Row};
use std::sync::{Arc, Mutex};
use crate::domain::model::device::DeviceId;
use crate::domain::model::file_entry::{BlockList, ConflictResolution, EntryType, FileEntry, SyncConflict, VersionVector};
use crate::domain::model::share::ShareId;
use crate::domain::port::file_index_repo::{FileIndexRepository, LocalBlockCopy};

pub struct SqliteFileIndexRepository {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl SqliteFileIndexRepository {
    pub fn new(db_path: &str) -> Result<Self, String> {
        let conn = rusqlite::Connection::open(db_path).map_err(|e| e.to_string())?;
        
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;"
        ).map_err(|e| e.to_string())?;

        let conn = Arc::new(Mutex::new(conn));

        // Initialize tables if they don't exist
        {
            let c = conn.lock().unwrap();
            
            c.execute(
                "CREATE TABLE IF NOT EXISTS file_entries (
                    share_id        TEXT NOT NULL,
                    path            TEXT NOT NULL,
                    entry_type      TEXT NOT NULL DEFAULT 'file',
                    size            INTEGER NOT NULL DEFAULT 0,
                    modified_at     INTEGER,
                    modified_by     TEXT,
                    version_vector  TEXT NOT NULL DEFAULT '{}',
                    sha256          TEXT,
                    blocks          TEXT,
                    deleted         INTEGER NOT NULL DEFAULT 0,
                    deleted_at      INTEGER,
                    updated_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                    PRIMARY KEY (share_id, path)
                )",
                [],
            ).unwrap();

            c.execute(
                "CREATE TABLE IF NOT EXISTS sync_conflicts (
                    conflict_id     TEXT PRIMARY KEY,
                    share_id        TEXT NOT NULL,
                    file_path       TEXT NOT NULL,
                    local_entry     TEXT NOT NULL,
                    remote_entry    TEXT NOT NULL,
                    resolution      TEXT DEFAULT 'pending',
                    created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                    resolved_at     INTEGER
                )",
                [],
            ).unwrap();
        }

        Ok(Self { conn })
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
}

#[async_trait]
impl FileIndexRepository for SqliteFileIndexRepository {
    async fn save(&self, entry: &FileEntry) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        
        let entry_type_str = match entry.entry_type {
            EntryType::File => "file",
            EntryType::Directory => "directory",
            EntryType::Symlink => "symlink",
        };
        
        let version_json = serde_json::to_string(&entry.version)
            .map_err(|e| format!("Failed to serialize version: {}", e))?;
            
        let blocks_json = serde_json::to_string(&entry.blocks)
            .map_err(|e| format!("Failed to serialize blocks: {}", e))?;

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
        ).map_err(|e| format!("DB Error saving file entry: {}", e))?;

        Ok(())
    }

    async fn find_by_path(&self, share_id: &ShareId, path: &str) -> Result<Option<FileEntry>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM file_entries WHERE share_id = ?1 AND path = ?2")
            .map_err(|e| e.to_string())?;
        
        let result = stmt.query_row(params![share_id.0, path], Self::row_to_file_entry)
            .optional()
            .map_err(|e| e.to_string())?;

        Ok(result)
    }

    async fn find_all_by_share(&self, share_id: &ShareId) -> Result<Vec<FileEntry>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM file_entries WHERE share_id = ?1")
            .map_err(|e| e.to_string())?;
        
        let mut rows = stmt.query(params![share_id.0])
            .map_err(|e| e.to_string())?;

        let mut entries = Vec::new();
        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            entries.push(Self::row_to_file_entry(row).map_err(|e| e.to_string())?);
        }

        Ok(entries)
    }

    async fn find_blocks_by_hash(&self, share_id: &ShareId, hash: &str) -> Result<Vec<LocalBlockCopy>, String> {
        let conn = self.conn.lock().unwrap();
        
        // This is a slow operation in SQLite unless we create a virtual table or extract blocks into a separate table.
        // For MVP, since `blocks` is stored as JSON, doing a LIKE query is a hack but works for deduplication fallback.
        // A better approach would be normalizing `blocks` into a `file_blocks` table, but we will follow the provided schema.
        
        let like_query = format!("%\"hash\":\"{}\"%", hash);
        let mut stmt = conn.prepare("SELECT path, blocks FROM file_entries WHERE share_id = ?1 AND blocks LIKE ?2 AND deleted = 0")
            .map_err(|e| e.to_string())?;

        let mut rows = stmt.query(params![share_id.0, like_query])
            .map_err(|e| e.to_string())?;

        let mut copies = Vec::new();
        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
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
    }

    async fn save_conflict(&self, conflict: &SyncConflict) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        
        let local_json = serde_json::to_string(&conflict.local).map_err(|e| e.to_string())?;
        let remote_json = serde_json::to_string(&conflict.remote).map_err(|e| e.to_string())?;
        
        let resolution_str = match &conflict.resolution {
            ConflictResolution::Pending => "pending",
            ConflictResolution::KeepLocal => "keep_local",
            ConflictResolution::KeepRemote => "keep_remote",
            ConflictResolution::KeepBoth { .. } => "keep_both",
        };

        let conflict_id = uuid::Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO sync_conflicts (
                conflict_id, share_id, file_path, local_entry, remote_entry, resolution
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                conflict_id,
                conflict.local.share_id.0,
                conflict.path,
                local_json,
                remote_json,
                resolution_str
            ],
        ).map_err(|e| format!("DB Error saving conflict: {}", e))?;

        Ok(())
    }

    async fn find_conflicts_by_share(&self, share_id: &ShareId) -> Result<Vec<SyncConflict>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM sync_conflicts WHERE share_id = ?1")
            .map_err(|e| e.to_string())?;
        
        let mut rows = stmt.query(params![share_id.0]).map_err(|e| e.to_string())?;

        let mut conflicts = Vec::new();
        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            let local_str: String = row.get("local_entry").unwrap_or_default();
            let remote_str: String = row.get("remote_entry").unwrap_or_default();
            
            let local: FileEntry = serde_json::from_str(&local_str).map_err(|e| e.to_string())?;
            let remote: FileEntry = serde_json::from_str(&remote_str).map_err(|e| e.to_string())?;
            
            let resolution_str: String = row.get("resolution").unwrap_or_default();
            let resolution = match resolution_str.as_str() {
                "keep_local" => ConflictResolution::KeepLocal,
                "keep_remote" => ConflictResolution::KeepRemote,
                "keep_both" => ConflictResolution::KeepBoth { conflict_copy_path: "".to_string() }, // Simplification
                _ => ConflictResolution::Pending,
            };

            conflicts.push(SyncConflict {
                path: row.get("file_path").unwrap_or_default(),
                local,
                remote,
                resolution,
            });
        }

        Ok(conflicts)
    }

    async fn delete_conflict(&self, conflict_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM sync_conflicts WHERE conflict_id = ?1", params![conflict_id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
