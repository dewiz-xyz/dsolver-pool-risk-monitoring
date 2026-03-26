use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("database error: {0}")]
    Database(sqlx::Error),

    #[error("serialization error: {0}")]
    Serialization(serde_json::Error),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("spawned task panicked: {0}")]
    TaskPanic(String),

    #[error("incomplete simulation response: {0}")]
    IncompleteData(String),
}
