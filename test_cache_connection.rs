#!/usr/bin/env rust-script

//! Simple test script to verify that the LSP client connects to the daemon
//! and that the call hierarchy cache is working correctly.
//!
//! Usage: Run this from the probe directory:
//! ```
//! ./target/debug/probe lsp start -f &  # Start daemon in foreground
//! cargo run --bin test_cache_connection
//! ```

use std::path::PathBuf;
use std::process;
use std::time::Duration;
use tokio::time::sleep;

use probe_code::lsp_integration::{LspClient, LspConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging to see debug output
    env_logger::init();

    println!("🧪 Testing LSP client-daemon connection and cache functionality...\n");

    // Use a simple Rust file for testing
    let test_file = PathBuf::from("src/main.rs");
    if !test_file.exists() {
        println!("❌ Test file {:?} not found", test_file);
        process::exit(1);
    }

    // Create LspClient with daemon enabled
    let config = LspConfig {
        use_daemon: true,
        workspace_hint: None,
        timeout_ms: 10000,
    };

    println!("📡 Creating LSP client...");
    let mut client = match LspClient::new(config).await {
        Ok(client) => {
            println!("✅ LSP client created successfully");
            client
        }
        Err(e) => {
            println!("❌ Failed to create LSP client: {}", e);
            process::exit(1);
        }
    };

    println!("\n📊 Getting daemon status...");
    match client.get_status().await {
        Ok(status) => {
            println!("✅ Daemon is running:");
            println!("   Uptime: {:?}", status.uptime);
            println!("   Total requests: {}", status.total_requests);
            println!("   Active connections: {}", status.active_connections);
        }
        Err(e) => {
            println!("❌ Failed to get daemon status: {}", e);
            process::exit(1);
        }
    }

    println!("\n🔍 Testing call hierarchy cache...");
    println!("Making first call hierarchy request...");

    // First request - should hit the language server
    let start1 = std::time::Instant::now();
    match client.get_symbol_info(&test_file, "main", 1, 0).await {
        Ok(Some(info)) => {
            let elapsed1 = start1.elapsed();
            println!("✅ First request completed in {:?}", elapsed1);
            println!("   Symbol: {}", info.name);
            if let Some(hierarchy) = &info.call_hierarchy {
                println!("   Incoming calls: {}", hierarchy.incoming_calls.len());
                println!("   Outgoing calls: {}", hierarchy.outgoing_calls.len());
            } else {
                println!("   No call hierarchy information");
            }
        }
        Ok(None) => {
            println!("⚠️ First request returned no symbol info");
        }
        Err(e) => {
            println!("❌ First request failed: {}", e);
        }
    }

    println!("\n⏱️ Waiting 1 second before second request...");
    sleep(Duration::from_secs(1)).await;

    println!("Making second identical call hierarchy request (should hit cache)...");

    // Second request - should hit the cache
    let start2 = std::time::Instant::now();
    match client.get_symbol_info(&test_file, "main", 1, 0).await {
        Ok(Some(info)) => {
            let elapsed2 = start2.elapsed();
            println!("✅ Second request completed in {:?}", elapsed2);
            println!("   Symbol: {}", info.name);
            if let Some(hierarchy) = &info.call_hierarchy {
                println!("   Incoming calls: {}", hierarchy.incoming_calls.len());
                println!("   Outgoing calls: {}", hierarchy.outgoing_calls.len());
            } else {
                println!("   No call hierarchy information");
            }
        }
        Ok(None) => {
            println!("⚠️ Second request returned no symbol info");
        }
        Err(e) => {
            println!("❌ Second request failed: {}", e);
        }
    }

    println!("\n📋 Getting daemon logs to verify cache hit...");
    match client.get_logs(50).await {
        Ok(logs) => {
            let cache_hits: Vec<_> = logs
                .iter()
                .filter(|log| log.message.contains("Call hierarchy cache HIT"))
                .collect();

            if !cache_hits.is_empty() {
                println!("✅ Found {} cache hit(s) in logs:", cache_hits.len());
                for hit in cache_hits.iter().take(3) {
                    println!("   📝 {}", hit.message);
                }
            } else {
                println!("⚠️ No cache hits found in recent logs");
                println!("   Recent log entries:");
                for log in logs.iter().take(10) {
                    println!("   📝 {}: {}", log.level, log.message);
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to get daemon logs: {}", e);
        }
    }

    println!("\n🎯 Test completed!");
    println!("\n💡 To see full debug logs:");
    println!("   RUST_LOG=debug ./target/debug/probe search \"main\" ./src --lsp");

    Ok(())
}
