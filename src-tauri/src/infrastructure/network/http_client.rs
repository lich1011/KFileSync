use async_trait::async_trait;
use reqwest::{Client, ClientBuilder};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use crate::domain::error::DomainError;
use crate::domain::port::network::{NetworkClient, PairConfirmOutcome, ShareInvite, TransferResponse, SkipChunkInfo};
use kfilesync_core::protocol::{dto as core_dto, headers};

fn net_err(e: impl std::fmt::Display) -> DomainError {
    DomainError::Network(format!("{}", e))
}

pub struct ReqwestNetworkClient {
    client: Mutex<Client>,
    tls_config: Arc<TlsRotationConfig>,
    created_at: Mutex<Instant>,
    bytes_transferred: AtomicU64,
    local_device_id: Mutex<Option<String>>,
}

struct TlsRotationConfig {
    max_age: Duration,
    max_byte: u64,
    trusted_fingerprints: Arc<Mutex<std::collections::HashSet<String>>>,
    bootstrap: Arc<AtomicBool>,
    /// Sprint 5: `enable_bootstrap`/`disable_bootstrap` are reference-counted
    /// so two concurrent pairing attempts (e.g. this device dialing out while
    /// also mid-ceremony as a responder) don't have one call's `disable`
    /// switch bootstrap off underneath the other's still-in-flight dial.
    bootstrap_refcount: AtomicU32,
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
        let client = Self::build_client(trusted_fingerprints.clone(), bootstrap.clone())?;

        Ok(Self {
            client: Mutex::new(client),
            tls_config: Arc::new(TlsRotationConfig {
                max_age: Duration::from_secs(3600),
                max_byte: 1_073_741_824,
                trusted_fingerprints,
                bootstrap,
                bootstrap_refcount: AtomicU32::new(0),
            }),
            created_at: Mutex::new(Instant::now()),
            bytes_transferred: AtomicU64::new(0),
            local_device_id: Mutex::new(None),
        })
    }

    pub fn set_local_device_id(&self, device_id: String) {
        if let Ok(mut g) = self.local_device_id.lock() {
            *g = Some(device_id);
        }
    }

    fn caller_id(&self) -> String {
        self.local_device_id.lock().ok().and_then(|g| g.clone()).unwrap_or_default()
    }

    fn build_client(
        trusted_fingerprints: Arc<Mutex<HashSet<String>>>,
        bootstrap: Arc<AtomicBool>,
    ) -> Result<Client, DomainError> {
        let verifier = cert_pinning::FingerprintVerifier::new(trusted_fingerprints, bootstrap);

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

    fn maybe_rotate(&self) {
        let should_rotate = {
            let created_at = self.created_at.lock().unwrap();
            let elapsed = created_at.elapsed();
            let bytes = self.bytes_transferred.load(Ordering::Relaxed);
            elapsed >= self.tls_config.max_age || bytes >= self.tls_config.max_byte
        };

        if should_rotate {
            if let Ok(new_client) = Self::build_client(
                self.tls_config.trusted_fingerprints.clone(),
                self.tls_config.bootstrap.clone(),
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
        let trimmed = ip.trim();
        let trimmed = trimmed
            .strip_prefix("https://")
            .or_else(|| trimmed.strip_prefix("http://"))
            .unwrap_or(trimmed);

        let host_port = trimmed.split('/').next().unwrap_or(trimmed);

        let (host, final_port) = if host_port.starts_with('[') {
            if let Some(end_bracket) = host_port.find(']') {
                let host = &host_port[1..end_bracket];
                let rest = &host_port[end_bracket + 1..];
                let port_val = rest
                    .strip_prefix(':')
                    .and_then(|p| p.parse::<u16>().ok())
                    .unwrap_or(port);
                (host, port_val)
            } else {
                (host_port, port)
            }
        } else if let Some((h, p_str)) = host_port.rsplit_once(':') {
            if h.contains(':') {
                // Multiple colons means it is an unbracketed IPv6 address
                (host_port, port)
            } else if let Ok(p_val) = p_str.parse::<u16>() {
                (h, p_val)
            } else {
                (host_port, port)
            }
        } else {
            (host_port, port)
        };

        if host.contains(':') && !host.starts_with('[') {
            format!("https://[{}]:{}{}", host, final_port, path)
        } else {
            format!("https://{}:{}{}", host, final_port, path)
        }
    }

    fn generate_nanoc() -> String {
        use rand::RngCore;
        let mut rng = rand::rngs::OsRng;
        format!("{:016x}", rng.next_u64())
    }

    fn now_timestamp() -> u64 {
        // Sprint 2 desktop upgrade: milliseconds, to match
        // `kfilesync_core::service::nonce_window::verify_and_record`'s
        // `timestamp_ms` contract used server-side.
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
    }

    /// Sprint 4: anti-replay identifiers (`device_id`/`timestamp`/`nonce`)
    /// now travel as headers instead of JSON body / query fields.
    fn anti_replay_headers(&self) -> Vec<(&'static str, String)> {
        vec![
            (headers::HEADER_DEVICE_ID, self.caller_id()),
            (headers::HEADER_TIMESTAMP, Self::now_timestamp().to_string()),
            (headers::HEADER_NONCE, Self::generate_nanoc()),
        ]
    }
}

#[async_trait]
impl NetworkClient for ReqwestNetworkClient {
    async fn send_pair_request(&self, peer_addr: &str, port: u16, body: Vec<u8>, headers: Vec<(String, String)>) -> Result<(), DomainError> {
        let url = Self::format_url(peer_addr, port, kfilesync_core::protocol::routes::PAIR_REQUEST);
        let client = self.get_client();

        let mut builder = client.post(&url)
            .header("content-type", "application/json")
            .body(body);
        for (name, value) in headers {
            builder = builder.header(name, value);
        }

        let response = builder.send().await.map_err(net_err)?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(DomainError::Network(format!("Server returned error: {}", response.status())))
        }
    }

    async fn send_pair_confirm(&self, peer_addr: &str, port: u16, body: Vec<u8>, headers: Vec<(String, String)>) -> Result<PairConfirmOutcome, DomainError> {
        let url = Self::format_url(peer_addr, port, kfilesync_core::protocol::routes::PAIR_CONFIRM);
        let client = self.get_client();

        let mut builder = client.post(&url)
            .header("content-type", "application/json")
            .body(body);
        for (name, value) in headers {
            builder = builder.header(name, value);
        }

        let response = builder.send().await.map_err(net_err)?;

        if response.status().is_success() {
            let res_dto = response.json::<core_dto::PairResultDto>().await
                .map_err(net_err)?;
            Ok(PairConfirmOutcome {
                accepted: res_dto.accepted,
                peer_certificate_pem: res_dto.peer_certificate_pem,
                reason: res_dto.reason,
            })
        } else {
            Err(DomainError::Network(format!("Server returned error: {}", response.status())))
        }
    }

    /// Enable TOFU mode — accept any cert presented by the peer. Use ONLY
    /// during a pairing flow guarded by an out-of-band PIN (see
    /// `crate::domain::port::network::BootstrapGuard`). Reference-counted:
    /// bootstrap only actually turns off once every `enable_bootstrap` call
    /// has a matching `disable_bootstrap`.
    fn enable_bootstrap(&self) {
        let prev = self.tls_config.bootstrap_refcount.fetch_add(1, Ordering::SeqCst);
        if prev == 0 {
            self.tls_config.bootstrap.store(true, Ordering::SeqCst);
        }
    }

    fn disable_bootstrap(&self) {
        let prev = self.tls_config.bootstrap_refcount.fetch_sub(1, Ordering::SeqCst);
        if prev == 1 {
            self.tls_config.bootstrap.store(false, Ordering::SeqCst);
        }
    }

    fn trust_fingerprint(&self, fingerprint: &str) {
        if let Ok(mut fps) = self.tls_config.trusted_fingerprints.lock() {
            fps.insert(fingerprint.to_string());
        }
    }

    fn untrust_fingerprint(&self, fingerprint: &str) {
        if let Ok(mut fps) = self.tls_config.trusted_fingerprints.lock() {
            fps.remove(fingerprint);
        }
    }

    async fn invite_to_share(&self, peer_ip: &str, port: u16, invite: ShareInvite) -> Result<(), DomainError> {
        let url = Self::format_url(peer_ip, port, kfilesync_core::protocol::routes::SHARE_INVITE);
        let client = self.get_client();

        let dto = core_dto::ShareInviteDto {
            share_id: invite.share_id,
            share_name: invite.share_name,
            from_device_id: invite.invited_by,
            default_permission: invite.permission,
            sync_mode: invite.sync_mode,
            invited_at_ms: Self::now_timestamp() as i64,
        };

        let mut builder = client.post(&url).json(&dto);
        for (name, value) in self.anti_replay_headers() {
            builder = builder.header(name, value);
        }

        let response = builder.send().await.map_err(net_err)?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(DomainError::Network(format!("Server returned error: {}", response.status())))
        }
    }

    async fn fetch_remote_index(&self, peer_ip: &str, port: u16, share_id: &str) -> Result<String, DomainError> {
        let url = Self::format_url(peer_ip, port, kfilesync_core::protocol::routes::SYNC_INDEX);
        let client = self.get_client();

        let mut builder = client.get(&url)
            .query(&[
                ("share_id", share_id.to_string()),
                ("since_version", "0".to_string()),
            ]);
        for (name, value) in self.anti_replay_headers() {
            builder = builder.header(name, value);
        }

        let response = builder.send().await.map_err(net_err)?;

        if response.status().is_success() {
            response.text().await.map_err(net_err)
        } else {
            Err(DomainError::Network(format!("Server returned error: {}", response.status())))
        }
    }

    async fn request_transfer(&self, peer_ip: &str, port: u16, request: crate::domain::port::network::TransferRequest) -> Result<crate::domain::port::network::TransferResponse, DomainError> {
        let url = Self::format_url(peer_ip, port, kfilesync_core::protocol::routes::TRANSFER_REQUEST);
        let client = self.get_client();

        let dto = core_dto::TransferRequestDto {
            job_id: request.job_id,
            session_id: request.session_id,
            from_device_id: request.sender_device_id,
            from_alias: self.caller_id(),
            share_id: None,
            files: request.items.iter().map(|item| core_dto::TransferItemDto {
                file_id: item.file_id.clone(),
                path: item.file_path.clone(),
                size: item.file_size,
                sha256: item.sha256.clone(),
                chunk_size: item.chunk_size,
                chunk_hashes: item.chunk_hashes.clone(),
            }).collect(),
        };

        let mut builder = client.post(&url).json(&dto);
        for (name, value) in self.anti_replay_headers() {
            builder = builder.header(name, value);
        }

        let response = builder.send().await.map_err(net_err)?;

        if response.status().is_success() {
            let res_dto = response.json::<core_dto::TransferAcceptDto>().await
                .map_err(net_err)?;
            Ok(TransferResponse {
                status: if res_dto.accepted { "accepted".to_string() } else { res_dto.reason.unwrap_or_else(|| "rejected".to_string()) },
                skip_chunks: res_dto.skip_chunks.0.iter().map(|(file_id, indices)| SkipChunkInfo {
                    file_id: file_id.clone(),
                    chunk_indices: indices.clone(),
                }).collect(),
            })
        } else {
            Err(DomainError::Network(format!("Server returned error: {}", response.status())))
        }
    }

    async fn download_chunk(&self, peer_ip: &str, port: u16, job_id: &str, file_id: &str, chunk_index: u32) -> Result<Vec<u8>, DomainError> {
        let url = Self::format_url(peer_ip, port,
            &format!("/api/lansync/v1/transfer/{}/chunk/{}/{}", job_id, file_id, chunk_index));
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

    async fn upload_chunk(&self, peer_ip: &str, port: u16, job_id: &str, file_id: &str, chunk_index: u32, chunk_hash: &str, data: Vec<u8>) -> Result<(), DomainError> {
        let url = Self::format_url(peer_ip, port,
            &format!("/api/lansync/v1/transfer/{}/chunk/{}/{}", job_id, file_id, chunk_index));
        let client = self.get_client();

        let len = data.len();
        let response = client.post(&url)
            .header(headers::HEADER_CHUNK_HASH, chunk_hash)
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
        let url = Self::format_url(peer_ip, port, kfilesync_core::protocol::routes::SHARE_LEAVE);
        let client = self.get_client();

        let dto = core_dto::ShareLeaveDto {
            share_id: share_id.to_string(),
            device_id: device_id.to_string(),
            left_at_ms: Self::now_timestamp() as i64,
        };

        let mut builder = client.post(&url).json(&dto);
        for (name, value) in self.anti_replay_headers() {
            builder = builder.header(name, value);
        }

        let response = builder.send().await.map_err(net_err)?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(DomainError::Network(format!("Server returned error: {}", response.status())))
        }
    }
}