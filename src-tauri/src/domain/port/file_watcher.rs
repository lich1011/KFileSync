use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc::Sender;
use crate::domain::error::DomainError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileEventType {
    Created,
    Modified,
    Deleted,
    Renamed { from: PathBuf },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEvent {
    pub path: PathBuf,
    pub event_type: FileEventType,
    pub timestamp: u64,
}

pub type WatchHandle = String;

#[async_trait]
pub trait FileWatcher: Send + Sync {
    /// Start watching a directory, sending events to the provided channel.
    /// Debouncing is handled inside the implementation.
    async fn watch(&self, path: &Path, tx: Sender<FileEvent>) -> Result<WatchHandle, DomainError>;
    
    /// Stop watching based on handle
    async fn unwatch(&self, handle: WatchHandle) -> Result<(), DomainError>;
}
