// Example demonstrating process monitoring functionality

use adasa::config::{LimitAction, ProcessConfig};
use adasa::process::ProcessManager;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a process manager with integrated monitoring
    let mut manager = ProcessManager::new();

    // Spawn a process
    let config = ProcessConfig {
        name: "my-app".to_string(),
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
        limit_action: LimitAction::Log,
        stop_signal: "SIGTERM".to_string(),
        stop_timeout_secs: 10,
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
