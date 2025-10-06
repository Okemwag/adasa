// Example demonstrating restart logic with backoff
// This is not meant to be compiled, just for documentation

use adasa::config::ProcessConfig;
use adasa::ipc::protocol::ProcessId;
use adasa::process::{BackoffStrategy, ProcessManager, RestartPolicy};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a process manager
    let mut manager = ProcessManager::new();

    // Configure a process with restart settings
    let config = ProcessConfig {
        name: "my-app".to_string(),
        script: "/usr/bin/node".into(),
        args: vec!["server.js".to_string()],
        cwd: None,
        env: std::collections::HashMap::new(),
        instances: 1,
        autorestart: true,     // Enable automatic restart
        max_restarts: 10,      // Max 10 restarts in 60 seconds
        restart_delay_secs: 1, // Initial delay of 1 second
        max_memory: None,
        max_cpu: None,
        limit_action: adasa::config::LimitAction::Log,
        stop_signal: "SIGTERM".to_string(),
        stop_timeout_secs: 10,
    };

    let process_id = manager.spawn(config).await?;
    println!("Process spawned with ID: {}", process_id);

    // Monitoring loop
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Update process statistics
        manager.update_stats()?;

        // Detect any crashed processes
        let crashed = manager.detect_crashes();

        for crashed_id in crashed {
            println!("Process {} crashed!", crashed_id);

            // Get restart info before attempting restart
            if let Some((restart_count, can_restart)) = manager.get_restart_info(crashed_id) {
                println!("  Restart count: {}", restart_count);
                println!("  Can restart: {}", can_restart);
            }

            // Try automatic restart with backoff
            match manager.try_auto_restart(crashed_id).await {
                Ok(true) => {
                    println!("  ✓ Process restarted successfully");

                    // Show updated stats
                    if let Some(process) = manager.get_status(crashed_id) {
                        println!("  New PID: {}", process.stats.pid);
                        println!("  Total restarts: {}", process.stats.restarts);
                        if let Some(last_restart) = process.stats.last_restart {
                            println!("  Last restart: {:?}", last_restart);
                        }
                    }
                }
                Ok(false) => {
                    println!("  ✗ Restart blocked by policy (max restarts reached)");
                    println!("  Process will remain in errored state");
                }
                Err(e) => {
                    println!("  ✗ Restart failed: {}", e);
                }
            }
        }

        // Display status of all processes
        for process in manager.list() {
            println!("\nProcess: {} ({})", process.name, process.id);
            println!("  State: {}", process.state);
            println!("  PID: {}", process.stats.pid);
            println!("  Uptime: {:?}", process.stats.uptime());
            println!("  Restarts: {}", process.stats.restarts);
            println!("  CPU: {:.2}%", process.stats.cpu_usage);
            println!("  Memory: {} MB", process.stats.memory_usage / 1024 / 1024);
        }
    }
}

// Example: Manual restart
async fn manual_restart_example(manager: &mut ProcessManager, process_id: ProcessId) {
    println!("Manually restarting process...");

    match manager.restart(process_id).await {
        Ok(()) => {
            println!("✓ Process restarted successfully");

            if let Some(process) = manager.get_status(process_id) {
                println!("  New PID: {}", process.stats.pid);
                println!("  Restart count: {}", process.stats.restarts);
            }
        }
        Err(e) => {
            println!("✗ Restart failed: {}", e);
        }
    }
}

// Example: Custom restart policy
fn custom_policy_example() {
    // Create a custom restart policy
    let policy = RestartPolicy {
        enabled: true,
        max_restarts: 5,       // Only 5 restarts
        time_window_secs: 120, // In 2 minutes
        initial_delay_secs: 2, // Start with 2 second delay
        backoff_strategy: BackoffStrategy::Exponential {
            max_delay_secs: 30, // Cap at 30 seconds
        },
    };

    // Backoff progression:
    // Restart 0: 2 seconds
    // Restart 1: 4 seconds
    // Restart 2: 8 seconds
    // Restart 3: 16 seconds
    // Restart 4: 30 seconds (capped)
    // Restart 5: blocked (max_restarts reached)
}
