//! Performance benchmarks for RustVault server
//! 
//! Tests latency and throughput under various load conditions

use rustvault::Client;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

#[derive(Debug)]
struct BenchmarkResults {
    operation: String,
    total_operations: usize,
    duration: Duration,
    ops_per_second: f64,
    avg_latency_ms: f64,
    p95_latency_ms: f64,
    p99_latency_ms: f64,
}

impl BenchmarkResults {
    fn new(operation: String, total_operations: usize, duration: Duration, latencies: &mut [Duration]) -> Self {
        latencies.sort();
        
        let ops_per_second = total_operations as f64 / duration.as_secs_f64();
        let avg_latency_ms = latencies.iter().map(|d| d.as_secs_f64() * 1000.0).sum::<f64>() / latencies.len() as f64;
        
        let p95_index = (latencies.len() as f64 * 0.95) as usize;
        let p99_index = (latencies.len() as f64 * 0.99) as usize;
        
        let p95_latency_ms = latencies.get(p95_index).unwrap_or(&Duration::ZERO).as_secs_f64() * 1000.0;
        let p99_latency_ms = latencies.get(p99_index).unwrap_or(&Duration::ZERO).as_secs_f64() * 1000.0;
        
        Self {
            operation,
            total_operations,
            duration,
            ops_per_second,
            avg_latency_ms,
            p95_latency_ms,
            p99_latency_ms,
        }
    }
    
    fn print(&self) {
        println!("=== {} Benchmark Results ===", self.operation);
        println!("Total operations: {}", self.total_operations);
        println!("Duration: {:.2}s", self.duration.as_secs_f64());
        println!("Throughput: {:.2} ops/sec", self.ops_per_second);
        println!("Average latency: {:.2}ms", self.avg_latency_ms);
        println!("P95 latency: {:.2}ms", self.p95_latency_ms);
        println!("P99 latency: {:.2}ms", self.p99_latency_ms);
        println!();
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server_addr = "127.0.0.1:8080";
    
    println!("RustVault Performance Benchmarks");
    println!("=================================");
    println!("Server: {}", server_addr);
    println!();
    
    // Wait for server to be ready
    println!("Waiting for server to be ready...");
    loop {
        if let Ok(mut client) = Client::connect(server_addr).await {
            let _ = client.close().await;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    println!("Server is ready!");
    println!();
    
    // Run benchmarks
    run_single_client_benchmarks(server_addr).await?;
    run_concurrent_benchmarks(server_addr).await?;
    
    Ok(())
}

async fn run_single_client_benchmarks(server_addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Running single client benchmarks...");
    
    // SET benchmark
    let set_results = benchmark_set_operations(server_addr, 10000).await?;
    set_results.print();
    
    // GET benchmark
    let get_results = benchmark_get_operations(server_addr, 10000).await?;
    get_results.print();
    
    // Mixed workload benchmark
    let mixed_results = benchmark_mixed_workload(server_addr, 10000).await?;
    mixed_results.print();
    
    Ok(())
}

async fn run_concurrent_benchmarks(server_addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Running concurrent client benchmarks...");
    
    // Test with different numbers of concurrent clients
    for num_clients in [10, 50, 100] {
        let results = benchmark_concurrent_operations(server_addr, num_clients, 1000).await?;
        results.print();
    }
    
    Ok(())
}

async fn benchmark_set_operations(server_addr: &str, num_operations: usize) -> Result<BenchmarkResults, Box<dyn std::error::Error>> {
    let mut client = Client::connect(server_addr).await?;
    let mut latencies = Vec::with_capacity(num_operations);
    
    let start = Instant::now();
    
    for i in 0..num_operations {
        let key = format!("bench_key_{}", i);
        let value = format!("bench_value_{}", i);
        
        let op_start = Instant::now();
        client.set(&key, &value).await?;
        let op_duration = op_start.elapsed();
        
        latencies.push(op_duration);
    }
    
    let total_duration = start.elapsed();
    client.close().await?;
    
    Ok(BenchmarkResults::new(
        "SET".to_string(),
        num_operations,
        total_duration,
        &mut latencies,
    ))
}

async fn benchmark_get_operations(server_addr: &str, num_operations: usize) -> Result<BenchmarkResults, Box<dyn std::error::Error>> {
    // First, populate the store with data
    let mut setup_client = Client::connect(server_addr).await?;
    for i in 0..num_operations {
        let key = format!("get_bench_key_{}", i);
        let value = format!("get_bench_value_{}", i);
        setup_client.set(&key, &value).await?;
    }
    setup_client.close().await?;
    
    // Now benchmark GET operations
    let mut client = Client::connect(server_addr).await?;
    let mut latencies = Vec::with_capacity(num_operations);
    
    let start = Instant::now();
    
    for i in 0..num_operations {
        let key = format!("get_bench_key_{}", i);
        
        let op_start = Instant::now();
        let _value = client.get(&key).await?;
        let op_duration = op_start.elapsed();
        
        latencies.push(op_duration);
    }
    
    let total_duration = start.elapsed();
    client.close().await?;
    
    Ok(BenchmarkResults::new(
        "GET".to_string(),
        num_operations,
        total_duration,
        &mut latencies,
    ))
}

async fn benchmark_mixed_workload(server_addr: &str, num_operations: usize) -> Result<BenchmarkResults, Box<dyn std::error::Error>> {
    let mut client = Client::connect(server_addr).await?;
    let mut latencies = Vec::with_capacity(num_operations);
    
    let start = Instant::now();
    
    for i in 0..num_operations {
        let key = format!("mixed_key_{}", i % 1000); // Reuse keys for realistic workload
        
        let op_start = Instant::now();
        
        match i % 10 {
            0..=6 => {
                // 70% GET operations
                let _value = client.get(&key).await?;
            }
            7..=8 => {
                // 20% SET operations
                let value = format!("mixed_value_{}", i);
                client.set(&key, &value).await?;
            }
            9 => {
                // 10% DELETE operations
                let _deleted = client.delete(&key).await?;
            }
            _ => unreachable!(),
        }
        
        let op_duration = op_start.elapsed();
        latencies.push(op_duration);
    }
    
    let total_duration = start.elapsed();
    client.close().await?;
    
    Ok(BenchmarkResults::new(
        "Mixed Workload".to_string(),
        num_operations,
        total_duration,
        &mut latencies,
    ))
}

async fn benchmark_concurrent_operations(
    server_addr: &str,
    num_clients: usize,
    ops_per_client: usize,
) -> Result<BenchmarkResults, Box<dyn std::error::Error>> {
    let semaphore = Arc::new(Semaphore::new(num_clients));
    let mut handles = Vec::new();
    let mut all_latencies = Vec::new();
    
    let start = Instant::now();
    
    for client_id in 0..num_clients {
        let semaphore = Arc::clone(&semaphore);
        let server_addr = server_addr.to_string();
        
        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            let mut client = Client::connect(&server_addr).await.map_err(|e| format!("Connect error: {}", e))?;
            let mut latencies = Vec::with_capacity(ops_per_client);
            
            for i in 0..ops_per_client {
                let key = format!("concurrent_key_{}_{}", client_id, i);
                let value = format!("concurrent_value_{}_{}", client_id, i);
                
                let op_start = Instant::now();
                client.set(&key, &value).await.map_err(|e| format!("Set error: {}", e))?;
                let op_duration = op_start.elapsed();
                
                latencies.push(op_duration);
            }
            
            client.close().await.map_err(|e| format!("Close error: {}", e))?;
            Ok::<Vec<Duration>, String>(latencies)
        });
        
        handles.push(handle);
    }
    
    // Collect results from all clients
    for handle in handles {
        let latencies = handle.await.map_err(|e| format!("Join error: {}", e))?.map_err(|e| format!("Task error: {}", e))?;
        all_latencies.extend(latencies);
    }
    
    let total_duration = start.elapsed();
    let total_operations = num_clients * ops_per_client;
    
    Ok(BenchmarkResults::new(
        format!("Concurrent ({} clients)", num_clients),
        total_operations,
        total_duration,
        &mut all_latencies,
    ))
}