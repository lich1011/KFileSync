use std::sync::Arc;
use crate::domain::model::file_entry::BlockInfo;
use crate::domain::model::share::ShareId;
use crate::domain::port::file_index_repo::{FileIndexRepository, LocalBlockCopy};
use crate::domain::error::DomainError;

pub struct DeduplicationResult {
    pub local_copies: Vec<LocalBlockCopy>,
    pub network_fetches: Vec<BlockInfo>,
}

pub struct BlockDeduplicator {
    file_index_repo: Arc<dyn FileIndexRepository>,
}

impl BlockDeduplicator {
    pub fn new(file_index_repo: Arc<dyn FileIndexRepository>) -> Self {
        Self { file_index_repo }
    }

    /// Deduplicates a list of needed blocks against the local file index
    pub async fn deduplicate(&self, share_id: &ShareId, needed_blocks: &[BlockInfo]) -> Result<DeduplicationResult, DomainError> {
        let mut local_copies = Vec::new();
        let mut network_fetches = Vec::new();

        for block in needed_blocks {
            // Find if we already have a block with this hash in this share
            let copies = self.file_index_repo.find_blocks_by_hash(share_id, &block.hash).await?;
            
            if let Some(first_copy) = copies.into_iter().next() {
                // We found a local copy, we can just copy it directly
                local_copies.push(first_copy);
            } else {
                // We don't have it locally, need to fetch from network
                network_fetches.push(block.clone());
            }
        }

        Ok(DeduplicationResult {
            local_copies,
            network_fetches,
        })
    }
}
