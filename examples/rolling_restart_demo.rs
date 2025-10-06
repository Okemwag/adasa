/// Example demonstrating rolling restart functionality for multi-instance processes
///
/// This example shows how to:
/// 1. Spawn multiple instances of the same application
/// 2. Perform a rolling restart to update all instances sequentially
/// 3. Maintain availability during the restart process
///
/// Run with: cargo run --example rolling_restart_demo

use adasa::config::ProcessConfig;
use adasa::process::ProcessManager;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Rolling Restart Demo ===\n");

    // Create a process manager
    let mut manager = ProcessManager::new();

    // Spawn 3 instances of a simple process
    println!("Spawning 3 instances of the application...");
    let mut instance_ids = Vec::new();

    for i in 0..3 {
        let config = ProcessConfig {
            name: format!("web-server-{}", i),
            script: PathBuf::from("/bin/sleep"),
            args: vec!["60".to_string()],
            cwd: None,
            env: HashMap::new(),
            instances: 1,
            autorestart: true,
            max_restarts: 10,
            restart_delay_secs: 1,
            max_memory: None,
            max_cpu: None,
            limit_action: adasa::config::LimitAction::Log,
            stop_signal: "SIGTERM".to_string(),
            stop_timeout_secs: 5,
        };

        let id = manager.spawn(config).await?;
        instance_ids.push(id);
        println!("  ✓ Spawned instance {} (ID: {})", i, id);
    }

    // Display initial status
    println!("\nInitial process status:");
    for id in &instance_ids {
        if let Some(process) = manager.get_status(*id) {
            println!(
                "  {} - PID: {}, State: {}, Restarts: {}",
                process.name, process.stats.pid, process.state, process.stats.restarts
            );
        }
    }

    // Wait a moment for processes to stabilize
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Perform rolling restart
    println!("\n🔄 Starting rolling restart...");
    println!("This will restart each instance sequentially with health checks\n");

    let health_check_delay = Duration::from_secs(2);
    let result = manager
        .rolling_restart("web-server", health_check_delay)
        .await?;

    println!("\n✅ Rolling restart completed: {} instances restarted\n", result);

    // Display final status
    println!("Final process status:");
    for id in &instance_ids {
        if let Some(process) = manager.get_status(*id) {
            println!(
                "  {} - PID: {}, State: {}, Restarts: {}",
                process.name, process.stats.pid, process.state, process.stats.restarts
            );
        }
    }

    // Cleanup
    println!("\nCleaning up...");
    for id in instance_ids {
        manager.stop(id, true).await?;
        println!("  ✓ Stopped instance {}", id);
    }

    println!("\n=== Demo Complete ===");

    Ok(())
}
