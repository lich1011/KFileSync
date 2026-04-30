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
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SyncIndexResponseDto {
    pub share_id: String,
    pub index_version: u64,
    pub entries: Vec<crate::domain::model::file_entry::FileEntry>,
}
