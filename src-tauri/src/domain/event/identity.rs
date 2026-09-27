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
    fn payload(&self) -> serde_json::Value {
        serde_json::json!({
            "localDevice": self.local_device.0,
            "peerDevice": self.peer_device.0,
            "pairedAt": self.paired_at
        })
    }
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

/// Sprint 5: raised when this device receives an inbound `POST
/// /pair/request` (dual-PIN OOB ceremony, ADR-010) — surfaced to the UI so
/// the local user can be shown `our_pin` to read aloud/compare against the
/// peer's screen, and prompted for the peer's own PIN.
#[derive(Debug, Clone)]
pub struct PairingRequestReceived {
    pub request_id: String,
    pub from_device_id: DeviceId,
    pub from_alias: String,
    pub our_pin: String,
}

impl DomainEvent for PairingRequestReceived {
    fn event_type(&self) -> &str { "PairingRequestReceived" }
    fn aggregate_id(&self) -> &str { &self.from_device_id.0 }
    fn payload(&self) -> serde_json::Value {
        serde_json::json!({
            "requestId": self.request_id,
            "fromDeviceId": self.from_device_id.0,
            "fromAlias": self.from_alias,
            "ourPin": self.our_pin
        })
    }
}