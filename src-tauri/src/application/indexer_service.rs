use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::path::Path;
use tokio::sync::mpsc::{channel, Receiver};
use crate::domain::model::device::DeviceId;
use crate::domain::model::file_entry::{BlockInfo, BlockList, EntryType, FileEntry};
use crate::domain::model::share::ShareId;
use crate::domain::service::specification::{IgnoreSpec, IgnoreContext, Specification};
use crate::infrastructure::security::chunk_hasher::ChunkHasher;
use crate::domain::service::chunking::ChunkingStrategy;
use crate::domain::error::DomainError;
use crate::domain::port::file_index_repo::FileIndexRepository;
use crate::domain::port::file_watcher::{FileEvent, FileEventType, FileWatcher, WatchHandle};
use crate::domain::port::share_repo::ShareRepository;

pub struct IndexerService {
    file_index_repo: Arc<dyn FileIndexRepository>,
    share_repo: Arc<dyn ShareRepository>,
    file_watcher: Arc<dyn FileWatcher>,
    local_device_id: DeviceId,
    /// Tracks active watch handles to prevent duplicate registrations and enable cleanup.
    chunking: Arc<dyn ChunkingStrategy>,
    active_watchers: Mutex<HashMap<ShareId, WatchHandle>>,
}

impl IndexerService {
    pub fn new(
        file_index_repo: Arc<dyn FileIndexRepository>,
        share_repo: Arc<dyn ShareRepository>,
        file_watcher: Arc<dyn FileWatcher>,
        local_device_id: DeviceId,
        chunking: Arc<dyn ChunkingStrategy>,
    ) -> Self {
        Self {
            file_index_repo,
            share_repo,
            file_watcher,
            local_device_id,    
            chunking,
            active_watchers: Mutex::new(HashMap::new()),
        }
    }

    /// Starts watching a share directory and processing its events.
    /// If already watching, returns Ok immediately (idempotent).
    pub async fn start_watching(&self, share_id: &ShareId) -> Result<(), DomainError> {
        // Guard: prevent duplicate watchers for the same share
        {
            let watchers = self.active_watchers.lock().unwrap();
            if watchers.contains_key(share_id) {
                return Ok(()); // Already watching
            }
        }

        let share = self.share_repo.find_by_id(share_id).await?
            .ok_or_else(|| DomainError::ShareNotFound(share_id.0.clone()))?;
        
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
        let chunking = self.chunking.clone();
        
        tokio::spawn(async move {
            Self::process_events(&share_id, &local_device_id, file_index_repo, &mut rx, &share_root, chunking).await;
        });

        Ok(())
    }

    /// Stops watching a share directory.
    #[allow(dead_code)]
    pub async fn stop_watching(&self, share_id: &ShareId) -> Result<(), DomainError> {
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
        chunking: Arc<dyn ChunkingStrategy>,
    ) {
        let sync_ignore_path = share_root.join(".syncignore");
        
        let mut ignore_space = IgnoreSpec::from_file(&sync_ignore_path)
            .unwrap_or_else(|_| IgnoreSpec::new(share_root, &[]).expect("default ignore space"));
        
        while let Some(event) = rx.recv().await {
            // Convert absolute path from notify to share-relative path
            let relative_path = event.path
                .strip_prefix(share_root)
                .unwrap_or(&event.path);
                
            let path_str = relative_path.to_string_lossy().to_string();

            if path_str == ".syncignore" {
                ignore_space = IgnoreSpec::from_file(&sync_ignore_path)
                    .unwrap_or_else(|_| IgnoreSpec::new(share_root, &[]).expect("default ignore space"));
                continue;
            }

            let is_dir = event.path.is_dir();
            let ignore_context = IgnoreContext { path: relative_path, is_dir };
            if ignore_space.is_satisfied_by(&ignore_context){
                continue;
            }
            
            // Get current entry or create new
            let existing_entry = file_index_repo.find_by_path(share_id, &path_str).await.unwrap_or(None);

            match event.event_type {
                FileEventType::Created | FileEventType::Modified => {
                    let abs_path = event.path.clone();
                    let chunking_ref = chunking.clone();
                    let device_id= local_device_id.clone();

                    let computed = tokio::task::spawn_blocking(move || -> Result<(u64, String, Vec<crate::domain::model::transfer::ChunkInfo>), DomainError> {
                        let meta = std::fs::metadata(&abs_path)
                            .map_err(|e| DomainError::FileSystem(e.to_string()))?;
                        let size = meta.len();
                        let sha256 = ChunkHasher::compute_sha256(&abs_path)?;
                        let chunk_size=chunking_ref.compute_chunk_size(size);
                        let blocks=ChunkHasher::hash_file_chunks(&abs_path, chunk_size, )?;
                        Ok((size, sha256, blocks))
                    }).await.unwrap_or_else(|e| Err(DomainError::FileSystem(e.to_string())));

                    match computed {
                        Ok((size, sha256, chunks)) => {
                            let blocks = BlockList(chunks.into_iter().map(|c| BlockInfo{
                                index: c.index,
                                size: c.size,
                                hash: c.hash,
                            }).collect());

                            // Update existing entry or create new one
                            let entry = if let Some(mut e) = existing_entry {
                                e.size = size;
                                e.sha256 = Some(sha256);
                                e.blocks = blocks;
                                e.modified_at = event.timestamp;
                                e.modified_by = device_id.clone();
                                e.version = e.version.increment(&device_id);
                                e
                            } else {
                                let mut new_entry = FileEntry::new(
                                    share_id.clone(), path_str.clone(), EntryType::File, &device_id
                                );
                                new_entry.size = size;
                                new_entry.sha256 = Some(sha256);
                                new_entry.blocks = blocks;
                                new_entry
                            };

                            if let Err(e) = file_index_repo.save(&entry).await {
                                eprintln!("[IndexerService] Failed to save file entry for '{}': {}", path_str, e);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to process file {}: {}", path_str, e);
                        }
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
