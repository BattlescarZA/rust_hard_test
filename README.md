# RustVault

A high-performance, concurrent key-value store with TCP interface, write-ahead logging, and zero-copy parsing built in Rust.

## Features

- **High Performance**: Zero-copy protocol parsing using `nom` for minimal latency
- **Concurrent Access**: Supports 100+ concurrent clients using `tokio` async I/O
- **Persistence**: Write-ahead logging (WAL) for durability and crash recovery
- **Thread-Safe**: In-memory store using `Arc<RwLock>` for safe concurrent access
- **Custom Protocol**: Simple text-based protocol for SET, GET, and DELETE operations
- **Graceful Shutdown**: Handles SIGINT (Ctrl+C) for clean server termination
- **Comprehensive Testing**: Unit tests, integration tests, and performance benchmarks

## Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   TCP Client    │◄──►│   RustVault      │◄──►│  Write-Ahead    │
│                 │    │   Server         │    │  Log (WAL)      │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                              │
                              ▼
                       ┌──────────────────┐
                       │   In-Memory      │
                       │   HashMap        │
                       │   (Thread-Safe)  │
                       └──────────────────┘
```

## Quick Start

### Prerequisites

- Rust 1.70+ with Cargo
- No external dependencies required

### Building

```bash
# Clone the repository
git clone <repository-url>
cd rustvault

# Build the project
cargo build --release

# Run tests
cargo test

# Run benchmarks
cargo bench
```

### Running the Server

```bash
# Start the server (default: 127.0.0.1:8080)
cargo run --bin server

# Or run the release build
./target/release/server
```

The server will:
- Listen on `127.0.0.1:8080` by default
- Create/use `vault.log` for persistence
- Restore state from WAL on startup
- Handle graceful shutdown on Ctrl+C

### Using the Client

```bash
# Start the interactive client
cargo run --bin client

# Or connect to a specific server
cargo run --bin client 127.0.0.1:8080
```

#### Client Commands

```
> set mykey myvalue    # Set a key-value pair
OK

> get mykey           # Get value by key
myvalue

> delete mykey        # Delete a key
OK

> get mykey           # Key not found
(nil)

> help               # Show available commands
> quit               # Exit client
```

## Protocol

RustVault uses a simple text-based protocol over TCP:

### Commands

- `SET <key> <value>\r\n` - Store a key-value pair
- `GET <key>\r\n` - Retrieve value by key  
- `DELETE <key>\r\n` - Remove a key-value pair

### Responses

- `OK\r\n` - Command succeeded
- `VALUE <value>\r\n` - GET command result
- `NOT_FOUND\r\n` - Key doesn't exist
- `ERROR <message>\r\n` - Command failed

### Example Session

```
Client: SET user:1 john\r\n
Server: OK\r\n

Client: GET user:1\r\n  
Server: VALUE john\r\n

Client: DELETE user:1\r\n
Server: OK\r\n

Client: GET user:1\r\n
Server: NOT_FOUND\r\n
```

## Performance

RustVault is designed for high performance:

### Benchmarks

Run the included benchmarks:

```bash
cargo run --bin benchmark
```

Expected performance on modern hardware:
- **GET latency**: <10ms for single client
- **Throughput**: 10,000+ ops/sec for mixed workload
- **Concurrent clients**: 100+ without degradation
- **Memory usage**: Minimal overhead with zero-copy parsing

### Performance Features

- **Zero-copy parsing** with `nom` for minimal allocations
- **Async I/O** with `tokio` for high concurrency
- **Efficient data structures** with `HashMap` and `RwLock`
- **Write-ahead logging** with buffered I/O
- **Connection pooling** support for clients

## Persistence

### Write-Ahead Log (WAL)

All write operations (SET, DELETE) are logged to `vault.log` before being applied to the in-memory store. This ensures:

- **Durability**: Data survives server crashes
- **Recovery**: Automatic state restoration on restart
- **Consistency**: Operations are atomic

### WAL Format

Each entry is JSON-serialized with timestamp:

```json
{"timestamp":1640995200000,"command":{"Set":{"key":"user:1","value":"john"}}}
{"timestamp":1640995201000,"command":{"Delete":{"key":"user:1"}}}
```

### Recovery Process

On startup, the server:
1. Reads the WAL file line by line
2. Replays all operations in order
3. Rebuilds the in-memory state
4. Continues normal operation

## Development

### Project Structure

```
src/
├── lib.rs          # Library exports
├── main.rs         # Server binary
├── client.rs       # Client library
├── error.rs        # Error types
├── protocol.rs     # Protocol parser
├── server.rs       # TCP server
├── store.rs        # Key-value store
├── wal.rs          # Write-ahead log
└── bin/
    ├── client.rs   # Client binary
    └── benchmark.rs # Benchmark suite
tests/
└── integration_tests.rs # Integration tests
```

### Key Components

#### Store Trait

```rust
pub trait Store: Send + Sync {
    async fn set(&self, key: String, value: String) -> Result<()>;
    async fn get(&self, key: &str) -> Result<Option<String>>;
    async fn delete(&self, key: &str) -> Result<bool>;
    // ... other methods
}
```

#### Error Handling

Custom error types with `thiserror`:

```rust
#[derive(Error, Debug)]
pub enum RustVaultError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Protocol parse error: {0}")]
    Protocol(String),
    // ... other variants
}
```

### Running Tests

```bash
# Unit tests
cargo test

# Integration tests  
cargo test --test integration_tests

# Test with output
cargo test -- --nocapture

# Test specific module
cargo test store::tests
```

### Adding Features

1. **New Commands**: Add to `Command` enum in `protocol.rs`
2. **New Store Types**: Implement the `Store` trait
3. **Protocol Changes**: Update parser in `protocol.rs`
4. **Error Types**: Add variants to `RustVaultError`

## Configuration

### Server Configuration

```rust
pub struct ServerConfig {
    pub bind_addr: String,      // Default: "127.0.0.1:8080"
    pub wal_path: String,       // Default: "vault.log"  
    pub max_connections: usize, // Default: 1000
}
```

### Environment Variables

Currently uses defaults, but can be extended to support:
- `RUSTVAULT_BIND_ADDR`
- `RUSTVAULT_WAL_PATH`
- `RUSTVAULT_MAX_CONNECTIONS`

## Safety and Correctness

### Memory Safety

- No `unsafe` code in the core implementation
- All data structures are thread-safe
- Automatic memory management with Rust's ownership system

### Concurrency Safety

- `Arc<RwLock<HashMap>>` for thread-safe store access
- `tokio::sync` primitives for async coordination
- No data races or deadlocks

### Error Handling

- Comprehensive error types with `thiserror`
- Graceful error propagation with `Result<T>`
- Client and server error recovery

## Limitations

- **In-memory only**: Data size limited by available RAM
- **Single node**: No clustering or replication
- **Simple protocol**: No authentication or encryption
- **WAL compaction**: Manual compaction required for large logs

## Future Enhancements

- [ ] Clustering and replication
- [ ] Authentication and authorization  
- [ ] TLS/SSL encryption
- [ ] Automatic WAL compaction
- [ ] Metrics and monitoring
- [ ] Configuration file support
- [ ] Multiple data types (lists, sets, etc.)
- [ ] Pub/sub messaging
- [ ] HTTP REST API
- [ ] Admin interface

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Built with [Tokio](https://tokio.rs/) for async I/O
- Protocol parsing with [nom](https://github.com/Geal/nom)
- Error handling with [thiserror](https://github.com/dtolnay/thiserror)
- Serialization with [serde](https://serde.rs/)