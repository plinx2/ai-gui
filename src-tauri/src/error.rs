use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API error: {0}")]
    Api(String),
    #[error("Session not found: {0}")]
    SessionNotFound(String),
    #[error("Config missing API key for: {0}")]
    MissingApiKey(String),
    #[error("Path error: {0}")]
    Path(String),
}

impl From<AppError> for String {
    fn from(e: AppError) -> Self {
        e.to_string()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
