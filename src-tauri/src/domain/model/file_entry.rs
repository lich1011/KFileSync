use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use crate::domain::model::device::DeviceId;
use crate::domain::model::share::ShareId;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Eq, PartialEq, Hash, Debug, Default, Serialize, Deserialize)]
pub struct VersionVector(pub BTreeMap<DeviceId, u64>);

impl VersionVector {
    pub fn new() -> Self {
        VersionVector(BTreeMap::new())
    }

    pub fn is_ancestor_of(&self, other: &Self) -> bool {
        for (device_id, &self_version) in &self.0 {
            let other_version = other.0.get(device_id).copied().unwrap_or(0);
            if self_version > other_version {
                return false;
            }
        }
        true
    }
    
    pub fn conflicts_with(&self, other: &Self) -> bool {
        !self.is_ancestor_of(other) && !other.is_ancestor_of(self)
    }
    
    pub fn increment(&self, device: &DeviceId) -> Self {
        let mut new_vec = self.0.clone();
        let count = new_vec.entry(device.clone()).or_insert(0);
        *count += 1;
        VersionVector(new_vec)
    }
    
    pub fn merge(&self, other: &Self) -> Self {
        let mut result = self.0.clone();
        for (device_id, &other_version) in &other.0 {
            let self_version = result.entry(device_id.clone()).or_insert(0);
            if other_version > *self_version {
                *self_version = other_version;
            }
        }
        VersionVector(result)
    }
}

// ---------------------------------------------------------
// Sync Domain Models
// ---------------------------------------------------------

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum EntryType {
    File,
    Directory,
    Symlink,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct BlockInfo {
    pub index: u32,
    pub size: u32,
    pub hash: String, // BLAKE3
}

#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct BlockList(pub Vec<BlockInfo>);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEntry {
    pub share_id: ShareId,
    pub path: String,                // Relative to share root
    pub entry_type: EntryType,
    pub size: u64,
    pub modified_at: u64,            // Timestamp
    pub modified_by: DeviceId,
    pub version: VersionVector,
    pub sha256: Option<String>,
    pub blocks: BlockList,
    pub deleted: bool,
    pub deleted_at: Option<u64>,
}

impl FileEntry {
    pub fn new(share_id: ShareId, path: String, entry_type: EntryType, device_id: &DeviceId) -> Self {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        Self {
            share_id,
            path,
            entry_type,
            size: 0,
            modified_at: now,
            modified_by: device_id.clone(),
            version: VersionVector::new().increment(device_id),
            sha256: None,
            blocks: BlockList(Vec::new()),
            deleted: false,
            deleted_at: None,
        }
    }

    pub fn update_content(mut self, size: u64, sha256: String, blocks: BlockList, device_id: &DeviceId) -> Self {
        self.size = size;
        self.sha256 = Some(sha256);
        self.blocks = blocks;
        self.modified_by = device_id.clone();
        self.modified_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        self.version = self.version.increment(device_id);
        self
    }

    pub fn mark_deleted(mut self, device_id: &DeviceId) -> Self {
        self.deleted = true;
        self.deleted_at = Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs());
        self.modified_by = device_id.clone();
        self.version = self.version.increment(device_id);
        self
    }

    pub fn apply_remote_version(mut self, remote: &FileEntry) -> Self {
        self.version = self.version.merge(&remote.version);
        self
    }
}

// ---------------------------------------------------------
// Sync Session & Orchestration
// ---------------------------------------------------------

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum SyncStatus {
    Idle,
    Syncing,
    HasConflicts,
    Paused,
}

#[derive(Clone, Debug)]
pub struct SyncSession {
    pub session_id: String,
    pub share_id: ShareId,
    pub peer_device_id: DeviceId,
    pub status: SyncStatus,
    pub plan: Option<SyncPlan>,
    pub started_at: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct SyncAction {
    pub path: String,
    pub entry: FileEntry,
    pub missing_blocks: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConflictResolution {
    Pending,
    KeepLocal,
    KeepRemote,
    KeepBoth { conflict_copy_path: String },
}

#[derive(Clone, Debug)]
pub struct SyncConflict {
    pub path: String,
    pub local: FileEntry,
    pub remote: FileEntry,
    pub resolution: ConflictResolution,
}

#[derive(Clone, Debug)]
pub struct SyncPlan {
    pub to_pull: Vec<SyncAction>,
    pub to_push: Vec<SyncAction>,
    pub conflicts: Vec<SyncConflict>,
    pub unchanged: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Tombstone {
    pub share_id: ShareId,
    pub path: String,
    pub deleted_at: u64,
    pub version: VersionVector,
}
