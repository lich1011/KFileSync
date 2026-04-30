use crate::domain::model::transfer::{JobId, TransferJob};
use async_trait::async_trait;

#[async_trait]
pub trait TransferRepository: Send + Sync {
    async fn find_by_id(&self, job_id: &JobId) -> Result<Option<TransferJob>, String>;
    async fn save(&self, job: TransferJob) -> Result<(), String>;
    async fn find_incomplete_jobs(&self) -> Result<Vec<TransferJob>, String>;
}
