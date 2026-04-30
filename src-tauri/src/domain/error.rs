/// 全局领域错误类型，统一在此定义，不应散落在各聚合模型中。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// 尝试触发当前状态下不允许的状态转移
    InvalidStateTransition(&'static str),
    /// 配对会话已过期
    SessionExpired,
    /// 配对验证码不匹配
    InvalidPinCode,
    /// 通用业务规则违例
    BusinessRuleViolation(String),
}

impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidStateTransition(msg) => write!(f, "Invalid state transition: {}", msg),
            Self::SessionExpired => write!(f, "Pairing session has expired"),
            Self::InvalidPinCode => write!(f, "PIN code does not match"),
            Self::BusinessRuleViolation(msg) => write!(f, "Business rule violation: {}", msg),
        }
    }
}

impl std::error::Error for DomainError {}
