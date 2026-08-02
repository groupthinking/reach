use thiserror::Error;

/// Sanitized error alphabet per Buzz spec Σ_err
#[derive(Debug, Error, Clone)]
pub enum RelayError {
    #[error("auth-required")]
    AuthRequired,
    #[error("restricted")]
    Restricted,
    #[error("invalid")]
    Invalid,
    #[error("duplicate")]
    Duplicate,
    #[error("pow")]
    Pow,
    #[error("rate-limited")]
    RateLimited,
    #[error("blocked")]
    Blocked,
    #[error("error")]
    InternalError,
    #[error("frame-too-large")]
    FrameTooLarge,
}

impl RelayError {
    pub fn prefix(&self) -> &'static str {
        match self {
            RelayError::AuthRequired => "auth-required",
            RelayError::Restricted => "restricted",
            RelayError::Invalid => "invalid",
            RelayError::Duplicate => "duplicate",
            RelayError::Pow => "pow",
            RelayError::RateLimited => "rate-limited",
            RelayError::Blocked => "blocked",
            RelayError::InternalError => "error",
            RelayError::FrameTooLarge => "frame-too-large",
        }
    }
}
