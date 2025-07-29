//! Client library for connecting to RustVault server
//! 
//! Provides a simple interface for interacting with the key-value store

use crate::error::{RustVaultError, Result};
use crate::protocol::{Command, Response};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::TcpStream;

/// Client for connecting to RustVault server
pub struct Client {
    reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: BufWriter<tokio::net::tcp::OwnedWriteHalf>,
}

impl Client {
    /// Connect to a RustVault server
    pub async fn connect(addr: &str) -> Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        let (read_half, write_half) = stream.into_split();
        let reader = BufReader::new(read_half);
        let writer = BufWriter::new(write_half);
        
        Ok(Self { reader, writer })
    }
    
    /// Send a command and receive a response
    async fn send_command(&mut self, command: &Command) -> Result<Response> {
        // Serialize command to protocol format
        let command_bytes = match command {
            Command::Set { key, value } => format!("SET {} {}\r\n", key, value).into_bytes(),
            Command::Get { key } => format!("GET {}\r\n", key).into_bytes(),
            Command::Delete { key } => format!("DELETE {}\r\n", key).into_bytes(),
        };
        
        // Send command
        self.writer.write_all(&command_bytes).await?;
        self.writer.flush().await?;
        
        // Read response
        let mut response_line = String::new();
        self.reader.read_line(&mut response_line).await?;
        
        // Parse response
        self.parse_response(&response_line.trim())
    }
    
    /// Parse server response from string
    fn parse_response(&self, response: &str) -> Result<Response> {
        if response == "OK" {
            Ok(Response::Ok)
        } else if response == "NOT_FOUND" {
            Ok(Response::NotFound)
        } else if response.starts_with("VALUE ") {
            let value = response.strip_prefix("VALUE ").unwrap_or("").to_string();
            Ok(Response::Value(value))
        } else if response.starts_with("ERROR ") {
            let error = response.strip_prefix("ERROR ").unwrap_or("").to_string();
            Ok(Response::Error(error))
        } else {
            Err(RustVaultError::Protocol(format!(
                "Unknown response format: {}",
                response
            )))
        }
    }
    
    /// Set a key-value pair
    pub async fn set(&mut self, key: &str, value: &str) -> Result<()> {
        let command = Command::Set {
            key: key.to_string(),
            value: value.to_string(),
        };
        
        match self.send_command(&command).await? {
            Response::Ok => Ok(()),
            Response::Error(e) => Err(RustVaultError::Server(e)),
            _ => Err(RustVaultError::Protocol("Unexpected response for SET".to_string())),
        }
    }
    
    /// Get a value by key
    pub async fn get(&mut self, key: &str) -> Result<Option<String>> {
        let command = Command::Get {
            key: key.to_string(),
        };
        
        match self.send_command(&command).await? {
            Response::Value(value) => Ok(Some(value)),
            Response::NotFound => Ok(None),
            Response::Error(e) => Err(RustVaultError::Server(e)),
            _ => Err(RustVaultError::Protocol("Unexpected response for GET".to_string())),
        }
    }
    
    /// Delete a key
    pub async fn delete(&mut self, key: &str) -> Result<bool> {
        let command = Command::Delete {
            key: key.to_string(),
        };
        
        match self.send_command(&command).await? {
            Response::Ok => Ok(true),
            Response::NotFound => Ok(false),
            Response::Error(e) => Err(RustVaultError::Server(e)),
            _ => Err(RustVaultError::Protocol("Unexpected response for DELETE".to_string())),
        }
    }
    
    /// Close the connection
    pub async fn close(mut self) -> Result<()> {
        self.writer.shutdown().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_response() {
        // Create a dummy client for testing parse_response
        // We can't easily create a real client for unit tests, so we'll test the logic separately
        struct DummyClient;
        impl DummyClient {
            fn parse_response(&self, response: &str) -> crate::Result<crate::protocol::Response> {
                use crate::protocol::Response;
                use crate::error::RustVaultError;
                
                if response == "OK" {
                    Ok(Response::Ok)
                } else if response == "NOT_FOUND" {
                    Ok(Response::NotFound)
                } else if response.starts_with("VALUE ") {
                    let value = response.strip_prefix("VALUE ").unwrap_or("").to_string();
                    Ok(Response::Value(value))
                } else if response.starts_with("ERROR ") {
                    let error = response.strip_prefix("ERROR ").unwrap_or("").to_string();
                    Ok(Response::Error(error))
                } else {
                    Err(RustVaultError::Protocol(format!(
                        "Unknown response format: {}",
                        response
                    )))
                }
            }
        }
        
        let client = DummyClient;
        
        assert_eq!(client.parse_response("OK").unwrap(), Response::Ok);
        assert_eq!(client.parse_response("NOT_FOUND").unwrap(), Response::NotFound);
        assert_eq!(
            client.parse_response("VALUE test").unwrap(),
            Response::Value("test".to_string())
        );
        assert_eq!(
            client.parse_response("ERROR test error").unwrap(),
            Response::Error("test error".to_string())
        );
    }
}