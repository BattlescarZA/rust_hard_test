//! Error types for RustVault

use thiserror::Error;
use std::io;

/// Result type alias for RustVault operations
pub type Result<T> = std::result::Result<T, RustVaultError>;

/// Custom error types for RustVault
#[derive(Error, Debug)]
pub enum RustVaultError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Protocol parse error: {0}")]
    Protocol(String),
    
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    
    #[error("Invalid command: {0}")]
    InvalidCommand(String),
    
    #[error("Server error: {0}")]
    Server(String),
    
    #[error("Client error: {0}")]
    Client(String),
    
    #[error("WAL error: {0}")]
    Wal(String),
}

impl From<nom::Err<nom::error::Error<&[u8]>>> for RustVaultError {
    fn from(err: nom::Err<nom::error::Error<&[u8]>>) -> Self {
        RustVaultError::Protocol(format!("Parse error: {:?}", err))
    }
}