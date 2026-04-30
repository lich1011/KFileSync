use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::path::Path;
use tokio::sync::mpsc::{channel, Receiver};
use crate::domain::model::device::DeviceId;
use crate::domain::model::file_entry::{BlockList, EntryType, FileEntry};
use crate::domain::model::share::ShareId;
use crate::domain::port::file_index_repo::FileIndexRepository;
use crate::domain::port::file_watcher::{FileEvent, FileEventType, FileWatcher, WatchHandle};
use crate::domain::port::share_repo::ShareRepository;

pub struct IndexerService {
    file_index_repo: Arc<dyn FileIndexRepository>,
    share_repo: Arc<dyn ShareRepository>,
    file_watcher: Arc<dyn FileWatcher>,
    local_device_id: DeviceId,
    /// Tracks active watch handles to prevent duplicate registrations and enable cleanup.
    active_watchers: Mutex<HashMap<ShareId, WatchHandle>>,
}

impl IndexerService {
    pub fn new(
        file_index_repo: Arc<dyn FileIndexRepository>,
        share_repo: Arc<dyn ShareRepository>,
        file_watcher: Arc<dyn FileWatcher>,
        local_device_id: DeviceId,
    ) -> Self {
        Self {
            file_index_repo,
            share_repo,
            file_watcher,
            local_device_id,
            active_watchers: Mutex::new(HashMap::new()),
        }
    }

    /// Starts watching a share directory and processing its events.
    /// If already watching, returns Ok immediately (idempotent).
    pub async fn start_watching(&self, share_id: &ShareId) -> Result<(), String> {
        // Guard: prevent duplicate watchers for the same share
        {
            let watchers = self.active_watchers.lock().unwrap();
            if watchers.contains_key(share_id) {
                return Ok(()); // Already watching
            }
        }

        let share = self.share_repo.find_by_id(share_id).await?.ok_or("Share not found")?;
        
        let (tx, mut rx) = channel::<FileEvent>(100);
        
        // Use share.local_path (now a String) correctly as a Path reference
        let watch_handle = self.file_watcher.watch(Path::new(&share.local_path), tx).await?;

        // Register the handle
        self.active_watchers.lock().unwrap().insert(share_id.clone(), watch_handle);

        // Spawn background task to process events
        let file_index_repo = self.file_index_repo.clone();
        let local_device_id = self.local_device_id.clone();
        let share_id = share_id.clone();
        let share_root = std::path::PathBuf::from(&share.local_path);
        
        tokio::spawn(async move {
            Self::process_events(&share_id, &local_device_id, file_index_repo, &mut rx, &share_root).await;
        });

        Ok(())
    }

    /// Stops watching a share directory.
    #[allow(dead_code)]
    pub async fn stop_watching(&self, share_id: &ShareId) -> Result<(), String> {
        let handle = {
            let mut watchers = self.active_watchers.lock().unwrap();
            watchers.remove(share_id)
        };

        if let Some(h) = handle {
            self.file_watcher.unwatch(h).await?;
        }

        Ok(())
    }

    async fn process_events(
        share_id: &ShareId,
        local_device_id: &DeviceId,
        file_index_repo: Arc<dyn FileIndexRepository>,
        rx: &mut Receiver<FileEvent>,
        share_root: &std::path::Path,
    ) {
        while let Some(event) = rx.recv().await {
            // Convert absolute path from notify to share-relative path
            let path_str = event.path
                .strip_prefix(share_root)
                .unwrap_or(&event.path)
                .to_string_lossy()
                .to_string();
            
            // Get current entry or create new
            let existing_entry = file_index_repo.find_by_path(share_id, &path_str).await.unwrap_or(None);

            match event.event_type {
                FileEventType::Created | FileEventType::Modified => {
                    // Re-calculate size and hash (mocked for MVP flow setup)
                    let entry = if let Some(mut e) = existing_entry {
                        e.size = 1024; // Mock size
                        e.sha256 = Some("mock_hash".to_string());
                        e.blocks = BlockList::default();
                        e.modified_at = event.timestamp;
                        e.modified_by = local_device_id.clone();
                        e.version = e.version.increment(local_device_id);
                        e
                    } else {
                        FileEntry::new(share_id.clone(), path_str.clone(), EntryType::File, local_device_id)
                    };
                    
                    if let Err(e) = file_index_repo.save(&entry).await {
                        eprintln!("[IndexerService] Failed to save file entry for '{}': {}", path_str, e);
                    }
                }
                FileEventType::Deleted => {
                    if let Some(e) = existing_entry {
                        let deleted_entry = e.mark_deleted(local_device_id);
                        if let Err(e) = file_index_repo.save(&deleted_entry).await {
                            eprintln!("[IndexerService] Failed to save deletion for '{}': {}", path_str, e);
                        }
                    }
                }
                FileEventType::Renamed { .. } => {
                    // MVP simplification: treated as modify of new location
                }
            }
        }
    }
}
