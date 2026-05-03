use crate::domain::model::device::{Device, DeviceId, DeviceState};
use crate::domain::error::DomainError;

use async_trait::async_trait;

#[async_trait]
pub trait DeviceRepository: Send + Sync {
    async fn find_by_id(&self, id: DeviceId) -> Result<Option<Device>, DomainError>;
    async fn find_paired(&self) -> Result<Vec<Device>, DomainError>;
    async fn save(&self, device: Device) -> Result<(), DomainError>;
    async fn update_trust_status(&self, id: DeviceId, status: DeviceState) -> Result<(), DomainError>;
}
