use async_trait::async_trait;
use crate::domain::model::share::{Share, ShareId};
use crate::domain::model::device::DeviceId;

#[async_trait]
pub trait ShareRepository: Send + Sync {
    async fn save(&self, share: &Share) -> Result<(), String>;
    async fn find_by_id(&self, id: &ShareId) -> Result<Option<Share>, String>;
    async fn find_by_member(&self, device_id: &DeviceId) -> Result<Vec<Share>, String>;
    async fn find_all(&self) -> Result<Vec<Share>, String>;
}
