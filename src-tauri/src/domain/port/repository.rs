use crate::domain::model::device::{Device, DeviceId, DeviceState};

use async_trait::async_trait;

#[async_trait]
pub trait DeviceRepository: Send + Sync {
    async fn find_by_id(&self, id: DeviceId) -> Result<Option<Device>, String>;
    async fn find_paired(&self) -> Result<Vec<Device>, String>;
    async fn save(&self, device: Device) -> Result<(), String>;
    async fn update_trust_status(&self, id: DeviceId, status: DeviceState) -> Result<(), String>;
}
