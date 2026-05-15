use crate::infrastructure::network::discovery::composite::DiscoveryStrategy;
use crate::domain::port::discovery::DiscoveredDevice;
use crate::domain::model::device::DeviceId;
use crate::domain::error::DomainError;
use async_trait::async_trait;
use tokio::sync::mpsc::Sender;
use std::sync::Mutex;

pub struct ManualIpStrategy {
    ips: Mutex<Vec<String>>,
}

impl ManualIpStrategy {
    pub fn new() -> Self {
        Self {
            ips: Mutex::new(Vec::new()),
        }
    }

    pub fn add_ip(&self, ip: String) {
        let mut ips = self.ips.lock().unwrap();
        if !ips.contains(&ip) {
            ips.push(ip);
        }
    }
}

#[async_trait]
impl DiscoveryStrategy for ManualIpStrategy {
    fn name(&self) -> &str { "Manual IP" }
    fn priority(&self) -> u8 { 3 }

    async fn announce(&self, _info: &crate::domain::port::discovery::DeviceInfo) -> Result<(), DomainError> {
        Ok(())
    }

    async fn discover(&self, tx: Sender<DiscoveredDevice>) -> Result<(), DomainError> {
        let ips: Vec<String> = self.ips.lock().unwrap().clone();
        let port = crate::DEFAULT_PORT;

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .map_err(|e| DomainError::Network(e.to_string()))?;

        tokio::spawn(async move {
            for ip in ips {
                let url = format!("https://{}:{}/api/lansync/v1/info", ip, port);
                if let Ok(resp) = client.get(&url).send().await {
                    if let Ok(info) = resp.json::<DeviceInfoResponse>().await {
                        let _ = tx.send(DiscoveredDevice {
                            device_id: DeviceId(info.device_id),
                            alias: info.alias,
                            address: ip,
                        }).await;
                    }
                }
            }
        });

        Ok(())
    }

    async fn stop(&self) -> Result<(), DomainError> {
        Ok(())
    }
}

#[derive(serde::Deserialize)]
struct DeviceInfoResponse {
    device_id: String,
    alias: String,
}