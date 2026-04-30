use async_trait::async_trait;
use crate::domain::model::file_entry::{FileEntry, SyncConflict};
use crate::domain::model::share::ShareId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalBlockCopy {
    pub hash: String,
    pub source_path: String,
    pub source_offset: u64,
}

#[async_trait]
pub trait FileIndexRepository: Send + Sync {
    async fn save(&self, entry: &FileEntry) -> Result<(), String>;
    
    async fn find_by_path(&self, share_id: &ShareId, path: &str) -> Result<Option<FileEntry>, String>;
    
    async fn find_all_by_share(&self, share_id: &ShareId) -> Result<Vec<FileEntry>, String>;
    
    async fn find_blocks_by_hash(&self, share_id: &ShareId, hash: &str) -> Result<Vec<LocalBlockCopy>, String>;
    
    async fn save_conflict(&self, conflict: &SyncConflict) -> Result<(), String>;
    
    async fn find_conflicts_by_share(&self, share_id: &ShareId) -> Result<Vec<SyncConflict>, String>;
    
    async fn delete_conflict(&self, conflict_id: &str) -> Result<(), String>;
}
