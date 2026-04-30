use async_trait::async_trait;

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

#[async_trait]
pub trait NetworkClient: Send + Sync {
    /// Send a pairing request to a peer device.
    async fn request_pairing(&self, peer_addr: &str, port: u16, req: PairingRequest) -> Result<PairingResponse, String>;

    /// Invite a peer device to join a share.
    async fn invite_to_share(&self, peer_addr: &str, port: u16, invite: ShareInvite) -> Result<(), String>;

    /// Fetch the file index from a remote peer for a given share.
    async fn fetch_remote_index(&self, peer_addr: &str, port: u16, share_id: &str) -> Result<String, String>;
}
