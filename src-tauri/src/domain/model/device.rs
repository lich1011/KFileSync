use serde::{Deserialize, Serialize};
use crate::domain::error::DomainError;

#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct DeviceId(pub String);

/// 设备证书包装类型，存储 PEM 格式的公钥证书字符串。
/// 内部字符串是私有的，只能通过 `from_pem()` 构造或 `as_pem()` 读取，防止绕过格式校验。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Certificate(String);

impl Certificate {
    /// 构造 Certificate，要求内容是合法的 PEM 格式（含 `-----BEGIN` 标头）。
    pub fn from_pem(pem: String) -> Result<Self, DomainError> {
        if !pem.trim_start().starts_with("-----BEGIN") {
            return Err(DomainError::BusinessRuleViolation(
                "Certificate must be a valid PEM-encoded string starting with '-----BEGIN'".into(),
            ));
        }
        Ok(Self(pem))
    }

    /// 读取 PEM 字符串内容。
    pub fn as_pem(&self) -> &str {
        &self.0
    }

    /// 测试与内部代码用：绕过格式校验，直接创建（仅用于 PoC / Mock 数据）。
    #[cfg(any(test, feature = "test-utils"))]
    pub fn mock(raw: &str) -> Self {
        Self(raw.to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredData {
    pub alias: String,
    pub address: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairedData {
    pub certificate: Certificate,
    pub paired_at: u64,
    pub alias: String,
    pub address: String,
    #[serde(default)]
    pub last_seen_at: Option<u64>
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RevokedData {
    pub revoked_at: u64,
    pub certificate: Certificate,
    pub alias: String,
    pub address: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeviceState {
    Discovered(DiscoveredData),
    Paired(PairedData),
    Revoked(RevokedData),
}

impl DeviceState {
    pub fn confirm_pairing(self, certificate: Certificate, timestamp: u64) -> Result<Self, DomainError> {
        match self {
            Self::Discovered(data) => Ok(Self::Paired(PairedData {
                certificate,
                paired_at: timestamp,
                alias: data.alias,
                address: data.address,
                last_seen_at:None
            })),
            _ => Err(DomainError::InvalidStateTransition("only Discovered can be paired")),
        }
    }

    pub fn revoke(self, timestamp: u64) -> Result<Self, DomainError> {
        match self {
            Self::Paired(data) => Ok(Self::Revoked(RevokedData {
                revoked_at: timestamp,
                certificate: data.certificate,
                alias: data.alias,
                address: data.address,
            })),
            _ => Err(DomainError::InvalidStateTransition("only Paired can be revoked")),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Device {
    pub id: DeviceId,
    pub state: DeviceState,
}
