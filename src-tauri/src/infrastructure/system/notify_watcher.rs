use async_trait::async_trait;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use tokio::sync::mpsc::Sender;
use crate::domain::error::DomainError;
use crate::domain::port::file_watcher::{FileEvent, FileEventType, FileWatcher, WatchHandle};

fn fs_err(e: impl std::fmt::Display) -> DomainError{
    DomainError::FileSystem(e.to_string())
}

pub struct NotifyWatcherAdapter {
    watchers: Arc<Mutex<HashMap<WatchHandle, RecommendedWatcher>>>,
}

impl NotifyWatcherAdapter {
    pub fn new() -> Self {
        Self {
            watchers: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for NotifyWatcherAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FileWatcher for NotifyWatcherAdapter {
    async fn watch(&self, path: &Path, tx: Sender<FileEvent>) -> Result<WatchHandle, DomainError> {
        let handle = uuid::Uuid::new_v4().to_string();
        
        let tx_clone = tx.clone();
        
        // Setup the notify watcher
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                // Map notify events to our domain events
                let event_type = match event.kind {
                    EventKind::Create(_) => Some(FileEventType::Created),
                    EventKind::Modify(_) => Some(FileEventType::Modified),
                    EventKind::Remove(_) => Some(FileEventType::Deleted),
                    // For renames, notify usually sends two events or a single Modify/Rename.
                    // We simplify it to Modified for MVP, or extract paths if it's a rename.
                    _ => None,
                };

                if let Some(et) = event_type {
                    for path in event.paths {
                        // Ignore tmp and sync metadata files
                        if path.to_string_lossy().contains(".lansync-tmp") || path.to_string_lossy().contains(".sync-conflict") {
                            continue;
                        }

                        let domain_event = FileEvent {
                            path,
                            event_type: et.clone(),
                            timestamp,
                        };

                        // Send event to channel (ignore error if receiver is dropped)
                        if tx_clone.blocking_send(domain_event).is_err(){
                            break;
                        };
                    }
                }
            }
        }).map_err(fs_err)?;

        watcher.watch(path, RecursiveMode::Recursive)
            .map_err(fs_err)?;

        // Store the watcher so it isn't dropped
        self.watchers.lock().unwrap().insert(handle.clone(), watcher);

        Ok(handle)
    }

    async fn unwatch(&self, handle: WatchHandle) -> Result<(), DomainError> {
        let mut watchers = self.watchers.lock().unwrap();
        if watchers.remove(&handle).is_some() {
            Ok(())
        } else {
            Err(DomainError::NotFound(format!("Watcher handle {} not found", handle)))
        }
    }
}
