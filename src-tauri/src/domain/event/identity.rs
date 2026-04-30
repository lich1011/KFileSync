use crate::domain::port::event_bus::DomainEvent;
use crate::domain::model::device::DeviceId;

#[derive(Debug, Clone)]
pub struct DeviceDiscovered {
    pub device_id: DeviceId,
    pub alias: String,
}

impl DomainEvent for DeviceDiscovered {
    fn event_type(&self) -> &str { "DeviceDiscovered" }
    fn aggregate_id(&self) -> &str { &self.device_id.0 }
}

#[derive(Debug, Clone)]
pub struct PairingCompleted {
    pub local_device: DeviceId,
    pub peer_device: DeviceId,
    pub paired_at: u64,
}

impl DomainEvent for PairingCompleted {
    fn event_type(&self) -> &str { "PairingCompleted" }
    fn aggregate_id(&self) -> &str { &self.peer_device.0 }
}

#[derive(Debug, Clone)]
pub struct TrustRevoked {
    pub device_id: DeviceId,
    pub revoked_at: u64,
}

impl DomainEvent for TrustRevoked {
    fn event_type(&self) -> &str { "TrustRevoked" }
    fn aggregate_id(&self) -> &str { &self.device_id.0 }
}
