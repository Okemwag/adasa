use adasa::config::ProcessConfig;
use adasa::process::{ProcessManager, ProcessSupervisor, SupervisorConfig};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== Process Supervisor Demo ===\n");

    // Create process manager
    let mut manager = ProcessManager::new();

    // Create supervisor with custom configuration
    let supervisor_config = SupervisorConfig {
        check_interval_secs: 2, // Check every 2 seconds
        enabled: true,
    };
    let mut supervisor = ProcessSupervisor::new(supervisor_config);

    // Configure a process that will crash (exits immediately)
    let crashing_process = ProcessConfig {
        name: "crasher".to_string(),
        script: PathBuf::from("/bin/sh"),
        args: vec!["-c".to_string(), "echo 'I will crash!'; exit 1".to_string()],
        cwd: None,
        env: HashMap::new(),
        instances: 1,
        autorestart: true,
        max_restarts: 3, // Allow 3 restarts
        restart_delay_secs: 1,
        max_memory: None,
        stop_signal: "SIGTERM".to_string(),
        stop_timeout_secs: 2,
    };

    // Configure a stable process
    let stable_process = ProcessConfig {
        name: "stable".to_string(),
        script: PathBuf::from("/bin/sleep"),
        args: vec!["30".to_string()],
        cwd: None,
        env: HashMap::new(),
        instances: 1,
        autorestart: true,
        max_restarts: 10,
        restart_delay_secs: 1,
        max_memory: None,
        stop_signal: "SIGTERM".to_string(),
        stop_timeout_secs: 2,
    };

    // Spawn processes
    println!("Spawning processes...");
    let crasher_id = manager.spawn(crashing_process).await?;
    let stable_id = manager.spawn(stable_process).await?;
    println!("  - Crasher process: {}", crasher_id);
    println!("  - Stable process: {}\n", stable_id);

    // Run supervisor for a limited time (in production, this would run indefinitely)
    println!("Starting supervisor (will run for 15 seconds)...\n");

    let supervisor_handle = tokio::spawn(async move {
        for i in 0..7 {
            // Run 7 health checks (every 2 seconds)
            tokio::time::sleep(Duration::from_secs(2)).await;

            if let Err(e) = supervisor.trigger_check(&mut manager).await {
                eprintln!("Health check error: {}", e);
            }

            // Print status
            println!("--- Health Check #{} ---", i + 1);
            for process in manager.list() {
                let (restart_count, can_restart) =
                    manager.get_restart_info(process.id).unwrap_or((0, false));

                println!(
                    "  {} [{}]: restarts={}, can_restart={}",
                    process.name, process.state, restart_count, can_restart
                );
            }
            println!();
        }
        manager
    });

    // Wait for supervisor to finish
    let mut manager = supervisor_handle.await?;

    // Final status
    println!("\n=== Final Status ===");
    for process in manager.list() {
        let (restart_count, can_restart) =
            manager.get_restart_info(process.id).unwrap_or((0, false));

        println!(
            "  {} [{}]: restarts={}, can_restart={}",
            process.name, process.state, restart_count, can_restart
        );
    }

    // Cleanup
    println!("\nCleaning up...");
    let process_ids: Vec<_> = manager.list().iter().map(|p| p.id).collect();
    for process_id in process_ids {
        let _ = manager.stop(process_id, true).await;
    }

    println!("Demo complete!");
    Ok(())
}
