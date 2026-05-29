use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Base64 error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("Encryption error")]
    Encryption,
    #[error("Password verification failed")]
    PasswordVerificationFailed,
    #[error("App is locked")]
    Locked,
    #[error("{0}")]
    Message(String),
}

pub type AppResult<T> = Result<T, AppError>;
