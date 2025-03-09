//! Very basic example to showcase how to use iroh's APIs.
//!
//! This example implements a simple protocol that echos any data sent to it in the first stream.
//!
//! ## Usage
//!
//!     cargo run --bin client -- --public-key <public-key>

use anyhow::Result;
use iroh::{Endpoint, NodeAddr, PublicKey, endpoint::Connection};
use tokio::time::{Instant, sleep};
use hex;
use std::time::Duration;
use clap::Parser;

/// Each protocol is identified by its ALPN string.
///
/// The ALPN, or application-layer protocol negotiation, is exchanged in the connection handshake,
/// and the connection is aborted unless both nodes pass the same bytestring.
const ALPN: &[u8] = b"iroh-example/print/0";

/// CLI arguments
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Public key in hex format
    #[arg(short, long)]
    public_key: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Decode the hex string into raw bytes
    let pk_bytes = hex::decode(&args.public_key)?;
    let pk_array: [u8; 32] = pk_bytes[..].try_into()
        .map_err(|_| anyhow::anyhow!("Invalid public key length - expected 32 bytes"))?;
    
    // Create public key and node address
    let public_key = PublicKey::from_bytes(&pk_array)?;
    let node_addr = NodeAddr::new(public_key);
    println!("Node Address: {:?}", node_addr);

    connect_side(node_addr).await?;

    Ok(())
}

async fn connect_side(addr: NodeAddr) -> Result<()> {
    let endpoint = Endpoint::builder().discovery_n0().bind().await?;

    // Perform multiple measurements with different data sizes
    let mb = 1024 * 1024;
    let sizes = vec![1 * mb, 2 * mb, 5 * mb, 10 * mb]; // 1KB, 1MB, 10MB
    
    // Actual benchmarks
    println!("\nStarting benchmarks:");
    for size in sizes {
        println!("\nTesting with {} MB:", size / (1024 * 1024));
        
        let iterations = 5;
        let mut bandwidths = Vec::new();
        
        for i in 0..iterations {
            println!("Iteration {}", i + 1);
            let conn = endpoint.connect(addr.clone(), ALPN).await?;
            let bw = benchmark_transfer(&conn, size).await?;
            bandwidths.push(bw);
            conn.close(0u32.into(), b"bye!");
            if i < iterations - 1 {
                sleep(Duration::from_millis(100)).await;
            }
        }
        
        // Calculate statistics
        let avg_bw = bandwidths.iter().sum::<f64>() / iterations as f64;
        let min_bw = bandwidths.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_bw = bandwidths.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        
        println!("Bandwidth statistics (Mbit/s):");
        println!("  Average: {:.2}", avg_bw);
        println!("  Min: {:.2}", min_bw);
        println!("  Max: {:.2}", max_bw);
    }

    Ok(())
}

async fn benchmark_transfer(conn: &Connection, size: usize) -> Result<f64> {
    let (mut send, mut recv) = conn.open_bi().await?;
    
    // Create data chunk of specified size
    let data = vec![0u8; size];
    
    // Start timing before send
    let t0 = Instant::now();
    
    // Send data
    send.write_all(&data).await?;
    send.finish()?;
    
    // Wait for small acknowledgment from server
    let ack = recv.read_to_end(8).await?; // Read up to 8 bytes for ack
    assert_eq!(&ack, b"received", "Invalid acknowledgment from server");
    
    let total_time = t0.elapsed();
    
    // Calculate bandwidth (only counting the sent data, not the tiny ack)
    let bandwidth = (size as f64 / total_time.as_secs_f64()) * 8.0 / 1_000_000.0; // Convert to Mbit/s
    
    Ok(bandwidth)
}