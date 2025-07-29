//! RustVault - A high-performance key-value store with TCP interface
//! 
//! This library provides a thread-safe, persistent key-value store with:
//! - TCP server interface with custom protocol
//! - Write-ahead logging for durability
//! - Zero-copy parsing for performance
//! - Concurrent client support

pub mod client;
pub mod error;
pub mod protocol;
pub mod server;
pub mod store;
pub mod wal;

pub use error::{RustVaultError, Result};
pub use store::{Store, MemoryStore};
pub use protocol::{Command, Response};
pub use client::Client;
pub use server::{RustVaultServer, ServerConfig};