//! Integration tests for RustVault
//! 
//! Tests the complete system including server, client, and persistence

use rustvault::Client;
use std::time::Duration;
use tempfile::NamedTempFile;
use tokio::time::sleep;

/// Helper function to start a test server
async fn start_test_server(port: u16, wal_path: String) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let config = rustvault::ServerConfig {
            bind_addr: format!("127.0.0.1:{}", port),
            wal_path,
            max_connections: 100,
        };
        
        let server = rustvault::RustVaultServer::new(config).await.unwrap();
        let _ = server.run().await;
    })
}

/// Helper function to wait for server to be ready
async fn wait_for_server(addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    for _ in 0..50 {
        if let Ok(client) = Client::connect(addr).await {
            let _ = client.close().await;
            return Ok(());
        }
        sleep(Duration::from_millis(100)).await;
    }
    Err("Server failed to start".into())
}

#[tokio::test]
async fn test_basic_operations() {
    let temp_file = NamedTempFile::new().unwrap();
    let wal_path = temp_file.path().to_string_lossy().to_string();
    let port = 18080;
    let addr = format!("127.0.0.1:{}", port);
    
    // Start server
    let _server_handle = start_test_server(port, wal_path).await;
    wait_for_server(&addr).await.unwrap();
    
    // Connect client
    let mut client = Client::connect(&addr).await.unwrap();
    
    // Test SET operation
    client.set("test_key", "test_value").await.unwrap();
    
    // Test GET operation
    let value = client.get("test_key").await.unwrap();
    assert_eq!(value, Some("test_value".to_string()));
    
    // Test GET non-existent key
    let value = client.get("nonexistent").await.unwrap();
    assert_eq!(value, None);
    
    // Test DELETE operation
    let deleted = client.delete("test_key").await.unwrap();
    assert!(deleted);
    
    // Test DELETE non-existent key
    let deleted = client.delete("test_key").await.unwrap();
    assert!(!deleted);
    
    // Verify key is gone
    let value = client.get("test_key").await.unwrap();
    assert_eq!(value, None);
    
    client.close().await.unwrap();
}

#[tokio::test]
async fn test_concurrent_clients() {
    let temp_file = NamedTempFile::new().unwrap();
    let wal_path = temp_file.path().to_string_lossy().to_string();
    let port = 18081;
    let addr = format!("127.0.0.1:{}", port);
    
    // Start server
    let _server_handle = start_test_server(port, wal_path).await;
    wait_for_server(&addr).await.unwrap();
    
    let num_clients = 10;
    let ops_per_client = 100;
    let mut handles = Vec::new();
    
    // Spawn multiple clients
    for client_id in 0..num_clients {
        let addr = addr.clone();
        let handle = tokio::spawn(async move {
            let mut client = Client::connect(&addr).await.unwrap();
            
            // Each client performs operations with unique keys
            for i in 0..ops_per_client {
                let key = format!("client_{}_key_{}", client_id, i);
                let value = format!("client_{}_value_{}", client_id, i);
                
                // SET
                client.set(&key, &value).await.unwrap();
                
                // GET
                let retrieved = client.get(&key).await.unwrap();
                assert_eq!(retrieved, Some(value));
                
                // DELETE every other key
                if i % 2 == 0 {
                    let deleted = client.delete(&key).await.unwrap();
                    assert!(deleted);
                }
            }
            
            client.close().await.unwrap();
        });
        handles.push(handle);
    }
    
    // Wait for all clients to complete
    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
async fn test_persistence_and_recovery() {
    let temp_file = NamedTempFile::new().unwrap();
    let wal_path = temp_file.path().to_string_lossy().to_string();
    let port = 18082;
    let addr = format!("127.0.0.1:{}", port);
    
    // Start first server instance
    let server_handle = start_test_server(port, wal_path.clone()).await;
    wait_for_server(&addr).await.unwrap();
    
    // Connect and add some data
    let mut client = Client::connect(&addr).await.unwrap();
    client.set("persistent_key1", "persistent_value1").await.unwrap();
    client.set("persistent_key2", "persistent_value2").await.unwrap();
    client.set("persistent_key3", "persistent_value3").await.unwrap();
    client.delete("persistent_key2").await.unwrap();
    client.close().await.unwrap();
    
    // Stop the server
    server_handle.abort();
    sleep(Duration::from_millis(500)).await;
    
    // Start a new server instance with the same WAL
    let port2 = 18083;
    let addr2 = format!("127.0.0.1:{}", port2);
    let _server_handle2 = start_test_server(port2, wal_path).await;
    wait_for_server(&addr2).await.unwrap();
    
    // Connect to new server and verify data persistence
    let mut client2 = Client::connect(&addr2).await.unwrap();
    
    let value1 = client2.get("persistent_key1").await.unwrap();
    assert_eq!(value1, Some("persistent_value1".to_string()));
    
    let value2 = client2.get("persistent_key2").await.unwrap();
    assert_eq!(value2, None); // Should be deleted
    
    let value3 = client2.get("persistent_key3").await.unwrap();
    assert_eq!(value3, Some("persistent_value3".to_string()));
    
    client2.close().await.unwrap();
}

#[tokio::test]
async fn test_large_values() {
    let temp_file = NamedTempFile::new().unwrap();
    let wal_path = temp_file.path().to_string_lossy().to_string();
    let port = 18084;
    let addr = format!("127.0.0.1:{}", port);
    
    // Start server
    let _server_handle = start_test_server(port, wal_path).await;
    wait_for_server(&addr).await.unwrap();
    
    let mut client = Client::connect(&addr).await.unwrap();
    
    // Test with large value (1MB)
    let large_value = "x".repeat(1024 * 1024);
    client.set("large_key", &large_value).await.unwrap();
    
    let retrieved = client.get("large_key").await.unwrap();
    assert_eq!(retrieved, Some(large_value));
    
    client.close().await.unwrap();
}

#[tokio::test]
async fn test_special_characters() {
    let temp_file = NamedTempFile::new().unwrap();
    let wal_path = temp_file.path().to_string_lossy().to_string();
    let port = 18085;
    let addr = format!("127.0.0.1:{}", port);
    
    // Start server
    let _server_handle = start_test_server(port, wal_path).await;
    wait_for_server(&addr).await.unwrap();
    
    let mut client = Client::connect(&addr).await.unwrap();
    
    // Test with special characters
    let special_key = "key_with_特殊字符_and_émojis_🚀";
    let special_value = "value_with_newlines\nand\ttabs\rand_quotes\"'";
    
    client.set(special_key, special_value).await.unwrap();
    
    let retrieved = client.get(special_key).await.unwrap();
    assert_eq!(retrieved, Some(special_value.to_string()));
    
    client.close().await.unwrap();
}

#[tokio::test]
async fn test_error_handling() {
    // Test connection to non-existent server
    let result = Client::connect("127.0.0.1:99999").await;
    assert!(result.is_err());
}