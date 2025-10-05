// Example demonstrating process monitoring functionality
// This is not meant to be compiled, just for documentation

use adasa::process::{ProcessConfig, ProcessManager};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a process manager with integrated monitoring
    let mut manager = ProcessManager::new();

    // Spawn a process
    let config = ProcessConfig {
        name: "my-app".to_string(),
        script: "/usr/bin/node".into(),
        args: vec!["server.js".to_string()],
        // ... other config
    };

    let process_id = manager.spawn(config).await?;

    // Monitoring loop
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Update statistics for all processes
        manager.update_stats()?;

        // Get process status with updated stats
        if let Some(process) = manager.get_status(process_id) {
            println!("Process: {}", process.name);
            println!("  State: {}", process.state);
            println!("  PID: {}", process.stats.pid);
            println!("  CPU: {:.2}%", process.stats.cpu_usage);
            println!("  Memory: {} MB", process.stats.memory_usage / 1024 / 1024);
            println!("  Uptime: {:?}", process.stats.uptime());
            println!("  Restarts: {}", process.stats.restarts);
        }

        // Detect any crashed processes
        let crashed = manager.detect_crashes();
        for crashed_id in crashed {
            println!("Process {} has crashed!", crashed_id);
            // Could trigger restart logic here
        }

        // Check if a specific process is alive
        if !manager.is_alive(process_id) {
            println!("Process is no longer alive!");
            break;
        }
    }

    Ok(())
}
