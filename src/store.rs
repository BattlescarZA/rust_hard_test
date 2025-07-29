//! In-memory key-value store implementation with thread-safe access
//! 
//! Provides a thread-safe store using Arc and RwLock for concurrent access

use crate::error::Result;
use crate::protocol::Command;
use crate::wal::WriteAheadLog;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Trait defining the interface for key-value storage operations
pub trait Store: Send + Sync {
    /// Set a key-value pair
    async fn set(&self, key: String, value: String) -> Result<()>;
    
    /// Get a value by key
    async fn get(&self, key: &str) -> Result<Option<String>>;
    
    /// Delete a key-value pair
    async fn delete(&self, key: &str) -> Result<bool>;
    
    /// Check if a key exists
    async fn exists(&self, key: &str) -> Result<bool>;
    
    /// Get all key-value pairs (for WAL compaction)
    async fn get_all(&self) -> Result<Vec<(String, String)>>;
    
    /// Clear all data
    async fn clear(&self) -> Result<()>;
    
    /// Get the number of stored items
    async fn len(&self) -> Result<usize>;
}

/// Thread-safe in-memory key-value store
pub struct MemoryStore {
    data: Arc<RwLock<HashMap<String, String>>>,
    wal: Option<Arc<WriteAheadLog>>,
}

impl MemoryStore {
    /// Create a new memory store without WAL
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            wal: None,
        }
    }
    
    /// Create a new memory store with WAL for persistence
    pub fn with_wal(wal: Arc<WriteAheadLog>) -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            wal: Some(wal),
        }
    }
    
    /// Restore state from WAL
    pub async fn restore_from_wal(&self) -> Result<()> {
        if let Some(wal) = &self.wal {
            let store_clone = self.clone();
            wal.replay(move |command| {
                // Use blocking operations for WAL replay since it's synchronous
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    match command {
                        Command::Set { key, value } => {
                            // Direct insertion without WAL logging during replay
                            let mut data = store_clone.data.write().await;
                            data.insert(key, value);
                            Ok(())
                        }
                        Command::Delete { key } => {
                            // Direct deletion without WAL logging during replay
                            let mut data = store_clone.data.write().await;
                            data.remove(&key);
                            Ok(())
                        }
                        Command::Get { .. } => {
                            // GET commands don't modify state, skip during replay
                            Ok(())
                        }
                    }
                })
            })?;
        }
        Ok(())
    }
    
    /// Apply a command without WAL logging (used during replay)
    async fn apply_command_direct(&self, command: Command) -> Result<()> {
        match command {
            Command::Set { key, value } => {
                let mut data = self.data.write().await;
                data.insert(key, value);
                Ok(())
            }
            Command::Delete { key } => {
                let mut data = self.data.write().await;
                data.remove(&key);
                Ok(())
            }
            Command::Get { .. } => {
                // GET commands don't modify state
                Ok(())
            }
        }
    }
}

impl Clone for MemoryStore {
    fn clone(&self) -> Self {
        Self {
            data: Arc::clone(&self.data),
            wal: self.wal.clone(),
        }
    }
}

impl Store for MemoryStore {
    async fn set(&self, key: String, value: String) -> Result<()> {
        // Log to WAL first for durability
        if let Some(wal) = &self.wal {
            let command = Command::Set {
                key: key.clone(),
                value: value.clone(),
            };
            wal.log_command(command).await?;
        }
        
        // Then update in-memory store
        let mut data = self.data.write().await;
        data.insert(key, value);
        Ok(())
    }
    
    async fn get(&self, key: &str) -> Result<Option<String>> {
        let data = self.data.read().await;
        Ok(data.get(key).cloned())
    }
    
    async fn delete(&self, key: &str) -> Result<bool> {
        // Log to WAL first for durability
        if let Some(wal) = &self.wal {
            let command = Command::Delete {
                key: key.to_string(),
            };
            wal.log_command(command).await?;
        }
        
        // Then update in-memory store
        let mut data = self.data.write().await;
        Ok(data.remove(key).is_some())
    }
    
    async fn exists(&self, key: &str) -> Result<bool> {
        let data = self.data.read().await;
        Ok(data.contains_key(key))
    }
    
    async fn get_all(&self) -> Result<Vec<(String, String)>> {
        let data = self.data.read().await;
        Ok(data.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }
    
    async fn clear(&self) -> Result<()> {
        let mut data = self.data.write().await;
        data.clear();
        Ok(())
    }
    
    async fn len(&self) -> Result<usize> {
        let data = self.data.read().await;
        Ok(data.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_memory_store_basic_operations() {
        let store = MemoryStore::new();
        
        // Test set and get
        store.set("key1".to_string(), "value1".to_string()).await.unwrap();
        let result = store.get("key1").await.unwrap();
        assert_eq!(result, Some("value1".to_string()));
        
        // Test exists
        assert!(store.exists("key1").await.unwrap());
        assert!(!store.exists("nonexistent").await.unwrap());
        
        // Test delete
        assert!(store.delete("key1").await.unwrap());
        assert!(!store.delete("key1").await.unwrap()); // Already deleted
        
        let result = store.get("key1").await.unwrap();
        assert_eq!(result, None);
    }
    
    #[tokio::test]
    async fn test_memory_store_with_wal() {
        let temp_file = NamedTempFile::new().unwrap();
        let wal = Arc::new(WriteAheadLog::new(temp_file.path()).unwrap());
        let store = MemoryStore::with_wal(wal);
        
        // Test operations with WAL
        store.set("key1".to_string(), "value1".to_string()).await.unwrap();
        store.set("key2".to_string(), "value2".to_string()).await.unwrap();
        
        let result1 = store.get("key1").await.unwrap();
        let result2 = store.get("key2").await.unwrap();
        
        assert_eq!(result1, Some("value1".to_string()));
        assert_eq!(result2, Some("value2".to_string()));
        
        // Test get_all
        let all_data = store.get_all().await.unwrap();
        assert_eq!(all_data.len(), 2);
    }
    
    #[tokio::test]
    async fn test_concurrent_access() {
        let store = Arc::new(MemoryStore::new());
        let mut handles = vec![];
        
        // Spawn multiple tasks to test concurrent access
        for i in 0..10 {
            let store_clone = Arc::clone(&store);
            let handle = tokio::spawn(async move {
                let key = format!("key{}", i);
                let value = format!("value{}", i);
                store_clone.set(key.clone(), value.clone()).await.unwrap();
                let result = store_clone.get(&key).await.unwrap();
                assert_eq!(result, Some(value));
            });
            handles.push(handle);
        }
        
        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Verify all data is present
        assert_eq!(store.len().await.unwrap(), 10);
    }
}