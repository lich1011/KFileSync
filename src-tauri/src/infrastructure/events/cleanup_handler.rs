use crate::domain::model::device::DeviceId;
use crate::domain::port::event_bus::DomainEvent;
use crate::domain::port::share_repo::ShareRepository;
use crate::domain::port::transfer_repo::TransferRepository;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;

const TRUST_REVOKED_EVENT: &str = "TrustRevoked";
pub struct CascadeCleanupHandler {
    transfer_repo: Arc<dyn TransferRepository>,
    share_repo: Arc<dyn ShareRepository>,
}

impl CascadeCleanupHandler {
    pub fn new(
        transfer_repo: Arc<dyn TransferRepository>,
        share_repo: Arc<dyn ShareRepository>) -> Self {
        Self { transfer_repo, share_repo }
    }

    pub async fn start(&self, mut rx: Receiver<Arc<dyn DomainEvent>>) {
        while let Ok(event) = rx.recv().await {
            if event.event_type() == TRUST_REVOKED_EVENT {
                let device_id = DeviceId(event.aggregate_id().to_string());
                self.handle_trust_revoked(&device_id).await;
            }
        }
    }   
    
    async fn handle_trust_revoked(&self, device_id: &DeviceId) {
        //1. Cancel all active transfers to/from this device   
        match self.transfer_repo.find_actions_by_peer(device_id).await {
            Ok(jobs) => {
                for job in jobs {
                   let jid = job.job_id.clone();
                   match job.cancel()  {
                    Ok(cancelled) => {
                        if let Err(e) = self.transfer_repo.save(cancelled).await {
                            eprintln!("[CascadeCleanup] Failed to save cancelled job {}: {}", jid.0, e);
                        }   
                    },
                    Err(e) => {
                        eprintln!("[CascadeCleanup] Failed to cancel job {}: {}", jid.0, e);
                    },
                   }
                }
            },
            Err(e) => {
                eprintln!("[CascadeCleanup] Failed to query transfers for {}: {}", device_id.0, e);
            },
        }

        //2. Remove device from all shares 
        match self.share_repo.find_by_member(device_id).await {
            Ok(shares) => {
                for share in shares {
                    let sid = share.share_id.clone();
                    match share.remove_member(device_id) {
                        Ok(updated_share) => {
                            if let Err(e) = self.share_repo.save(&updated_share).await {
                                eprintln!("[CascadeCleanup] Failed to save share {}: {}", sid.0, e);
                            }
                        },
                        Err(e) => {
                            eprintln!("[CascadeCleanup] Failed to remove {} from share {}: {}", device_id.0, sid.0 , e);
                        }
                    }
                }   
            },
            Err(e) => {
                eprintln!("[CascadeCleanup] Failed to query shares for {}: {}", device_id.0, e);
            }
        }
    } 
    
    println!("[CascadeCleanup] Completed cleanup for revoked device {}",device_id.0);
    
}

