//! Write-Ahead Log implementation for RustVault
//! 
//! Provides durable persistence by logging all operations before applying them

use crate::error::{RustVaultError, Result};
use crate::protocol::Command;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use tokio::sync::Mutex;

/// WAL entry representing a logged operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalEntry {
    pub timestamp: u64,
    pub command: Command,
}

impl WalEntry {
    pub fn new(command: Command) -> Self {
        Self {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            command,
        }
    }
}

/// Write-Ahead Log for durable persistence
pub struct WriteAheadLog {
    writer: Mutex<BufWriter<File>>,
    path: String,
}

impl WriteAheadLog {
    /// Create a new WAL instance
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        
        let writer = BufWriter::new(file);
        
        Ok(Self {
            writer: Mutex::new(writer),
            path: path_str,
        })
    }

    /// Write an entry to the WAL
    pub async fn write_entry(&self, entry: &WalEntry) -> Result<()> {
        let mut writer = self.writer.lock().await;
        let json = serde_json::to_string(entry)?;
        writeln!(writer, "{}", json)?;
        writer.flush()?;
        Ok(())
    }

    /// Log a command to the WAL
    pub async fn log_command(&self, command: Command) -> Result<()> {
        let entry = WalEntry::new(command);
        self.write_entry(&entry).await
    }

    /// Replay all entries from the WAL
    pub fn replay<F>(&self, mut apply_fn: F) -> Result<()>
    where
        F: FnMut(Command) -> Result<()>,
    {
        if !Path::new(&self.path).exists() {
            return Ok(());
        }

        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            let entry: WalEntry = serde_json::from_str(&line)
                .map_err(|e| RustVaultError::Wal(format!("Failed to parse WAL entry: {}", e)))?;
            
            apply_fn(entry.command)?;
        }

        Ok(())
    }

    /// Compact the WAL by rewriting it with current state
    pub async fn compact<F>(&self, get_all_entries: F) -> Result<()>
    where
        F: Fn() -> Vec<(String, String)>,
    {
        // Create a temporary file for the compacted WAL
        let temp_path = format!("{}.tmp", self.path);
        let temp_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&temp_path)?;
        
        let mut temp_writer = BufWriter::new(temp_file);
        
        // Write all current key-value pairs as SET commands
        for (key, value) in get_all_entries() {
            let command = Command::Set { key, value };
            let entry = WalEntry::new(command);
            let json = serde_json::to_string(&entry)?;
            writeln!(temp_writer, "{}", json)?;
        }
        
        temp_writer.flush()?;
        drop(temp_writer);
        
        // Replace the original WAL with the compacted version
        std::fs::rename(&temp_path, &self.path)?;
        
        // Reopen the writer
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        
        let new_writer = BufWriter::new(file);
        *self.writer.lock().await = new_writer;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_wal_write_and_replay() {
        let temp_file = NamedTempFile::new().unwrap();
        let wal = WriteAheadLog::new(temp_file.path()).unwrap();
        
        // Write some commands
        let cmd1 = Command::Set {
            key: "key1".to_string(),
            value: "value1".to_string(),
        };
        let cmd2 = Command::Get {
            key: "key1".to_string(),
        };
        
        wal.log_command(cmd1.clone()).await.unwrap();
        wal.log_command(cmd2.clone()).await.unwrap();
        
        // Replay commands
        let mut replayed_commands = Vec::new();
        wal.replay(|cmd| {
            replayed_commands.push(cmd);
            Ok(())
        }).unwrap();
        
        assert_eq!(replayed_commands.len(), 2);
        assert_eq!(replayed_commands[0], cmd1);
        assert_eq!(replayed_commands[1], cmd2);
    }
}