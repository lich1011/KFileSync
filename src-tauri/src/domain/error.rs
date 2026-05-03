/// 全局领域错误类型，统一在此定义，不应散落在各聚合模型中。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DomainError {
    #[error("invalid state transition: {0}")]
    InvalidStateTransition(&'static str),
    
    #[error("pairing session has expired")]
    SessionExpired,
    
    #[error("PIN code does not match")]
    InvalidPinCode,
    
    #[error("business rule violation: {0}")]
    BusinessRuleViolation(String),
    
    #[error("device not found: {0}")]
    DeviceNotFound(String),
    
    #[error("device not trusted: {0}")]
    DeviceNotTrusted(String),
    
    #[error("share not found: {0}")]
    ShareNotFound(String),
    
    #[error("transfer not found: {0}")]
    TransferNotFound(String),
    
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("version conflict: {0}")]
    VersionConflict(String),
    
    #[error("integrity error: {0}")]
    IntegrityError(String),
    
    #[error("not found: {0}")]
    NotFound(String),
    
    #[error("persistence error: {0}")]
    Persistence(String),
    
    #[error("network error: {0}")]
    Network(String),
    
    #[error("security error: {0}")]
    Security(String),
    
    #[error("file system error: {0}")]
    FileSystem(String),
    
    #[error("nonce replay detected")]
    NonceReplay,
    
    #[error("timestamp out of window")]
    TimestampOutOfWindow,
}

pub type DomainResult<T> = Result<T, DomainError>;

// impl std::fmt::Display for DomainError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         match self {
//             Self::InvalidStateTransition(msg) => write!(f, "Invalid state transition: {}", msg),
//             Self::SessionExpired => write!(f, "Pairing session has expired"),
//             Self::InvalidPinCode => write!(f, "PIN code does not match"),
//             Self::BusinessRuleViolation(msg) => write!(f, "Business rule violation: {}", msg),
//         }
//     }
// }

// impl std::error::Error for DomainError {}
