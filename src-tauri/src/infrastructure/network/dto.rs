//! Wire-format DTOs for the LanSync protocol.
//! These are serialization-specific structs used by the HTTP client/server adapters.
//! Domain code should NOT import from here — use `domain::port::network` types instead.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PairRequestDto {
    pub device_id: String,
    pub alias: String,
    pub platform: String,
    pub fingerprint_short: String,
    pub timestamp: u64,
    pub nonce: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PairResponseDto {
    pub status: String,
    pub device_id: String,
    pub alias: String,
    pub platform: String,
    pub fingerprint_short: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShareInviteDto {
    pub share_id: String,
    pub share_name: String,
    pub permission: String,
    pub invited_by: String,
    #[serde(default)]
    pub timestamp: u64,
    #[serde(default)]
    pub nonce: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SyncIndexResponseDto {
    pub share_id: String,
    pub index_version: u64,
    pub entries: Vec<crate::domain::model::file_entry::FileEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransferRequestDto {
    pub job_id: String,
    pub session_id: String,
    pub sender_device_id: String,
    pub items: Vec<TransferRequestItemDto>,
    #[serde(default)]
    pub timestamp: u64,
    #[serde(default)]
    pub nonce: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransferRequestItemDto {
    pub file_id: String,
    pub file_path: String,
    pub file_size: u64,
    pub sha256: String,
    pub chunk_count: u32,
    pub chunk_size: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransferResponseDto {
    pub status: String,
    pub skip_chunks: Vec<SkipChunkDto>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SkipChunkDto {
    pub file_id: String,
    pub chunks_already_done: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShareCancelDto{
    pub share_id: String,
    pub device_id: String,
    #[serde(default)]
    pub timestamp: u64,
    #[serde(default)]
    pub nonce: String,
}