/// Example demonstrating graceful shutdown with custom stop signals and timeouts
///
/// This example shows:
/// 1. Starting processes with different stop signals
/// 2. Graceful shutdown with configurable timeouts
/// 3. Force kill when timeout expires
/// 4. Stopping all processes at once

use adasa::config::{LimitAction, ProcessConfig};
use adasa::process::ProcessManager;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Graceful Shutdown Demo ===\n");

    let mut manager = ProcessManager::new();

    // Example 1: Process with SIGTERM (default)
    println!("1. Starting process with SIGTERM stop signal...");
    let config1 = ProcessConfig {
        name: "sigterm-process".to_string(),
        script: PathBuf::from("/bin/sleep"),
        args: vec!["30".to_string()],
        cwd: None,
        env: HashMap::new(),
        instances: 1,
        autorestart: false,
        max_restarts: 3,
        restart_delay_secs: 1,
        max_memory: None,
        max_cpu: None,
        limit_action: LimitAction::Log,
        stop_signal: "SIGTERM".to_string(),
        stop_timeout_secs: 5,
    };

    let id1 = manager.spawn(config1).await?;
    println!("   Started process {} with PID {}", id1, manager.get_status(id1).unwrap().stats.pid);

    // Example 2: Process with SIGINT
    println!("\n2. Starting process with SIGINT stop signal...");
    let config2 = ProcessConfig {
        name: "sigint-process".to_string(),
        script: PathBuf::from("/bin/sleep"),
        args: vec!["30".to_string()],
        cwd: None,
        env: HashMap::new(),
        instances: 1,
        autorestart: false,
        max_restarts: 3,
        restart_delay_secs: 1,
        max_memory: None,
        max_cpu: None,
        limit_action: LimitAction::Log,
        stop_signal: "SIGINT".to_string(),
        stop_timeout_secs: 3,
    };

    let id2 = manager.spawn(config2).await?;
    println!("   Started process {} with PID {}", id2, manager.get_status(id2).unwrap().stats.pid);

    // Example 3: Process with custom timeout
    println!("\n3. Starting process with custom 2-second timeout...");
    let config3 = ProcessConfig {
        name: "timeout-process".to_string(),
        script: PathBuf::from("/bin/sleep"),
        args: vec!["30".to_string()],
        cwd: None,
        env: HashMap::new(),
        instances: 1,
        autorestart: false,
        max_restarts: 3,
        restart_delay_secs: 1,
        max_memory: None,
        max_cpu: None,
        limit_action: LimitAction::Log,
        stop_signal: "SIGTERM".to_string(),
        stop_timeout_secs: 2,
    };

    let id3 = manager.spawn(config3).await?;
    println!("   Started process {} with PID {}", id3, manager.get_status(id3).unwrap().stats.pid);

    // List all processes
    println!("\n4. Current processes:");
    for process in manager.list() {
        println!("   - {} (ID: {}, PID: {}, Signal: {}, Timeout: {}s, State: {})",
            process.name,
            process.id,
            process.stats.pid,
            process.config.stop_signal,
            process.config.stop_timeout_secs,
            process.state
        );
    }

    // Wait a bit
    println!("\n5. Waiting 2 seconds...");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Gracefully stop first process
    println!("\n6. Gracefully stopping first process (SIGTERM)...");
    let start = std::time::Instant::now();
    manager.stop(id1, false).await?;
    let elapsed = start.elapsed();
    println!("   Stopped in {:?}", elapsed);
    
    let process1 = manager.get_status(id1).unwrap();
    println!("   State: {}", process1.state);

    // Gracefully stop second process
    println!("\n7. Gracefully stopping second process (SIGINT)...");
    let start = std::time::Instant::now();
    manager.stop(id2, false).await?;
    let elapsed = start.elapsed();
    println!("   Stopped in {:?}", elapsed);
    
    let process2 = manager.get_status(id2).unwrap();
    println!("   State: {}", process2.state);

    // Force stop third process
    println!("\n8. Force stopping third process (immediate SIGKILL)...");
    let start = std::time::Instant::now();
    manager.stop(id3, true).await?;
    let elapsed = start.elapsed();
    println!("   Stopped in {:?}", elapsed);
    
    let process3 = manager.get_status(id3).unwrap();
    println!("   State: {}", process3.state);

    // Demonstrate stop_all
    println!("\n9. Demonstrating stop_all with multiple processes...");
    
    // Spawn a few more processes
    for i in 0..3 {
        let config = ProcessConfig {
            name: format!("batch-process-{}", i),
            script: PathBuf::from("/bin/sleep"),
            args: vec!["30".to_string()],
            cwd: None,
            env: HashMap::new(),
            instances: 1,
            autorestart: false,
            max_restarts: 3,
            restart_delay_secs: 1,
            max_memory: None,
            max_cpu: None,
            limit_action: LimitAction::Log,
            stop_signal: "SIGTERM".to_string(),
            stop_timeout_secs: 5,
        };
        
        let id = manager.spawn(config).await?;
        println!("   Started {} (ID: {})", format!("batch-process-{}", i), id);
    }

    println!("\n10. Stopping all processes gracefully...");
    let start = std::time::Instant::now();
    manager.stop_all().await?;
    let elapsed = start.elapsed();
    println!("   All processes stopped in {:?}", elapsed);

    // Verify all stopped
    println!("\n11. Final process states:");
    for process in manager.list() {
        println!("   - {} (ID: {}): {}", process.name, process.id, process.state);
    }

    println!("\n=== Demo Complete ===");
    
    Ok(())
}
