use thiserror::Error;

/// Central error type shared across the Local Linux AI Assistant workspace.
///
/// Individual crates may define their own error enums for internal detail,
/// but should provide a `From` conversion into `AppError` so errors can
/// propagate cleanly across module boundaries (DIG SS8, TDS SS8). Never panic
/// for recoverable errors -- return `AppResult` instead (DIG SS8).
#[derive(Debug, Error)]
pub enum AppError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("event bus error: {0}")]
    EventBus(String),

    #[error("{module} error: {message}")]
    Module {
        module: &'static str,
        message: String,
    },

    #[error("not yet implemented: {0}")]
    NotImplemented(&'static str),
}

/// Standard result alias used throughout the workspace (Apis SS16 Error API).
pub type AppResult<T> = std::result::Result<T, AppError>;
