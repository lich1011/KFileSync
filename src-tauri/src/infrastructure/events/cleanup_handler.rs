use crate::domain::port::event_bus::DomainEvent;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;

pub struct CascadeCleanupHandler {
    // 依赖：ShareService, TransferService 的抽象接口或其队列等。
    // 在这里由于我们在基础建设阶段，先用 mock，并在控制台打印
}

impl CascadeCleanupHandler {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for CascadeCleanupHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl CascadeCleanupHandler {
    pub async fn start(&self, mut rx: Receiver<Arc<dyn DomainEvent>>) {
        while let Ok(event) = rx.recv().await {
            if event.event_type() == "TrustRevoked" {
                println!(
                    "[CascadeCleanup] Trust revoked for device: {}. Invoking cleanup tasks: \
                    1. Stop ongoing transfers. \
                    2. Remove from shared folders. \
                    3. Terminate TLS connection.",
                    event.aggregate_id()
                );
                // TODO: call actual services here when Transfer and Share context are implemented.
            }
        }
    }
}
