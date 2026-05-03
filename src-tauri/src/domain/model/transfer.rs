use serde::{Deserialize, Serialize};
use crate::domain::model::device::DeviceId;
use crate::domain::error::DomainError;
use uuid::Uuid;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct JobId(pub String);

#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct FileId(pub String);

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum TransferType {
    Send,
    Receive,
    SyncPull,
    SyncPush,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Checkpoint {
    pub file_id: FileId,
    pub chunks_done: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransferProgress {
    pub total_bytes: u64,
    pub transferred_bytes: u64,
    pub total_files: u32,
    pub completed_files: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkInfo {
    pub index: u32,
    pub offset: u64,
    pub size: u32,
    pub hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkManifest {
    pub chunks: Vec<ChunkInfo>,
    pub chunk_size: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransferItemStatus {
    Pending,
    Transferring,
    Verifying,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransferItem {
    pub file_id: FileId,
    pub file_path: String,
    pub file_size: u64,
    pub sha256: String,
    pub status: TransferItemStatus,
    pub chunk_manifest: ChunkManifest,
    pub chunks_done: u32,
    pub temp_path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransferError {
    ConnectionLost,
    VerificationFailed,
    StorageError,
    PeerRejected,
    Timeout,
    Unknown(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransferState {
    Pending,
    Active { started_at: u64 },
    Paused { checkpoint: Option<Checkpoint> },
    Verifying,
    Completed { completed_at: u64 },
    Failed { error: TransferError, retries: u32 },
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransferJob {
    pub job_id: JobId,
    pub session_id: String,
    pub job_type: TransferType,
    pub peer_device_id: DeviceId,
    pub share_id: Option<String>,
    pub state: TransferState,
    pub items: Vec<TransferItem>,
    pub created_at: u64,
}

#[derive(Clone, Debug)]
pub struct FileRequest {
    pub file_path: String,
    pub file_size: u64,
    pub sha256: String,
}

impl TransferJob {
    pub fn create_from_files(
        job_type: TransferType,
        peer_device_id: DeviceId,
        files: Vec<FileRequest>,
        chunking_strategy: &dyn crate::domain::service::chunking::ChunkingStrategy,
    ) -> Self {
        let mut items = Vec::new();

        for file in files {
            let chunk_size = chunking_strategy.compute_chunk_size(file.file_size);
            let mut chunks = Vec::new();
            
            if chunk_size == 0 {
                chunks.push(ChunkInfo {
                    index: 0,
                    offset: 0,
                    size: file.file_size as u32,
                    hash: "".to_string(), // In real app, calculate actual BLAKE3 hash
                });
            } else {
                let mut offset = 0;
                let mut index = 0;
                while offset < file.file_size {
                    let size = std::cmp::min(chunk_size as u64, file.file_size - offset) as u32;
                    chunks.push(ChunkInfo {
                        index,
                        offset,
                        size,
                        hash: "".to_string(), // In real app, calculate actual BLAKE3 hash
                    });
                    offset += size as u64;
                    index += 1;
                }
            }

            let manifest = ChunkManifest {
                chunks,
                chunk_size,
            };

            items.push(TransferItem {
                file_id: FileId(Uuid::new_v4().to_string()),
                file_path: file.file_path,
                file_size: file.file_size,
                sha256: file.sha256,
                status: TransferItemStatus::Pending,
                chunk_manifest: manifest,
                chunks_done: 0,
                temp_path: None,
            });
        }

        Self {
            job_id: JobId(Uuid::new_v4().to_string()),
            session_id: Uuid::new_v4().to_string(),
            job_type,
            peer_device_id,
            share_id: None,
            state: TransferState::Pending,
            items,
            created_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        }
    }

    /// 内部构造器，仅供测试和内部使用。外部应使用 `create_from_files()` 工厂方法。
    #[allow(dead_code)]
    pub(crate) fn new(
        job_type: TransferType,
        peer_device_id: DeviceId,
        items: Vec<TransferItem>,
    ) -> Self {
        Self {
            job_id: JobId(Uuid::new_v4().to_string()),
            session_id: Uuid::new_v4().to_string(),
            job_type,
            peer_device_id,
            share_id: None,
            state: TransferState::Pending,
            items,
            created_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        }
    }

    pub fn accept(mut self) -> Result<Self, DomainError> {
        if !matches!(self.state, TransferState::Pending) {
            return Err(DomainError::InvalidStateTransition("Only Pending jobs can be accepted"));
        }
        self.state = TransferState::Active {
            started_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        };
        Ok(self)
    }

    pub fn record_chunk_done(mut self, file_id: &FileId, chunk_index: u32) -> Result<Self, DomainError> {
        if !matches!(self.state, TransferState::Active { .. }) {
            return Err(DomainError::InvalidStateTransition("Job must be Active to record progress"));
        }

        for item in &mut self.items {
            if &item.file_id == file_id {
                item.chunks_done = std::cmp::max(item.chunks_done, chunk_index + 1);
                // 更新局部文件传输状态
                if item.chunks_done >= item.chunk_manifest.chunks.len() as u32 {
                    item.status = TransferItemStatus::Verifying;
                } else {
                    item.status = TransferItemStatus::Transferring;
                }
            }
        }

        // A2 FIX: 只有所有 item 均达到 Verifying 或 Completed 状态才进入下一阶段
        let all_done = self.items.iter().all(|item| {
            matches!(item.status, TransferItemStatus::Verifying | TransferItemStatus::Completed)
        });

        if all_done {
            self.state = TransferState::Verifying;
        }

        Ok(self)
    }

    pub fn pause(mut self, checkpoint: Option<Checkpoint>) -> Result<Self, DomainError> {
        if !matches!(self.state, TransferState::Active { .. }) {
            return Err(DomainError::InvalidStateTransition("Only Active jobs can be paused"));
        }
        self.state = TransferState::Paused { checkpoint };
        Ok(self)
    }

    pub fn resume(mut self) -> Result<Self, DomainError> {
        if !matches!(self.state, TransferState::Paused { .. }) {
            return Err(DomainError::InvalidStateTransition("Only Paused jobs can be resumed"));
        }
        self.state = TransferState::Active {
            started_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        };
        Ok(self)
    }

    pub fn begin_verify(mut self) -> Result<Self, DomainError> {
        if !matches!(self.state, TransferState::Active { .. }) {
            return Err(DomainError::InvalidStateTransition("Only Active jobs can begin verify"));
        }
        self.state = TransferState::Verifying;
        Ok(self)
    }

    pub fn complete(mut self) -> Result<Self, DomainError> {
        // A3 FIX: 严格按即状态机设计，只允许 Verifying -> Completed
        if !matches!(self.state, TransferState::Verifying) {
            return Err(DomainError::InvalidStateTransition("Job must be in Verifying state to be completed"));
        }
        self.state = TransferState::Completed {
            completed_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        };
        for item in &mut self.items {
            item.status = TransferItemStatus::Completed;
        }
        Ok(self)
    }

    pub fn fail(mut self, error: TransferError) -> Result<Self, DomainError> {

        match self.state {
            TransferState::Active { .. }
            | TransferState::Paused { .. }
            | TransferState::Verifying
            | TransferState::Failed { .. } => {}
            _ => {
                return Err(DomainError::InvalidStateTransition(
                    "Only Active, Paused, Verifying or Failed jobs can be failed"
                ));
            }
        }

        let retries = match self.state {
            TransferState::Failed { retries, .. } => retries + 1,
            _ => 0,
        };
        self.state = TransferState::Failed { error, retries };
        Ok(self)
    }

    pub fn cancel(mut self) -> Result<Self, DomainError> {
        self.state = TransferState::Cancelled;
        Ok(self)
    }

    pub fn progress(&self) -> TransferProgress {
        let mut total_bytes: u64 = 0;
        let mut transferred_bytes: u64 = 0;
        let mut total_files: u32 = 0;
        let mut completed_files: u32 = 0;

        for item in &self.items {
            total_bytes += item.file_size;
            total_files += 1;

            if item.status == TransferItemStatus::Completed {
                transferred_bytes += item.file_size;
                completed_files += 1;
            } else {
                // A1 FIX: chunk_size=0 表示小文件不分块，直接按已完成将整个文件计入进度
                if item.chunk_manifest.chunk_size == 0 {
                    if item.chunks_done > 0 {
                        transferred_bytes += item.file_size;
                    }
                } else {
                    let done_bytes = item.chunks_done as u64 * item.chunk_manifest.chunk_size as u64;
                    // Cap 在文件大小上，防止最后一块计算偏大
                    transferred_bytes += done_bytes.min(item.file_size);
                }
            }
        }

        TransferProgress {
            total_bytes,
            transferred_bytes,
            total_files,
            completed_files,
        }
    }
}
