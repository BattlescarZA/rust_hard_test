//! RustVault Server Binary
//!
//! Main entry point for the RustVault TCP server

use rustvault::{Result, RustVaultServer, ServerConfig};
use std::sync::Arc;
use tokio::signal;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments or use defaults
    let config = ServerConfig::default();
    
    // Create and start server
    let server = RustVaultServer::new(config).await?;
    let server = Arc::new(server);
    
    // Setup graceful shutdown on SIGINT (Ctrl+C)
    let server_clone = Arc::clone(&server);
    tokio::spawn(async move {
        if let Err(e) = signal::ctrl_c().await {
            eprintln!("Failed to listen for Ctrl+C: {}", e);
            return;
        }
        
        println!("\nReceived Ctrl+C, initiating graceful shutdown...");
        if let Err(e) = server_clone.shutdown() {
            eprintln!("Failed to initiate shutdown: {}", e);
        }
    });
    
    // Run the server
    server.run().await?;
    
    Ok(())
}
