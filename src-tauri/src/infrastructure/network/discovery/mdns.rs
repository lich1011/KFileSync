use crate::infrastructure::network::discovery::composite::DiscoveryStrategy;
use crate::domain::port::discovery::{DeviceInfo, DiscoveredDevice};
use crate::domain::model::device::DeviceId;
use crate::domain::error::DomainError;
use async_trait::async_trait;
use tokio::sync::mpsc::Sender;
use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::collections::HashMap;

const SERVICE_TYPE: &str = "_lansync._tcp.local.";

fn net_err(e: impl std::fmt::Display) -> DomainError {
    DomainError::Network(e.to_string())
}

pub struct MdnsStrategy {
    mdns: ServiceDaemon,
}

impl MdnsStrategy {
    pub fn new() -> Self {
        Self {
            mdns: ServiceDaemon::new().expect("Failed to create mDNS daemon"),
        }
    }
}

impl Default for MdnsStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DiscoveryStrategy for MdnsStrategy {
    fn name(&self) -> &str { "mDNS" }
    fn priority(&self) -> u8 { 1 }

    async fn announce(&self, info: &DeviceInfo) -> Result<(), DomainError> {
        let instance_name = info.alias.clone();
        let host_name = format!("{}.local.", info.alias.replace(" ", "-").to_lowercase());
        let port = info.port;
        let mut properties = HashMap::new();
        properties.insert("device_id".to_string(), info.device_id.0.clone());

        let service_info = ServiceInfo::new(
            SERVICE_TYPE,
            &instance_name,
            &host_name,
            info.ip.clone(),
            port,
            Some(properties),
        ).map_err(net_err)?;

        self.mdns.register(service_info).map_err(net_err)?;
        Ok(())
    }

    async fn discover(&self, tx: Sender<DiscoveredDevice>) -> Result<(), DomainError> {
        let receiver = self.mdns.browse(SERVICE_TYPE).map_err(net_err)?;
        
        tokio::spawn(async move {
            while let Ok(event) = receiver.recv_async().await {
                if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
                    let device_id_str = info.get_property_val_str("device_id").unwrap_or("unknown");

                    let fullname = info.get_fullname();
                    let alias = fullname
                    .strip_suffix(&format!(".{}", SERVICE_TYPE))
                    .unwrap_or(fullname)
                    .to_string();

                    let ip = info.get_addresses().iter().next().map(|ip| ip.to_string()).unwrap_or_default();
                    
                    let device = DiscoveredDevice {
                        device_id: DeviceId(device_id_str.to_string()),
                        alias,
                        address: ip,
                    };
                    let _ = tx.send(device).await;
                }
            }
        });
        Ok(())
    }

    async fn stop(&self) -> Result<(), DomainError> {
        // mdns_sd shutdown is implicit when daemon is dropped, or we can just ignore.
        self.mdns.stop_browse(SERVICE_TYPE).map_err(net_err)?;
        Ok(())
    }
}
