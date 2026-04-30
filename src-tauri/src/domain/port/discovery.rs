use crate::domain::model::device::DeviceId;
use async_trait::async_trait;
use tokio::sync::mpsc::Sender;

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub device_id: DeviceId,
    pub alias: String,
    pub ip: String,
    pub port: u16,
}

#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub device_id: DeviceId,
    pub alias: String,
    pub address: String,
}

#[async_trait]
pub trait DiscoveryProvider: Send + Sync {
    async fn announce(&self, info: &DeviceInfo) -> Result<(), String>;
    async fn listen(&self, tx: Sender<DiscoveredDevice>) -> Result<(), String>;
    async fn stop(&self) -> Result<(), String>;
}
