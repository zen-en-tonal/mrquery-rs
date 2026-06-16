use thiserror::Error;

#[derive(Debug, Error)]
pub enum MrqError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Image decode error: {0}")]
    Decode(String),
    #[error("Config error: {0}")]
    Config(String),
    #[error("Unsupported: {0}")]
    Unsupported(String),
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Index corrupt: {0}")]
    IndexCorrupt(String),
    #[error("Version mismatch: expected {expected}, got {got}")]
    VersionMismatch { expected: u64, got: u64 },
    #[error("Checksum mismatch for {path}")]
    ChecksumMismatch { path: String },
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Serialization error: {0}")]
    Serialize(String),
}

pub type Result<T> = std::result::Result<T, MrqError>;
