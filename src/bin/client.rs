//! Standalone client binary for testing RustVault server
//! 
//! Provides a command-line interface for interacting with the server

use rustvault::Client;
use std::env;
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let server_addr = args.get(1).unwrap_or(&"127.0.0.1:8080".to_string()).clone();
    
    println!("Connecting to RustVault server at {}...", server_addr);
    let mut client = Client::connect(&server_addr).await?;
    println!("Connected! Type 'help' for available commands or 'quit' to exit.");
    
    loop {
        print!("> ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        
        if input.is_empty() {
            continue;
        }
        
        match input {
            "quit" | "exit" => {
                println!("Goodbye!");
                break;
            }
            "help" => {
                print_help();
            }
            _ => {
                if let Err(e) = handle_command(&mut client, input).await {
                    println!("Error: {}", e);
                }
            }
        }
    }
    
    client.close().await?;
    Ok(())
}

async fn handle_command(client: &mut Client, input: &str) -> Result<(), Box<dyn std::error::Error>> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    
    match parts.get(0) {
        Some(&"set") => {
            if parts.len() != 3 {
                println!("Usage: set <key> <value>");
                return Ok(());
            }
            
            let key = parts[1];
            let value = parts[2];
            
            client.set(key, value).await?;
            println!("OK");
        }
        Some(&"get") => {
            if parts.len() != 2 {
                println!("Usage: get <key>");
                return Ok(());
            }
            
            let key = parts[1];
            
            match client.get(key).await? {
                Some(value) => println!("{}", value),
                None => println!("(nil)"),
            }
        }
        Some(&"delete") | Some(&"del") => {
            if parts.len() != 2 {
                println!("Usage: delete <key>");
                return Ok(());
            }
            
            let key = parts[1];
            
            if client.delete(key).await? {
                println!("OK");
            } else {
                println!("Key not found");
            }
        }
        _ => {
            println!("Unknown command: {}. Type 'help' for available commands.", parts[0]);
        }
    }
    
    Ok(())
}

fn print_help() {
    println!("Available commands:");
    println!("  set <key> <value>  - Set a key-value pair");
    println!("  get <key>          - Get value by key");
    println!("  delete <key>       - Delete a key");
    println!("  help               - Show this help message");
    println!("  quit               - Exit the client");
}