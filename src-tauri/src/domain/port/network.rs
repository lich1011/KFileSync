use async_trait::async_trait;
use crate::domain::error::DomainError;
use std::sync::Arc;

// Domain-level value objects for network operations.
// These are free of serialization concerns (no Serialize/Deserialize).
// Protocol details like nonce, timestamp are handled by the infrastructure adapter.

/// Sprint 5: outcome of `POST /pair/confirm` (`kfilesync_core::protocol::dto::PairResultDto`).
#[derive(Debug, Clone)]
pub struct PairConfirmOutcome {
    pub accepted: bool,
    /// The peer's certificate — present only when `accepted`.
    pub peer_certificate_pem: Option<String>,
    /// Present only when rejected.
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ShareInvite {
    pub share_id: String,
    pub share_name: String,
    pub permission: String,
    pub invited_by: String,
    /// `"two_way"` / `"send_only"` / `"receive_only"` — wired through to the
    /// wire's `sync_mode` field (Sprint 4, `kfilesync_core::protocol::dto::ShareInviteDto`).
    pub sync_mode: String,
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
    /// BLAKE3 hex per chunk, in order, Sprint 4 fix: previously these were
    /// never sent to the receiver, so per-chunk verification on upload was
    /// always skipped (`ChunkInfo.hash` stayed `String::new()`).
    pub chunk_hashes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TransferResponse {
    pub status: String,
    pub skip_chunks: Vec<SkipChunkInfo>,
}

#[derive(Debug, Clone)]
pub struct SkipChunkInfo {
    pub file_id: String,
    /// Chunk indices the receiver already has, per `TransferAcceptDto::skip_chunks`
    /// (a discrete set — a strict superset of the old "first N contiguous
    /// chunks" shape, since it can express arbitrary partial resumption).
    pub chunk_indices: Vec<u32>,
}

#[async_trait]
pub trait NetworkClient: Send + Sync {
    /// Send the dual-PIN OOB `POST /pair/request` (ADR-010). `body`/`headers`
    /// are pre-serialized by `kfilesync_core::trust::pairing_state::start_outbound`,
    /// which couples pairing-session creation with wire-body construction on
    /// the domain side — this is a deliberate exception to this trait's
    /// usual "free of serialization concerns" rule.
    async fn send_pair_request(&self, peer_addr: &str, port: u16, body: Vec<u8>, headers: Vec<(String, String)>) -> Result<(), DomainError>;

    /// Send `POST /pair/confirm` (see `pairing_state::prepare_confirm`) and
    /// parse the peer's `PairResultDto` response.
    async fn send_pair_confirm(&self, peer_addr: &str, port: u16, body: Vec<u8>, headers: Vec<(String, String)>) -> Result<PairConfirmOutcome, DomainError>;

    /// Enable TOFU bootstrap mode (accept any peer cert) for the duration of
    /// one outbound pairing call. Reference-counted by the implementation so
    /// concurrent pairing attempts don't disable each other's bootstrap
    /// window early — see [`BootstrapGuard`].
    fn enable_bootstrap(&self);
    /// Undo one `enable_bootstrap` call.
    fn disable_bootstrap(&self);

    /// Add a peer's cert fingerprint to the trusted/pinned set (called once
    /// pairing succeeds).
    fn trust_fingerprint(&self, fingerprint: &str);
    /// Remove a peer's cert fingerprint from the trusted set (called on revoke).
    fn untrust_fingerprint(&self, fingerprint: &str);

    /// Invite a peer device to join a share.
    async fn invite_to_share(&self, peer_addr: &str, port: u16, invite: ShareInvite) -> Result<(), DomainError>;

    /// Fetch the file index from a remote peer for a given share.
    async fn fetch_remote_index(&self, peer_addr: &str, port: u16, share_id: &str) -> Result<String, DomainError>;

    async fn request_transfer(&self, peer_addr: &str, port: u16, req: TransferRequest) -> Result<TransferResponse, DomainError>;

    async fn download_chunk(&self, peer_addr: &str, port: u16, job_id: &str, file_id: &str, chunk_index: u32) -> Result<Vec<u8>, DomainError>;

    async fn upload_chunk(&self, peer_addr: &str, port: u16, job_id: &str, file_id: &str, chunk_index: u32, chunk_hash: &str, data: Vec<u8>) -> Result<(), DomainError>;

    async fn cancel_share_invite(&self, peer_addr: &str, port: u16, share_id: &str, device_id: &str) -> Result<(), DomainError>;
}

/// RAII guard around [`NetworkClient::enable_bootstrap`]/`disable_bootstrap`.
///
/// Sprint 5: previously the desktop app called `enable_bootstrap()` once at
/// startup and never called `disable_bootstrap()` — TOFU (accept any peer
/// cert) was permanently on, making the `trusted_fingerprints` pinning set
/// pointless. Scope a guard tightly around each individual outbound pairing
/// call (`send_pair_request`, `send_pair_confirm`) instead: bootstrap is
/// enabled only while this device is dialing an as-yet-untrusted peer for
/// the pairing ceremony itself, and disabled again as soon as that call
/// returns (success or failure), regardless of how the caller unwinds.
pub struct BootstrapGuard {
    client: Arc<dyn NetworkClient>,
}

impl BootstrapGuard {
    #[must_use]
    pub fn new(client: Arc<dyn NetworkClient>) -> Self {
        client.enable_bootstrap();
        Self { client }
    }
}

impl Drop for BootstrapGuard {
    fn drop(&mut self) {
        self.client.disable_bootstrap();
    }
}