use crate::domain::port::event_bus::DomainEvent;
use crate::domain::model::device::DeviceId;
use crate::domain::model::share::{ShareId, SharePermission};

#[derive(Debug, Clone)]
pub struct ShareCreated {
    pub share_id: ShareId,
    pub created_by: DeviceId,
}

impl DomainEvent for ShareCreated {
    fn event_type(&self) -> &str { "ShareCreated" }
    fn aggregate_id(&self) -> &str { &self.share_id.0 }
}

#[derive(Debug, Clone)]
pub struct MemberAuthorized {
    pub share_id: ShareId,
    pub device_id: DeviceId,
    pub permission: SharePermission,
}

impl DomainEvent for MemberAuthorized {
    fn event_type(&self) -> &str { "MemberAuthorized" }
    fn aggregate_id(&self) -> &str { &self.share_id.0 }
}

#[derive(Debug, Clone)]
pub struct MemberRevoked {
    pub share_id: ShareId,
    pub device_id: DeviceId,
}

impl DomainEvent for MemberRevoked {
    fn event_type(&self) -> &str { "MemberRevoked" }
    fn aggregate_id(&self) -> &str { &self.share_id.0 }
}

#[derive(Debug, Clone)]
pub struct PermissionChanged {
    pub share_id: ShareId,
    pub device_id: DeviceId,
    pub old: SharePermission,
    pub new: SharePermission,
}

impl DomainEvent for PermissionChanged {
    fn event_type(&self) -> &str { "PermissionChanged" }
    fn aggregate_id(&self) -> &str { &self.share_id.0 }
}
