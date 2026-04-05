use serde::ser::SerializeStruct;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Agent error: {0}")]
    Agent(String),

    #[error("Lock poisoned")]
    Lock,
}

impl AppError {
    /// Returns a snake_case code identifier for this error variant.
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Database(_) => "db_error",
            AppError::NotFound(_) => "not_found",
            AppError::Io(_) => "io_error",
            AppError::Json(_) => "json_error",
            AppError::Agent(_) => "agent_error",
            AppError::Lock => "lock_error",
        }
    }
}

/// Serializes as `{ "code": "...", "message": "..." }` for structured IPC errors.
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("code", self.code())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}
