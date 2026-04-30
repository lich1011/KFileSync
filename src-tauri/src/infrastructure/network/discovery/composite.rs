use crate::domain::port::discovery::{DiscoveryProvider, DeviceInfo, DiscoveredDevice};
use async_trait::async_trait;
use tokio::sync::mpsc::Sender;

#[async_trait]
pub trait DiscoveryStrategy: Send + Sync {
    fn name(&self) -> &str;
    fn priority(&self) -> u8; // 1=mDNS, 2=HTTP, 3=Manual
    async fn announce(&self, info: &DeviceInfo) -> Result<(), String>;
    async fn discover(&self, tx: Sender<DiscoveredDevice>) -> Result<(), String>;
    async fn stop(&self) -> Result<(), String>;
}

pub struct CompositeDiscovery {
    strategies: Vec<Box<dyn DiscoveryStrategy>>,
}

impl CompositeDiscovery {
    pub fn new(mut strategies: Vec<Box<dyn DiscoveryStrategy>>) -> Self {
        strategies.sort_by_key(|s| s.priority());
        Self { strategies }
    }
}

#[async_trait]
impl DiscoveryProvider for CompositeDiscovery {
    async fn announce(&self, info: &DeviceInfo) -> Result<(), String> {
        for strategy in &self.strategies {
            if let Err(e) = strategy.announce(info).await {
                eprintln!("[CompositeDiscovery] Strategy {} failed to announce: {}", strategy.name(), e);
            } else {
                return Ok(()); // First successful strategy wins for announce
            }
        }
        Err("All discovery strategies failed to announce".to_string())
    }

    async fn listen(&self, tx: Sender<DiscoveredDevice>) -> Result<(), String> {
        for strategy in &self.strategies {
            let tx_clone = tx.clone();
            let name = strategy.name().to_string();
            // Fire and forget listening for all strategies to gather devices from multiple sources
            // or we could fall back. But typically we want to listen on all available.
            // Wait, the design doc says: "按 priority 依次尝试，失败则降级到下一个。"
            // Let's implement fallback:
            if let Err(e) = strategy.discover(tx_clone).await {
                eprintln!("[CompositeDiscovery] Strategy {} failed to listen: {}. Falling back...", name, e);
            } else {
                return Ok(()); // Listening successfully started
            }
        }
        Err("All discovery strategies failed to listen".to_string())
    }

    async fn stop(&self) -> Result<(), String> {
        for strategy in &self.strategies {
            let _ = strategy.stop().await;
        }
        Ok(())
    }
}
