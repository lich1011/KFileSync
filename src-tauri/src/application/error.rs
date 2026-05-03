use crate::domain::error::DomainError;

#[derive(Debug, Clone, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Domain(#[from] DomainError),
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::Internal(s)
    }
}

pub type AppResult<T> = Result<T, AppError>;