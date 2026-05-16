use crate::domain::model::device::DeviceId;
use crate::domain::model::share::ShareId;
use crate::domain::model::file_entry::VersionVector;
use crate::domain::port::event_bus::DomainEvent;

#[derive(Debug, Clone)]
pub struct SyncStarted {
    pub share_id: ShareId,
    pub peer_device: DeviceId,
}

impl DomainEvent for SyncStarted {
    fn event_type(&self) -> &str { "SyncStarted" }
    fn aggregate_id(&self) -> &str { &self.share_id.0 }
}

#[derive(Debug, Clone)]
pub struct SyncCompleted {
    pub share_id: ShareId,
    pub files_synced: u32,
}

impl DomainEvent for SyncCompleted {
    fn event_type(&self) -> &str { "SyncCompleted" }
    fn aggregate_id(&self) -> &str { &self.share_id.0 }
}

#[derive(Debug, Clone)]
pub struct ConflictDetected {
    pub share_id: ShareId,
    pub path: String,
    pub local_version: VersionVector,
    pub remote_version: VersionVector,
}

impl DomainEvent for ConflictDetected {
    fn event_type(&self) -> &str { "ConflictDetected" }
    fn aggregate_id(&self) -> &str { &self.share_id.0 }
}

#[derive(Debug, Clone)]
pub struct FileDeleted {
    pub share_id: ShareId,
    pub path: String,
    pub version: VersionVector,
}

impl DomainEvent for FileDeleted {
    fn event_type(&self) -> &str { "FileDeleted" }
    fn aggregate_id(&self) -> &str { &self.share_id.0 }
}

/// Fired whenever the local index for a share is mutated. Subscirbed by a 
/// debonunced scheduler that triggers outbound sync to paired peers.
#[derive(Debug,Clone)]
pub struct LocalIndexChanged {
    pub share_id:ShareId,
}

impl DomainEvent for LocalIndexChanged {
    fn event_type(&self) -> &str {
        "LocalIndexChanged"
    }
    fn aggregate_id(&self) -> &str {
        &self.share_id.0
    }
}