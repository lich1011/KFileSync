use crate::domain::error::DomainError;
use crate::domain::event::identity::{DeviceDiscovered, PairingCompleted, TrustRevoked};
use crate::domain::model::device::{Certificate, Device, DeviceId, DeviceState, DiscoveredData};
use crate::domain::model::pairing::PairingSessionStore;
use crate::domain::port::discovery::{DiscoveredDevice, DiscoveryProvider};
use crate::domain::port::event_bus::EventBus;
use crate::domain::port::key_store::KeyStore;
use crate::domain::port::network::{BootstrapGuard, NetworkClient};
use crate::domain::port::repository::DeviceRepository;
use kfilesync_core::domain::SecretPin;
use kfilesync_core::trust::pairing_state;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

const PAIRING_MAX_ATTEMPTS: u32 = 3;
const PAIRING_PIN_TTL_MS: i64 = 5 * 60 * 1000;

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn generate_pin() -> String {
    use rand::Rng;
    let mut rng = rand::rngs::OsRng;
    format!("{:06}", rng.gen_range(0..1_000_000u32))
}

fn generate_nonce() -> String {
    use rand::RngCore;
    let mut rng = rand::rngs::OsRng;
    format!("{:016x}", rng.next_u64())
}

#[allow(dead_code)]
pub struct DeviceAppService {
    local_device_id: DeviceId,
    local_alias: String,
    /// This device's own certificate, PEM-encoded — sent to the peer as part
    /// of `POST /pair/confirm` (`PairConfirmDto::certificate_pem`), replacing
    /// the old flow's user-supplied cert PEM.
    local_cert_pem: String,
    repo: Arc<dyn DeviceRepository>,
    discovery: Arc<dyn DiscoveryProvider>,
    key_store: Arc<dyn KeyStore>,
    network_client: Arc<dyn NetworkClient>,
    event_bus: Arc<dyn EventBus>,
    /// Shared with the inbound HTTP server (`http_server::ServerAppState`) —
    /// a session created by either role must be reachable regardless of
    /// which side created it.
    sessions: Arc<PairingSessionStore>,
}

impl DeviceAppService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        local_device_id: DeviceId,
        local_alias: String,
        local_cert_pem: String,
        repo: Arc<dyn DeviceRepository>,
        discovery: Arc<dyn DiscoveryProvider>,
        key_store: Arc<dyn KeyStore>,
        network_client: Arc<dyn NetworkClient>,
        event_bus: Arc<dyn EventBus>,
        sessions: Arc<PairingSessionStore>,
    ) -> Self {
        Self {
            local_device_id,
            local_alias,
            local_cert_pem,
            repo,
            discovery,
            key_store,
            network_client,
            event_bus,
            sessions,
        }
    }

    pub async fn discover_devices(&self) -> Result<Vec<DiscoveredDevice>, String> {
        let (tx, mut rx) = mpsc::channel(100);
        let discovery = self.discovery.clone();

        let listener_handle = tokio::spawn(async move {
            if let Err(e) = discovery.listen(tx).await {
                eprintln!("[DeviceAppService] Listen error: {}", e);
            }
        });

        // Let it listen for 2 seconds and collect results
        let mut devices = Vec::new();
        let timeout = sleep(Duration::from_secs(2));
        tokio::pin!(timeout);

        loop {
            tokio::select! {
                Some(device) = rx.recv() => {
                    devices.push(device);
                }
                _ = &mut timeout => {
                    break;
                }
            }
        }

        // Clean up: abort the listener task to prevent resource leak
        listener_handle.abort();
        if let Err(e) = self.discovery.stop().await {
            eprintln!("[DeviceAppService] Failed to stop discovery: {}", e);
        }

        // Fire DomainEvent for discovered devices
        for dev in &devices {
            self.event_bus.publish(Box::new(DeviceDiscovered {
                device_id: dev.device_id.clone(),
                alias: dev.alias.clone(),
            }));

            // Try to store as discovered if it doesn't exist
            if let Ok(None) = self.repo.find_by_id(dev.device_id.clone()).await {
                let new_dev = Device {
                    id: dev.device_id.clone(),
                    state: DeviceState::Discovered(DiscoveredData {
                        alias: dev.alias.clone(),
                        address: dev.address.clone(),
                    }),
                };
                if let Err(e) = self.repo.save(new_dev).await {
                    eprintln!(
                        "[DeviceAppService] Failed to persist discovered device: {}",
                        e
                    );
                }
            }
        }

        Ok(devices)
    }

    /// Initiate the dual-PIN OOB pairing ceremony (ADR-010) with `target`.
    /// Returns the local PIN to show the user — they read it aloud (or
    /// compare on-screen) so the peer's user can type it into their own
    /// `confirm_pairing` call.
    pub async fn initiate_pairing(&self, target: &DeviceId) -> Result<String, DomainError> {
        self.sessions.sweep_expired(now_ms());

        let device = self
            .repo
            .find_by_id(target.clone())
            .await?
            .ok_or_else(|| DomainError::DeviceNotFound(target.0.clone()))?;

        let address = match &device.state {
            DeviceState::Discovered(data) => data.address.clone(),
            _ => {
                return Err(DomainError::InvalidStateTransition(
                    "Device is not in Discovered state",
                ))
            }
        };

        let request_id = Uuid::new_v4().to_string();
        let nonce = generate_nonce();
        let our_pin = generate_pin();

        let kfilesync_core::trust::pairing_state::StartOutboundResult { session, request: prepared } = pairing_state::start_outbound(
            &request_id,
            &nonce,
            &target.0,
            &self.local_device_id.0,
            &self.local_alias,
            std::env::consts::OS,
            // Desktop's device_id IS the SHA-256 hex of its own cert DER, so
            // it doubles as the fingerprint here (Sprint 4 convention).
            &self.local_device_id.0,
            SecretPin::new(our_pin.clone()),
            PAIRING_MAX_ATTEMPTS,
            now_ms(),
            PAIRING_PIN_TTL_MS,
        )
        .map_err(|e| DomainError::Network(format!("{:?}", e)))?;

        self.sessions.insert(session);

        let guard = BootstrapGuard::new(self.network_client.clone());
        let headers = prepared.headers.into_iter().map(|h| (h.name, h.value)).collect();
        let result = self
            .network_client
            .send_pair_request(&address, crate::DEFAULT_PORT, prepared.body, headers)
            .await;
        drop(guard);

        if let Err(e) = result {
            self.sessions.remove(&request_id);
            return Err(e);
        }

        Ok(our_pin)
    }

    /// Complete the dual-PIN OOB ceremony: record the PIN the local user read
    /// off the peer's screen, send `POST /pair/confirm`, and — if the peer
    /// accepts — mark the peer device Paired using the certificate they
    /// return in the same response.
    pub async fn confirm_pairing(
        &self,
        target_device_id: &DeviceId,
        peer_pin: &str,
    ) -> Result<(), DomainError> {
        let request_id = self
            .sessions
            .find_request_id_by_device(&target_device_id.0)
            .ok_or_else(|| {
                DomainError::NotFound(format!(
                    "No active pairing session for device {}",
                    target_device_id.0
                ))
            })?;

        self.sessions.with_mut(&request_id, |session| {
            pairing_state::record_peer_pin(session, peer_pin);
        });

        let session = self
            .sessions
            .get_clone(&request_id)
            .ok_or_else(|| DomainError::NotFound("pairing session vanished".to_string()))?;

        let device = self
            .repo
            .find_by_id(target_device_id.clone())
            .await?
            .ok_or_else(|| DomainError::DeviceNotFound(target_device_id.0.clone()))?;

        let address = match &device.state {
            DeviceState::Discovered(data) => data.address.clone(),
            _ => {
                return Err(DomainError::InvalidStateTransition(
                    "Device is not in Discovered state",
                ))
            }
        };

        let nonce = generate_nonce();
        let prepared = match pairing_state::prepare_confirm(
            &session,
            &self.local_device_id.0,
            now_ms(),
            &nonce,
            &self.local_cert_pem,
        ) {
            pairing_state::PrepareConfirmOutcome::Ready(req) => req,
            pairing_state::PrepareConfirmOutcome::NotReady => {
                return Err(DomainError::BusinessRuleViolation(
                    "peer PIN was not recorded before confirming".to_string(),
                ));
            }
            pairing_state::PrepareConfirmOutcome::EncodeFailed(err) => {
                return Err(DomainError::Network(err));
            }
        };

        let guard = BootstrapGuard::new(self.network_client.clone());
        let headers = prepared.headers.into_iter().map(|h| (h.name, h.value)).collect();
        let outcome = self
            .network_client
            .send_pair_confirm(&address, crate::DEFAULT_PORT, prepared.body, headers)
            .await;
        drop(guard);

        // Sprint 5 scope decision: each `confirm_pairing` call is a single
        // attempt — a rejection (wrong PIN, expired, etc.) drops the local
        // session, and the user must re-run `initiate_pairing` to retry.
        // Retry-attempt bookkeeping lives entirely in the *responder's*
        // session (`PairingSession::attempts`, enforced server-side by
        // `verify_peer_pin`), which this device does not have local access
        // to renegotiate.
        self.sessions.remove(&request_id);

        let outcome = outcome?;
        if !outcome.accepted {
            return Err(DomainError::InvalidPinCode);
        }

        let cert_pem = outcome.peer_certificate_pem.ok_or_else(|| {
            DomainError::Network("peer accepted pairing but sent no certificate".to_string())
        })?;
        let cert = Certificate::from_pem(cert_pem)?;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let updated_state = device.state.confirm_pairing(cert, timestamp)?;

        let updated_device = Device {
            id: device.id.clone(),
            state: updated_state,
        };
        self.repo.save(updated_device).await?;

        self.network_client.trust_fingerprint(&target_device_id.0);

        self.event_bus.publish(Box::new(PairingCompleted {
            local_device: self.local_device_id.clone(),
            peer_device: target_device_id.clone(),
            paired_at: timestamp,
        }));

        Ok(())
    }

    pub async fn revoke_trust(&self, device_id: &DeviceId) -> Result<(), DomainError> {
        let device = self
            .repo
            .find_by_id(device_id.clone())
            .await?
            .ok_or_else(|| DomainError::DeviceNotFound(device_id.0.clone()))?;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let updated_state = device.state.revoke(timestamp)?;

        let updated_device = Device {
            id: device.id.clone(),
            state: updated_state,
        };

        self.repo.save(updated_device).await?;
        self.network_client.untrust_fingerprint(&device_id.0);

        self.event_bus.publish(Box::new(TrustRevoked {
            device_id: device_id.clone(),
            revoked_at: timestamp,
        }));

        Ok(())
    }

    /// Reject a pending pairing request from a device that's still in Discovered state
    /// (or any non-Paired state). This removes the pairing session and drops the device
    /// from local storage so the user can re-attempt later.
    pub async fn reject_pairing(&self, device_id: &DeviceId) -> Result<(), DomainError> {
        // Drop any in-flight pairing session for this device
        if let Some(request_id) = self.sessions.find_request_id_by_device(&device_id.0) {
            self.sessions.remove(&request_id);
        }

        // Look up the device and decide how to clear it
        let device = match self.repo.find_by_id(device_id.clone()).await? {
            Some(d) => d,
            None => return Ok(()), // nothing to clean up
        };

        match &device.state {
            crate::domain::model::device::DeviceState::Paired(_) => {
                // Already paired — go through proper revoke flow
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let updated_state = device.state.revoke(timestamp)?;
                let updated_device = Device {
                    id: device.id.clone(),
                    state: updated_state,
                };
                self.repo.save(updated_device).await?;
                self.event_bus.publish(Box::new(TrustRevoked {
                    device_id: device_id.clone(),
                    revoked_at: timestamp,
                }));
            }
            _ => {
                // Discovered or already Revoked — nothing security-sensitive to do.
                // We leave the row in place so the device can be re-discovered cleanly.
            }
        }

        Ok(())
    }
}