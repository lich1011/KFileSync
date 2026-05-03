use async_trait::async_trait;
use crate::domain::error::DomainError;

// Domain-level value objects for network operations.
// These are free of serialization concerns (no Serialize/Deserialize).
// Protocol details like nonce, timestamp are handled by the infrastructure adapter.

#[derive(Debug, Clone)]
pub struct PairingRequest {
    pub device_id: String,
    pub alias: String,
    pub platform: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone)]
pub struct PairingResponse {
    pub status: String,
    pub device_id: String,
    pub alias: String,
    pub platform: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone)]
pub struct ShareInvite {
    pub share_id: String,
    pub share_name: String,
    pub permission: String,
    pub invited_by: String,
}

#[derive(Debug, Clone)]
pub struct TransferRequest {
    pub job_id: String,
    pub session_id: String,
    pub sender_device_id: String,
    pub items: Vec<TransferRequestItem>,
}

#[derive(Debug, Clone)]
pub struct TransferRequestItem {
    pub file_id: String,
    pub file_path: String,
    pub file_size: u64,
    pub sha256: String,
    pub chunk_count: u32,
    pub chunk_size: u32,
}

#[derive(Debug, Clone)]
pub struct TransferResponse {
    pub status: String,
    pub skip_chunks: Vec<SkipChunkInfo>,
}

#[derive(Debug, Clone)]
pub struct SkipChunkInfo {
    pub file_id: String,
    pub chunks_already_done: u32,
}

#[async_trait]
pub trait NetworkClient: Send + Sync {
    /// Send a pairing request to a peer device.
    async fn request_pairing(&self, peer_addr: &str, port: u16, req: PairingRequest) -> Result<PairingResponse,DomainError>;

    /// Invite a peer device to join a share.
    async fn invite_to_share(&self, peer_addr: &str, port: u16, invite: ShareInvite) -> Result<(), DomainError>;

    /// Fetch the file index from a remote peer for a given share.
    async fn fetch_remote_index(&self, peer_addr: &str, port: u16, share_id: &str) -> Result<String, DomainError>;

    async fn request_transfer(&self, peer_addr: &str, port: u16, req: TransferRequest) -> Result<TransferResponse, DomainError>;

    async fn download_chunk(&self, peer_addr: &str, port: u16, job_id: &str, file_id: &str, chunk_index: u32) -> Result<Vec<u8>, DomainError>;

    async fn upload_chunk(&self, peer_addr: &str, port: u16, job_id: &str, file_id: &str, chunk_index: u32, data: Vec<u8>) -> Result<(), DomainError>;

    async fn cancel_share_invite(&self, peer_addr: &str, port: u16, share_id: &str, device_id: &str) -> Result<(), DomainError>;
}
