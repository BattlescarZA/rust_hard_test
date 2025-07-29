//! RustVault TCP Server
//! 
//! High-performance key-value store with TCP interface, WAL persistence,
//! and concurrent client support using tokio async I/O.

use crate::{
    error::{Result, RustVaultError},
    protocol::{parse_command, Command, Response},
    store::{MemoryStore, Store},
    wal::WriteAheadLog,
};
use std::sync::Arc;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::broadcast,
};

/// RustVault server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub bind_addr: String,
    pub wal_path: String,
    pub max_connections: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:8080".to_string(),
            wal_path: "vault.log".to_string(),
            max_connections: 1000,
        }
    }
}

/// RustVault TCP server
pub struct RustVaultServer {
    config: ServerConfig,
    store: Arc<MemoryStore>,
    shutdown_tx: broadcast::Sender<()>,
}

impl RustVaultServer {
    /// Create a new server instance
    pub async fn new(config: ServerConfig) -> Result<Self> {
        // Initialize WAL
        let wal = Arc::new(WriteAheadLog::new(&config.wal_path)?);
        
        // Initialize store with WAL
        let store = MemoryStore::with_wal(wal);
        
        // Restore state from WAL
        println!("Restoring state from WAL: {}", config.wal_path);
        store.restore_from_wal().await?;
        let restored_count = store.len().await?;
        println!("Restored {} key-value pairs from WAL", restored_count);
        
        let (shutdown_tx, _) = broadcast::channel(1);
        
        Ok(Self {
            config,
            store: Arc::new(store),
            shutdown_tx,
        })
    }
    
    /// Start the server
    pub async fn run(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.config.bind_addr).await?;
        println!("RustVault server listening on {}", self.config.bind_addr);
        
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        
        loop {
            tokio::select! {
                // Accept new connections
                result = listener.accept() => {
                    match result {
                        Ok((stream, addr)) => {
                            println!("New client connected: {}", addr);
                            let store = Arc::clone(&self.store);
                            let shutdown_rx = self.shutdown_tx.subscribe();
                            
                            // Spawn a task to handle the client
                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_client(stream, store, shutdown_rx).await {
                                    eprintln!("Error handling client {}: {}", addr, e);
                                }
                                println!("Client disconnected: {}", addr);
                            });
                        }
                        Err(e) => {
                            eprintln!("Failed to accept connection: {}", e);
                        }
                    }
                }
                
                // Handle shutdown signal
                _ = shutdown_rx.recv() => {
                    println!("Shutdown signal received, stopping server...");
                    break;
                }
            }
        }
        
        println!("Server stopped");
        Ok(())
    }
    
    /// Handle a single client connection
    async fn handle_client(
        mut stream: TcpStream,
        store: Arc<MemoryStore>,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<()> {
        let (reader, mut writer) = stream.split();
        let mut buf_reader = BufReader::new(reader);
        let mut line = String::new();
        
        loop {
            line.clear();
            
            tokio::select! {
                // Read command from client
                result = buf_reader.read_line(&mut line) => {
                    match result {
                        Ok(0) => {
                            // Client disconnected
                            break;
                        }
                        Ok(_) => {
                            let response = Self::process_command(&line, &store).await;
                            let response_bytes = response.to_bytes();
                            
                            if let Err(e) = writer.write_all(&response_bytes).await {
                                eprintln!("Failed to write response: {}", e);
                                break;
                            }
                            
                            if let Err(e) = writer.flush().await {
                                eprintln!("Failed to flush response: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to read from client: {}", e);
                            break;
                        }
                    }
                }
                
                // Handle shutdown signal
                _ = shutdown_rx.recv() => {
                    println!("Shutdown signal received, closing client connection");
                    break;
                }
            }
        }
        
        Ok(())
    }
    
    /// Process a command from a client
    async fn process_command(line: &str, store: &Arc<MemoryStore>) -> Response {
        let command_bytes = line.trim().as_bytes();
        if command_bytes.is_empty() {
            return Response::Error("Empty command".to_string());
        }
        
        // Add \r\n if not present for parser compatibility
        let mut full_command = command_bytes.to_vec();
        if !full_command.ends_with(b"\r\n") && !full_command.ends_with(b"\n") {
            full_command.extend_from_slice(b"\r\n");
        }
        
        match parse_command(&full_command) {
            Ok(command) => Self::execute_command(command, store).await,
            Err(e) => Response::Error(format!("Parse error: {}", e)),
        }
    }
    
    /// Execute a parsed command
    async fn execute_command(command: Command, store: &Arc<MemoryStore>) -> Response {
        match command {
            Command::Set { key, value } => {
                match store.set(key, value).await {
                    Ok(()) => Response::Ok,
                    Err(e) => Response::Error(format!("SET failed: {}", e)),
                }
            }
            Command::Get { key } => {
                match store.get(&key).await {
                    Ok(Some(value)) => Response::Value(value),
                    Ok(None) => Response::NotFound,
                    Err(e) => Response::Error(format!("GET failed: {}", e)),
                }
            }
            Command::Delete { key } => {
                match store.delete(&key).await {
                    Ok(true) => Response::Ok,
                    Ok(false) => Response::NotFound,
                    Err(e) => Response::Error(format!("DELETE failed: {}", e)),
                }
            }
        }
    }
    
    /// Trigger graceful shutdown
    pub fn shutdown(&self) -> Result<()> {
        self.shutdown_tx.send(()).map_err(|_| {
            RustVaultError::Server("Failed to send shutdown signal".to_string())
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_server_creation() {
        let temp_file = NamedTempFile::new().unwrap();
        let config = ServerConfig {
            bind_addr: "127.0.0.1:0".to_string(), // Use port 0 for testing
            wal_path: temp_file.path().to_string_lossy().to_string(),
            max_connections: 10,
        };
        
        let server = RustVaultServer::new(config).await.unwrap();
        // The shutdown might fail if there are no receivers, which is fine for this test
        let _ = server.shutdown();
    }
    
    #[tokio::test]
    async fn test_command_processing() {
        let temp_file = NamedTempFile::new().unwrap();
        let wal = Arc::new(WriteAheadLog::new(temp_file.path()).unwrap());
        let store = Arc::new(MemoryStore::with_wal(wal));
        
        // Test SET command
        let response = RustVaultServer::process_command("SET key1 value1", &store).await;
        assert_eq!(response, Response::Ok);
        
        // Test GET command
        let response = RustVaultServer::process_command("GET key1", &store).await;
        assert_eq!(response, Response::Value("value1".to_string()));
        
        // Test DELETE command
        let response = RustVaultServer::process_command("DELETE key1", &store).await;
        assert_eq!(response, Response::Ok);
        
        // Test GET after DELETE
        let response = RustVaultServer::process_command("GET key1", &store).await;
        assert_eq!(response, Response::NotFound);
    }
}