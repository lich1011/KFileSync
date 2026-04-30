use async_trait::async_trait;
use reqwest::{Client, ClientBuilder};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use crate::domain::port::network::{NetworkClient, PairingRequest, PairingResponse, ShareInvite};
use super::dto::{PairRequestDto, PairResponseDto, ShareInviteDto};

pub struct ReqwestNetworkClient {
    client: Client,
}

impl ReqwestNetworkClient {
    pub fn new() -> Result<Self, String> {
        // Accept self-signed certs because devices generate their own certs.
        // Real security is done via SHA-256 fingerprint pinning after pairing.
        let client = ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| e.to_string())?;

        Ok(Self { client })
    }

    fn format_url(ip: &str, port: u16, path: &str) -> String {
        format!("https://{}:{}/api/lansync/v1{}", ip, port, path)
    }
}

#[async_trait]
impl NetworkClient for ReqwestNetworkClient {
    async fn request_pairing(&self, peer_ip: &str, port: u16, req: PairingRequest) -> Result<PairingResponse, String> {
        let url = Self::format_url(peer_ip, port, "/pair/request");

        // Adapter responsibility: add protocol-level fields (timestamp, nonce)
        let dto = PairRequestDto {
            device_id: req.device_id,
            alias: req.alias,
            platform: req.platform,
            fingerprint_short: req.fingerprint,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            nonce: format!("{:x}", rand::random::<u64>()),
        };

        let response = self.client.post(&url)
            .json(&dto)
            .send()
            .await
            .map_err(|e| format!("Network error: {}", e))?;

        if response.status().is_success() {
            let res_dto = response.json::<PairResponseDto>().await
                .map_err(|e| format!("JSON error: {}", e))?;
            // Map DTO back to domain type
            Ok(PairingResponse {
                status: res_dto.status,
                device_id: res_dto.device_id,
                alias: res_dto.alias,
                platform: res_dto.platform,
                fingerprint: res_dto.fingerprint_short,
            })
        } else {
            Err(format!("Server returned error: {}", response.status()))
        }
    }

    async fn invite_to_share(&self, peer_ip: &str, port: u16, invite: ShareInvite) -> Result<(), String> {
        let url = Self::format_url(peer_ip, port, "/share/invite");

        let dto = ShareInviteDto {
            share_id: invite.share_id,
            share_name: invite.share_name,
            permission: invite.permission,
            invited_by: invite.invited_by,
        };

        let response = self.client.post(&url)
            .json(&dto)
            .send()
            .await
            .map_err(|e| format!("Network error: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("Server returned error: {}", response.status()))
        }
    }

    async fn fetch_remote_index(&self, peer_ip: &str, port: u16, share_id: &str) -> Result<String, String> {
        let url = Self::format_url(peer_ip, port, "/sync/index");

        let response = self.client.get(&url)
            .query(&[("share_id", share_id), ("since_version", "0")])
            .send()
            .await
            .map_err(|e| format!("Network error: {}", e))?;

        if response.status().is_success() {
            response.text().await.map_err(|e| format!("Error reading response: {}", e))
        } else {
            Err(format!("Server returned error: {}", response.status()))
        }
    }
}
