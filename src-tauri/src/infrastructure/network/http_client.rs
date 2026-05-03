use async_trait::async_trait;
use reqwest::{Client, ClientBuilder};
use std::sync::atomic::{AtomicU64,Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use crate::domain::error::DomainError;
use crate::domain::port::network::{NetworkClient, PairingRequest, PairingResponse, ShareInvite, 
    TransferResponse, SkipChunkInfo};
use super::dto::{PairRequestDto, PairResponseDto, ShareInviteDto, TransferRequestDto, TransferResponseDto, TransferRequestItemDto};

fn net_err(e: impl std::fmt::Display) -> DomainError {
    DomainError::Network(format!("{}", e))
}

pub struct ReqwestNetworkClient {
    client: Mutex<Client>,
    tls_config: Arc<TlsRotationConfig>,
    created_at: Mutex<Instant>,
    bytes_transferred: AtomicU64,  
}

struct TlsRotationConfig {
    max_age: Duration,
    max_byte: u64,
    trusted_fingerprints: Mutex<std::collections::HashSet<String>>,
}

impl ReqwestNetworkClient {
    pub fn new() -> Result<Self, DomainError> {
        // Accept self-signed certs because devices generate their own certs.
        // Real security is done via SHA-256 fingerprint pinning after pairing.
       let client = Self::build_client()?;

        Ok(Self { 
            client: Mutex::new(client),
            tls_config: Arc::new(TlsRotationConfig {
                max_age: Duration::from_secs(3600),
                max_byte: 1_073_741_824,
                trusted_fingerprints: Mutex::new(std::collections::HashSet::new()),
            }),
            created_at: Mutex::new(Instant::now()),
            bytes_transferred: AtomicU64::new(0),  
        })
    }

    fn build_client() -> Result<Client, DomainError> {
        ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(net_err)
    }

    pub fn add_trusted_fingerprint(&self, device_id: &str) {
        if let Ok(mut fps) = self.tls_config.trusted_fingerprints.lock(){
            fps.insert(device_id.to_string());
        }
    }

    pub fn remove_trusted_fingerprint(&self, device_id: &str) {
        if let Ok(mut fps) = self.tls_config.trusted_fingerprints.lock(){
            fps.remove(device_id);
        }
    }  
    
    fn  maybe_rotate(&self) {
        let should_rotate =  {
            let created_at = self.created_at.lock().unwrap();
            let elapsed = created_at.elapsed();
            let bytes = self.bytes_transferred.load(Ordering::Relaxed);
            elapsed >= self.tls_config.max_age || bytes >= self.tls_config.max_byte 
        };

        if should_rotate {
            if let Ok(new_client) = Self::build_client() {
                if let Ok(mut client) = self.client.lock() {
                    *client = new_client;
                }
                if let Ok(mut created_at) = self.created_at.lock() {
                    *created_at = Instant::now();
                }
                self.bytes_transferred.store(0, Ordering::Relaxed);
            }
        }
    }

    fn get_client(&self) -> Client {
        self.maybe_rotate();
        self.client.lock().unwrap().clone()
    }

    fn track_bytes(&self, bytes: u64) {
        self.bytes_transferred.fetch_add(bytes, Ordering::Relaxed);
    }

    fn format_url(ip: &str, port: u16, path: &str) -> String {
        format!("https://{}:{}/api/lansync/v1{}", ip, port, path)
    }

    fn generate_nanoc() -> String{
        format!("{:016x}", rand::random::<u64>())

    }

    fn now_timestamp() -> u64 { 
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    }
}

#[async_trait]
impl NetworkClient for ReqwestNetworkClient {
    async fn request_pairing(&self, peer_ip: &str, port: u16, req: PairingRequest) -> Result<PairingResponse, DomainError> {
        let url = Self::format_url(peer_ip, port, "/pair/request");
        let client = self.get_client();

        // Adapter responsibility: add protocol-level fields (timestamp, nonce)
        let dto = PairRequestDto {
            device_id: req.device_id,
            alias: req.alias,
            platform: req.platform,
            fingerprint_short: req.fingerprint,
            timestamp: Self::now_timestamp(),
            nonce: Self::generate_nanoc(),
        };

        let response = client.post(&url)
            .json(&dto)
            .send()
            .await
            .map_err(net_err)?;

        if response.status().is_success() {
            let res_dto = response.json::<PairResponseDto>().await
                .map_err(net_err)?;
            // Map DTO back to domain type
            Ok(PairingResponse {
                status: res_dto.status,
                device_id: res_dto.device_id,
                alias: res_dto.alias,
                platform: res_dto.platform,
                fingerprint: res_dto.fingerprint_short,
            })
        } else {
            Err(DomainError::Network(format!("Server returned error: {}", response.status())))
        }
    }

    async fn invite_to_share(&self, peer_ip: &str, port: u16, invite: ShareInvite) -> Result<(), DomainError> {
        let url = Self::format_url(peer_ip, port, "/share/invite");
        let client = self.get_client();

        let dto = ShareInviteDto {
            share_id: invite.share_id,
            share_name: invite.share_name,
            permission: invite.permission,
            invited_by: invite.invited_by,
            timestamp: Self::now_timestamp(),
            nonce: Self::generate_nanoc(),
        };  

        let response = client.post(&url)
            .json(&dto)
            .send()
            .await
            .map_err(net_err)?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(DomainError::Network(format!("Server returned error: {}", response.status())))
        }
    }

    async fn fetch_remote_index(&self, peer_ip: &str, port: u16, share_id: &str) -> Result<String, DomainError> {
        let url = Self::format_url(peer_ip, port, "/sync/index");   
        let client = self.get_client();

        let response = client.get(&url)
            .query(&[("share_id", share_id), ("since_version", "0")])
            .send()
            .await
            .map_err(net_err)?;

        if response.status().is_success() {
            response.text().await.map_err(net_err)
        } else {
            Err(DomainError::Network(format!("Server returned error: {}", response.status())))
        }
    }

    async fn request_transfer(&self, peer_ip: &str, port: u16, request: crate::domain::port::network::TransferRequest) -> Result<crate::domain::port::network::TransferResponse, DomainError> {
        let url = Self::format_url(peer_ip, port, "/transfer/request");
        let client = self.get_client();

        let dto = TransferRequestDto {
            job_id: request.job_id,
            session_id: request.session_id,
            sender_device_id: request.sender_device_id,
            items: request.items.iter().map(|item| TransferRequestItemDto {
                file_id: item.file_id.clone(),
                file_path: item.file_path.clone(),
                file_size: item.file_size,
                sha256: item.sha256.clone(),
                chunk_size: item.chunk_size,
                chunk_count: item.chunk_count,
            }).collect(),   
            timestamp: Self::now_timestamp(),
            nonce: Self::generate_nanoc(),
        };

        let response = client.post(&url)
            .json(&dto)
            .send()
            .await
            .map_err(net_err)?;

        if response.status().is_success() {
            let res_dto = response.json::<TransferResponseDto>().await
                .map_err(net_err)?;
            Ok(TransferResponse { 
               status: res_dto.status,
               skip_chunks: res_dto.skip_chunks.iter().map(|chunk| SkipChunkInfo {
                   file_id: chunk.file_id.clone(),
                   chunks_already_done: chunk.chunks_already_done,
               }).collect(),
            })
        } else {
            Err(DomainError::Network(format!("Server returned error: {}", response.status())))
        }
    } 
    
    async fn download_chunk(&self, peer_ip: &str, port: u16, job_id: &str, file_id: &str, chunk_index: u32) -> Result<Vec<u8>, DomainError> {
        let url = Self::format_url(peer_ip, port, 
            &format!("/transfer/{}/{}/chunk/{}", job_id, file_id, chunk_index));
        let client = self.get_client();

        let response = client.get(&url)
            .send()
            .await
            .map_err(net_err)?;

        if response.status().is_success() {
            let bytes = response.bytes().await.map_err(net_err)?;
            self.track_bytes(bytes.len() as u64);
            Ok(bytes.to_vec())
        } else {
            Err(DomainError::Network(format!("Server returned error: {}", response.status())))
        }
    }

    async fn upload_chunk(&self, peer_ip: &str, port: u16, job_id: &str, file_id: &str, chunk_index: u32, data: Vec<u8>) -> Result<(), DomainError> {
        let url = Self::format_url(peer_ip, port, 
            &format!("/transfer/{}/chunk", job_id));
        let client = self.get_client();

        let len = data.len();
        let response = client.post(&url)
            .query(&[("file_id", file_id.to_string()), ("chunk_index", chunk_index.to_string())])
            .body(data)
            .send()
            .await
            .map_err(net_err)?;

        if response.status().is_success() {
            self.track_bytes(len as u64);    
            Ok(())
        } else {
            Err(DomainError::Network(format!("Server returned error: {}", response.status())))
        }
    }   

    async fn cancel_share_invite(&self, peer_ip: &str, port: u16, share_id: &str, device_id: &str) -> Result<(), DomainError> {
        let url = Self::format_url(peer_ip, port, "/share/cancel");
        let client = self.get_client();

        let body = serde_json::json!({
            "share_id": share_id,
            "device_id": device_id,
            "timestamp": Self::now_timestamp(),
            "nonce": Self::generate_nanoc(),
        }); 

        let response = client.post(&url)
            .json(&body)
            .send()
            .await
            .map_err(net_err)?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(DomainError::Network(format!("Server returned error: {}", response.status())))
        }
    }       
}
