use crate::domain::port::event_bus::DomainEvent;
use crate::domain::model::device::DeviceId;
use crate::domain::model::transfer::{JobId, FileId, TransferError};

#[derive(Debug, Clone)]
pub struct TransferRequested {
    pub job_id: JobId,
    pub peer: DeviceId,
    // Note: files manifest could be included here
}

impl DomainEvent for TransferRequested {
    fn event_type(&self) -> &str { "TransferRequested" }
    fn aggregate_id(&self) -> &str { &self.job_id.0 }
}

#[derive(Debug, Clone)]
pub struct TransferProgressUpdated{
    pub job_id: JobId,
    pub file_id: FileId,
    pub chunks_done: u32,
    pub total_chunks: u32,
    pub bytes_done: u64,
    pub total_bytes: u64
}

impl DomainEvent for TransferProgressUpdated {
    fn event_type(&self) -> &str {
        "TransferProgressUpdated"
    }

    fn aggregate_id(&self) -> &str {
        &self.job_id.0
    }
}

#[derive(Debug, Clone)]
pub struct TransferCompleted {
    pub job_id: JobId,
    pub total_bytes: u64,
}

impl DomainEvent for TransferCompleted {
    fn event_type(&self) -> &str { "TransferCompleted" }
    fn aggregate_id(&self) -> &str { &self.job_id.0 }
}

#[derive(Debug, Clone)]
pub struct TransferFailed {
    pub job_id: JobId,
    pub error: TransferError,
}

impl DomainEvent for TransferFailed {
    fn event_type(&self) -> &str { "TransferFailed" }
    fn aggregate_id(&self) -> &str { &self.job_id.0 }
}

#[derive(Debug, Clone)]
pub struct ChunkVerificationFailed {
    pub job_id: JobId,
    pub file_id: FileId,
    pub chunk_index: u32,
}

impl DomainEvent for ChunkVerificationFailed {
    fn event_type(&self) -> &str { "ChunkVerificationFailed" }
    fn aggregate_id(&self) -> &str { &self.job_id.0 }
}
