use async_trait::async_trait;
use crate::domain::model::share::{Share, ShareId};
use crate::domain::model::device::DeviceId;
use crate::domain::error::DomainError;

#[async_trait]
pub trait ShareRepository: Send + Sync {
    async fn save(&self , share: &Share) -> Result<(), DomainError>;
    async fn find_by_id(&self, id: &ShareId) -> Result<Option<Share>, DomainError>;
    async fn find_by_member(&self, device_id: &DeviceId) -> Result<Vec<Share>, DomainError>;
    async fn find_all(&self) -> Result<Vec<Share>, DomainError>;
    
}
