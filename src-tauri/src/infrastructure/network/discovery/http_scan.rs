use crate::infrastructure::network::discovery::composite::DiscoveryStrategy;
use crate::domain::port::discovery::DiscoveredDevice;
use crate::domain::model::device::DeviceId;
use crate::domain::error::DomainError;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use std::net::IpAddr;

// 获取本地 IP 地址
fn get_local_ip() -> Option<IpAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|a| a.ip())
}

// 生成子网内所有可能的 IP 地址（排除自身）
fn subnet_ips(local_ip: IpAddr) -> Vec<IpAddr> {
    match local_ip {
        IpAddr::V4(ip) => {
            let octets = ip.octets();
            (1..=254)
                .filter(|&i| i != octets[3])
                .map(|i| IpAddr::V4(std::net::Ipv4Addr::new(octets[0], octets[1], octets[2], i)))
                .collect()
        }
        _ => Vec::new(),
    }
}

pub struct HttpScanStrategy;

impl HttpScanStrategy {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl DiscoveryStrategy for HttpScanStrategy {
    fn name(&self) -> &str { "HTTP Scan" } // 策略名称
    fn priority(&self) -> u8 { 2 }        // 优先级

    async fn announce(&self, _info: &crate::domain::port::discovery::DeviceInfo) -> Result<(), DomainError> {
        Ok(())
    }

    // 执行设备发现逻辑
    async fn discover(&self, tx: Sender<DiscoveredDevice>) -> Result<(), DomainError> {
        let local_ip = get_local_ip()
            .ok_or_else(|| DomainError::Network("Cannot determine local IP.".into()))?;
        let ips = subnet_ips(local_ip);
        let port = crate::DEFAULT_PORT;

        // 配置 HTTP 客户端，允许无效证书并设置 500ms 超时
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_millis(500))
            .build()
            .map_err(|e| DomainError::Network(e.to_string()))?;

        // throttle parallel probes so we don't spawn 254 sockets at once.
        let sem = Arc::new(tokio::sync::Semaphore::new(32));

        tokio::spawn(async move {
            let mut handles = Vec::new();
            for ip in ips {
                let client = client.clone();
                let tx = tx.clone();
                let sem =sem.clone();
                // 为每个 IP 并行发起请求(限流32并发)
                handles.push(tokio::spawn(async move {
                    let _permit = match sem.acquire().await{
                        Ok(p) => p,
                        Err(_) => return 
                    };
                    let url = format!("https://{}:{}/api/lansync/v1/info", ip, port);
                    if let Ok(resp) = client.get(&url).send().await {
                        if let Ok(info) = resp.json::<DeviceInfoResponse>().await {
                            let _ = tx.send(DiscoveredDevice {
                                device_id: DeviceId(info.device_id),
                                alias: info.alias,
                                address: ip.to_string(),
                            }).await;
                        }
                    }
                }));
            }

            // 等待所有扫描任务结束
            for h in handles {
                let _ = h.await;
            }
        });

        Ok(())
    }

    async fn stop(&self) -> Result<(), DomainError> {
        Ok(())
    }
}

// 内部使用的 API 响应结构
#[derive(serde::Deserialize)]
struct DeviceInfoResponse {
    device_id: String,
    alias: String,
}