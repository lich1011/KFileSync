use async_trait::async_trait;
use reqwest::{Client, ClientBuilder};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use crate::domain::error::DomainError;
use crate::domain::model::device;
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
    local_device_id: Mutex<Option<String>>
}

struct TlsRotationConfig {
    max_age: Duration,
    max_byte: u64,
    trusted_fingerprints: Arc<Mutex<std::collections::HashSet<String>>>,
    bootstrap: Arc<AtomicBool>
}

mod cert_pinning {
    use std::sync::{Arc, Mutex};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::collections::HashSet;
    use sha2::{Sha256, Digest};
    use rustls::client::danger::{ServerCertVerifier, ServerCertVerified, HandshakeSignatureValid};
    use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
    use rustls::{DigitallySignedStruct, SignatureScheme, Error};

    #[derive(Debug)]
    pub struct FingerprintVerifier {
        trusted: Arc<Mutex<HashSet<String>>>,
        bootstrap: Arc<AtomicBool>,
        schemes: Vec<SignatureScheme>,
    }

    impl FingerprintVerifier {
        pub fn new(trusted: Arc<Mutex<HashSet<String>>>, bootstrap: Arc<AtomicBool>) -> Self {
            Self {
                trusted,
                bootstrap,
                schemes: vec![
                    SignatureScheme::ECDSA_NISTP256_SHA256,
                    SignatureScheme::ECDSA_NISTP384_SHA384,
                    SignatureScheme::ED25519,
                    SignatureScheme::RSA_PSS_SHA256,
                    SignatureScheme::RSA_PSS_SHA384,
                    SignatureScheme::RSA_PSS_SHA512,
                    SignatureScheme::RSA_PKCS1_SHA256,
                    SignatureScheme::RSA_PKCS1_SHA384,
                    SignatureScheme::RSA_PKCS1_SHA512,
                ],
            }
        }
    }

    impl ServerCertVerifier for FingerprintVerifier {
        fn verify_server_cert(
            &self,
            end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            _ocsp_response: &[u8],
            _now: UnixTime,
        ) -> Result<ServerCertVerified, Error> {
            let hash = Sha256::digest(end_entity.as_ref());
            let fingerprint: String = hash.iter().map(|b| format!("{:02x}", b)).collect();

            let trusted = self.trusted.lock()
                .map_err(|_| Error::General("Lock poisoned".into()))?;

            if trusted.contains(&fingerprint) {
                return Ok(ServerCertVerified::assertion());
            }

            if self.bootstrap.load(Ordering::SeqCst) {
                return Ok(ServerCertVerified::assertion());
            }

            Err(Error::General(format!(
                "Certificate fingerprint {} not in trusted set", fingerprint
            )))
            
        }

        fn verify_tls12_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, Error> {
            rustls::crypto::verify_tls12_signature(
                message,
                cert,
                dss,
                &rustls::crypto::aws_lc_rs::default_provider().signature_verification_algorithms,
            )
        }

        fn verify_tls13_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, Error> {
            rustls::crypto::verify_tls13_signature(
                message,
                cert,
                dss,
                &rustls::crypto::aws_lc_rs::default_provider().signature_verification_algorithms,
            )
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            self.schemes.clone()
        }
    }
}

impl ReqwestNetworkClient {
    pub fn new() -> Result<Self, DomainError> {
        // Accept self-signed certs because devices generate their own certs.
        // Real security is done via SHA-256 fingerprint pinning after pairing.
       let trusted_fingerprints = Arc::new(Mutex::new(std::collections::HashSet::new()));
       let bootstrap = Arc::new(AtomicBool::new(false));
       let client = Self::build_client(trusted_fingerprints.clone(),bootstrap.clone())?;

        Ok(Self { 
            client: Mutex::new(client),
            tls_config: Arc::new(TlsRotationConfig {
                max_age: Duration::from_secs(3600),
                max_byte: 1_073_741_824,
                trusted_fingerprints,
                bootstrap
            }),
            created_at: Mutex::new(Instant::now()),
            bytes_transferred: AtomicU64::new(0),
            local_device_id: Mutex::new(None)
        })
    }

    pub fn set_local_device_id(&self, device_id: String){
        if let Ok(mut g) = self.local_device_id.lock(){
            *g =Some(device_id);
        }
    }

    fn caller_id(&self) -> String{
        self.local_device_id.lock().ok().and_then(|g| g.clone()).unwrap_or_default()
    }

    fn build_client(
        trusted_fingerprints: Arc<Mutex<HashSet<String>>>,
        bootstrap: Arc<AtomicBool>
    ) -> Result<Client, DomainError> {
        let verifier =  cert_pinning::FingerprintVerifier::new(trusted_fingerprints,bootstrap);

        let tls_config = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(verifier))
            .with_no_client_auth();

        ClientBuilder::new()
            .tls_backend_preconfigured(tls_config)
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(net_err)
    }

    /// Enable TOFU mode - accept any cert prestend by the peer. Use ONLY during a pairing
    /// flow that is guarded by an out-of-band PIN/code. Always pair this with a matching 
    /// `disable_bootstrap()` call when pairing finishes (success OR failure).
    pub fn endable_bootstrap(&self) {
        self.tls_config.bootstrap.store(true, Ordering::SeqCst);
    }

    pub fn disable_bootstrap(&self) {
        self.tls_config.bootstrap.store(false, Ordering::SeqCst);
    }


    pub fn add_trusted_fingerprint(&self, fingerprint: &str) {
        if let Ok(mut fps) = self.tls_config.trusted_fingerprints.lock(){
            fps.insert(fingerprint.to_string());
        }
    }

    pub fn remove_trusted_fingerprint(&self, fingerprint: &str) {
        if let Ok(mut fps) = self.tls_config.trusted_fingerprints.lock(){
            fps.remove(fingerprint);
        }
    }  
    
    fn maybe_rotate(&self) {
        let should_rotate =  {
            let created_at = self.created_at.lock().unwrap();
            let elapsed = created_at.elapsed();
            let bytes = self.bytes_transferred.load(Ordering::Relaxed);
            elapsed >= self.tls_config.max_age || bytes >= self.tls_config.max_byte
        };

        if should_rotate {
            if let Ok(new_client) = Self::build_client(
                self.tls_config.trusted_fingerprints.clone(),
                self.tls_config.bootstrap.clone()
            ) {
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
        use rand::RngCore;
        let mut rng = rand::rngs::OsRng;
        format!("{:016x}", rng.next_u64())
    }

    fn now_timestamp() -> u64 { 
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
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
        let caller = self.caller_id();
        let ts = Self::now_timestamp().to_string();
        let nonce = Self::generate_nanoc();

        let response = client.get(&url)
            .query(&[
                ("share_id", share_id.to_string()), 
                ("since_version", "0".to_string()),
                ("caller_device_id", caller),
                ("timestamp",ts),
                ("nonce",nonce)
            ])
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
            &format!("/transfer/{}/chunk/{}/{}", job_id, file_id, chunk_index));
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
