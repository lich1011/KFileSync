use crate::domain::error::DomainError;
use crate::domain::event::identity::{DeviceDiscovered, PairingCompleted, TrustRevoked};
use crate::domain::model::device::{Certificate, Device, DeviceId, DeviceState, DiscoveredData};
use crate::domain::model::pairing::PairingSession;
use crate::domain::port::discovery::{DiscoveredDevice, DiscoveryProvider};
use crate::domain::port::event_bus::EventBus;
use crate::domain::port::key_store::KeyStore;
use crate::domain::port::network::{NetworkClient, PairingRequest};
use crate::domain::port::repository::DeviceRepository;
use crate::infrastructure::security::keystore::fingerprint_short;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

#[allow(dead_code)]
pub struct DeviceAppService {
    local_device_id: DeviceId,
    local_alias: String,
    repo: Arc<dyn DeviceRepository>,
    discovery: Arc<dyn DiscoveryProvider>,
    key_store: Arc<dyn KeyStore>,
    network_client: Arc<dyn NetworkClient>,
    event_bus: Arc<dyn EventBus>,
    active_sessions: Mutex<HashMap<DeviceId, PairingSession>>,
}

impl DeviceAppService {
    pub fn new(
        local_device_id: DeviceId,
        local_alias: String,
        repo: Arc<dyn DeviceRepository>,
        discovery: Arc<dyn DiscoveryProvider>,
        key_store: Arc<dyn KeyStore>,
        network_client: Arc<dyn NetworkClient>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            local_device_id,
            local_alias,
            repo,
            discovery,
            key_store,
            network_client,
            event_bus,
            active_sessions: Mutex::new(HashMap::new()),
        }
    }

    fn sweep_expired_sessions(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut sessions = self.active_sessions.lock().unwrap();
        sessions.retain(|_, session| session.expires_at > now);
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

    pub async fn initiate_pairing(&self, target: &DeviceId) -> Result<PairingSession, DomainError> {
        self.sweep_expired_sessions();

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

        // Create pairing session
        let pin = {
            use rand::Rng;

            let mut rng = rand::rngs::OsRng;
            format!("{:06}", rng.gen_range(0..1_000_000u32))
        };
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expires_at = timestamp + 300;
        let session = PairingSession::new(target.clone(), pin.clone(), expires_at);

        let req = PairingRequest {
            device_id: self.local_device_id.0.clone(),
            alias: self.local_alias.clone(),
            platform: std::env::consts::OS.to_string(),
            fingerprint: fingerprint_short(&self.local_device_id.0),
        };

        // Send pairing request via network client
        self.network_client
            .request_pairing(&address, crate::DEFAULT_PORT, req)
            .await?;

        // Save session in memory only if request succeeded
        self.active_sessions
            .lock()
            .unwrap()
            .insert(target.clone(), session.clone());

        Ok(session)
    }

    /// 确认配对：验证 PIN，然后将对方设备状态从 Discovered 升级到 Paired。
    /// # Arguments
    /// * `target_device_id` - 目标设备 ID，用于查找对应的配对会话
    /// * `pin_code` - 用户输入的 PIN 码
    /// * `cert_pem` - 对方设备的 PEM 格式证书
    pub async fn confirm_pairing(
        &self,
        target_device_id: &DeviceId,
        pin_code: &str,
        cert_pem: String,
    ) -> Result<(), DomainError> {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let verify_result = {
            let mut sessions = self.active_sessions.lock().unwrap();
            let session = sessions.get_mut(target_device_id).ok_or_else(|| {
                DomainError::NotFound(format!(
                    "No active pairing session for device {}",
                    target_device_id.0
                ))
            })?;

            let result = session.verify(pin_code, current_time);

            if result.is_ok() || session.attempts >= session.max_attempts {
                // Remove in the same lock scope — no double-lock, no deadlock
                sessions.remove(target_device_id);
            }
            result
            // MutexGuard `sessions` is dropped here
        };

        verify_result?;

        let device = self
            .repo
            .find_by_id(target_device_id.clone())
            .await?
            .ok_or_else(|| DomainError::DeviceNotFound(target_device_id.0.clone()))?;

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
        {
            let mut sessions = self.active_sessions.lock().unwrap();
            sessions.remove(device_id);
        }

        // Look up the device and decide how to clear it
        let device = match self.repo.find_by_id(device_id.clone()).await? {
            Some(d) => d,
            None => return Ok(()), // nothing to clean up
        };

        match &device.state {
            crate::domain::model::device::DeviceState::Paired(_) => {
                // Already paired – go through proper revoke flow
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
                // Discovered or already Revoked – nothing security-sensitive to do.
                // We leave the row in place so the device can be re-discovered cleanly.
            }
        }

        Ok(())
    }
}
