//! Protocol parser and command definitions for RustVault
//! 
//! Implements zero-copy parsing using nom for high performance

use crate::error::{RustVaultError, Result};
use nom::{
    branch::alt,
    bytes::complete::{tag, take_until, take_while1},
    character::complete::space1,
    combinator::map,
    sequence::{terminated, tuple},
    IResult,
};
use serde::{Deserialize, Serialize};
use std::str;

/// Commands supported by the RustVault protocol
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Command {
    Set { key: String, value: String },
    Get { key: String },
    Delete { key: String },
}

/// Response types from the server
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Response {
    Ok,
    Value(String),
    NotFound,
    Error(String),
}

impl Response {
    /// Serialize response to bytes for network transmission
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Response::Ok => b"OK\r\n".to_vec(),
            Response::Value(v) => format!("VALUE {}\r\n", v).into_bytes(),
            Response::NotFound => b"NOT_FOUND\r\n".to_vec(),
            Response::Error(e) => format!("ERROR {}\r\n", e).into_bytes(),
        }
    }
}

/// Parse a complete command from input bytes using zero-copy techniques
pub fn parse_command(input: &[u8]) -> Result<Command> {
    let (_, command) = command_parser(input)
        .map_err(|e| RustVaultError::Protocol(format!("Failed to parse command: {:?}", e)))?;
    Ok(command)
}

/// Main command parser using nom combinators
fn command_parser(input: &[u8]) -> IResult<&[u8], Command> {
    terminated(
        alt((set_command, get_command, delete_command)),
        alt((tag(b"\r\n"), tag(b"\n"))),
    )(input)
}

/// Parse SET command: SET <key> <value>
fn set_command(input: &[u8]) -> IResult<&[u8], Command> {
    map(
        tuple((
            tag(b"SET"),
            space1,
            take_while1(|c| c != b' ' && c != b'\r' && c != b'\n'),
            space1,
            take_until("\r\n"),
        )),
        |(_, _, key_bytes, _, value_bytes)| {
            let key = str::from_utf8(key_bytes).unwrap_or("").to_string();
            let value = str::from_utf8(value_bytes).unwrap_or("").to_string();
            Command::Set { key, value }
        },
    )(input)
}

/// Parse GET command: GET <key>
fn get_command(input: &[u8]) -> IResult<&[u8], Command> {
    map(
        tuple((
            tag(b"GET"),
            space1,
            take_while1(|c| c != b' ' && c != b'\r' && c != b'\n'),
        )),
        |(_, _, key_bytes)| {
            let key = str::from_utf8(key_bytes).unwrap_or("").to_string();
            Command::Get { key }
        },
    )(input)
}

/// Parse DELETE command: DELETE <key>
fn delete_command(input: &[u8]) -> IResult<&[u8], Command> {
    map(
        tuple((
            tag(b"DELETE"),
            space1,
            take_while1(|c| c != b' ' && c != b'\r' && c != b'\n'),
        )),
        |(_, _, key_bytes)| {
            let key = str::from_utf8(key_bytes).unwrap_or("").to_string();
            Command::Delete { key }
        },
    )(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_set_command() {
        let input = b"SET mykey myvalue\r\n";
        let result = parse_command(input).unwrap();
        assert_eq!(
            result,
            Command::Set {
                key: "mykey".to_string(),
                value: "myvalue".to_string()
            }
        );
    }

    #[test]
    fn test_parse_get_command() {
        let input = b"GET mykey\r\n";
        let result = parse_command(input).unwrap();
        assert_eq!(
            result,
            Command::Get {
                key: "mykey".to_string()
            }
        );
    }

    #[test]
    fn test_parse_delete_command() {
        let input = b"DELETE mykey\r\n";
        let result = parse_command(input).unwrap();
        assert_eq!(
            result,
            Command::Delete {
                key: "mykey".to_string()
            }
        );
    }

    #[test]
    fn test_response_serialization() {
        assert_eq!(Response::Ok.to_bytes(), b"OK\r\n");
        assert_eq!(
            Response::Value("test".to_string()).to_bytes(),
            b"VALUE test\r\n"
        );
        assert_eq!(Response::NotFound.to_bytes(), b"NOT_FOUND\r\n");
        assert_eq!(
            Response::Error("test error".to_string()).to_bytes(),
            b"ERROR test error\r\n"
        );
    }
}