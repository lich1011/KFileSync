use crate::domain::model::device::{Device, DeviceId, DeviceState, DiscoveredData, Certificate};
use crate::domain::model::pairing::PairingSession;
use crate::domain::port::repository::DeviceRepository;
use crate::domain::port::key_store::KeyStore;
use crate::domain::port::discovery::{DiscoveryProvider, DiscoveredDevice};
use crate::domain::port::event_bus::EventBus;
use crate::domain::port::network::{NetworkClient, PairingRequest};
use crate::domain::error::DomainError;
use crate::domain::event::identity::{DeviceDiscovered, PairingCompleted, TrustRevoked};
use crate::infrastructure::security::keystore::fingerprint_short;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Mutex;
use std::collections::HashMap;

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

    fn sweep_expried_sessions(&self) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
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
        let _ = self.discovery.stop().await;

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
                let _ = self.repo.save(new_dev).await;
            }
        }
        
        Ok(devices)
    }

    pub async fn initiate_pairing(&self, target: &DeviceId) -> Result<PairingSession, DomainError> {
        self.sweep_expried_sessions();

        let device = self.repo.find_by_id(target.clone()).await?
            .ok_or_else(|| DomainError::DeviceNotFound(target.0.clone()))?;
            
        let address = match &device.state {
            DeviceState::Discovered(data) => data.address.clone(),
            _ => return Err(DomainError::InvalidStateTransition("Device is not in Discovered state")),
        };

        // Create pairing session
        let pin = format!("{:06}", rand::random::<u32>() % 1000000);
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let expires_at = timestamp + 300;
        let session = PairingSession::new(target.clone(), pin.clone(), expires_at);
        
        let req = PairingRequest {
            device_id: self.local_device_id.0.clone(),
            alias: self.local_alias.clone(),
            platform: std::env::consts::OS.to_string(),
            fingerprint: fingerprint_short(&self.local_device_id.0),
        };
        
        // Send pairing request via network client
        self.network_client.request_pairing(&address, crate::DEFAULT_PORT, req).await?;
        
        // Save session in memory only if request succeeded
        self.active_sessions.lock().unwrap().insert(target.clone(), session.clone());
        
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
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
       
        let verify_result ={
            let mut sessions = self.active_sessions.lock().unwrap();
            let session = sessions.get_mut(target_device_id)
                .ok_or_else(|| DomainError::NotFound(format!("No active pairing session for device {}", target_device_id.0)))?;
            
            let result = session.verify(pin_code, current_time);

            if result.is_ok() || session.attempts >= session.max_attempts{
                let target = session.target_device.clone();
                let _ = session;
                self.active_sessions.lock().unwrap().remove(&target);
                result
            } else {
                result
            }

        };

        verify_result?;

        let device = self.repo.find_by_id(target_device_id.clone()).await?
            .ok_or_else(|| DomainError::DeviceNotFound(target_device_id.0.clone()))?;

        let cert = Certificate::from_pem(cert_pem)?;
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
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
        let device = self.repo.find_by_id(device_id.clone()).await?
            .ok_or_else(|| DomainError::DeviceNotFound(device_id.0.clone()))?;

        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
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
}
