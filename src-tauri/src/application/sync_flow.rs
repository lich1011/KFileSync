use async_trait::async_trait;
use crate::domain::error::DomainError;
use crate::domain::model::device::DeviceId;
use crate::domain::model::file_entry::{FileEntry, SyncPlan};
use crate::domain::model::share::ShareId;

#[async_trait]
pub trait SyncFlowTemplate {
    /// Template method for synchronization execution flow
    async fn execute(&self, share_id: &ShareId, peer: &DeviceId) -> Result<SyncPlan, DomainError> {
        // Steps are ordered so that partial failure is safe: if execute_plan fails,
        // version are not updated and the paln can be re-executed on next sync cycle.
        self.verify_permission(share_id, peer).await?;
        
        // 1. Fetch remote index
        let remote_index = self.fetch_remote_index(share_id, peer).await?;
        
        // 2. Generate plan
        let plan = self.generate_plan(share_id, peer, &remote_index).await?;
        
        // 3. Execute plan
        self.execute_plan(&plan, peer).await?;
        
        // 4. Update versions and resolve conflicts
        self.update_versions(share_id, &plan).await?;
        
        // 5. Emit events
        self.emit_events(&plan).await?;
        
        Ok(plan)
    }

    // Abstract methods to be implemented by concrete classes
    async fn verify_permission(&self, share_id: &ShareId, peer: &DeviceId) -> Result<(), DomainError>;
    
    // In Phase 4, network fetching is mocked or returns empty/simulated data
    async fn fetch_remote_index(&self, share_id: &ShareId, peer: &DeviceId) -> Result<Vec<FileEntry>, DomainError>;
    
    async fn generate_plan(&self, share_id: &ShareId, peer: &DeviceId, remote_index: &[FileEntry]) -> Result<SyncPlan, DomainError>;
    
    async fn execute_plan(&self, plan: &SyncPlan, peer: &DeviceId) -> Result<(), DomainError>;
    
    async fn update_versions(&self, share_id: &ShareId, plan: &SyncPlan) -> Result<(), DomainError>;
    
    async fn emit_events(&self, plan: &SyncPlan) -> Result<(), DomainError>;
}
