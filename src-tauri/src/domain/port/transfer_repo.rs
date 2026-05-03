use crate::domain::model::transfer::{JobId, TransferJob};
use async_trait::async_trait;
use crate::domain::error::DomainError;
use crate::domain::model::device::DeviceId;

#[async_trait]
pub trait TransferRepository: Send + Sync {
    async fn find_by_id(&self, job_id: &JobId) -> Result<Option<TransferJob>, DomainError>;
    async fn save(&self, job: TransferJob) -> Result<(), DomainError>;
    async fn find_incomplete_jobs(&self) -> Result<Vec<TransferJob>, DomainError>;
    async fn find_actions_by_peer(&self, device_id: &DeviceId) -> Result<Vec<TransferJob>, DomainError>;
}
