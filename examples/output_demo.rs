// Demo of CLI output formatting
// This example demonstrates the enhanced output formatting capabilities

use adasa::ipc::protocol::{ProcessId, ProcessInfo, ProcessState, ProcessStats, ResponseData};
use std::time::Duration;

fn main() {
    println!("=== Adasa CLI Output Formatting Demo ===\n");

    // Demo 1: Process List
    println!("1. Process List Output:\n");
    let processes = vec![
        ProcessInfo {
            id: ProcessId::new(1),
            name: "web-server".to_string(),
            state: ProcessState::Running,
            stats: ProcessStats {
                pid: Some(12345),
                uptime: Duration::from_secs(3665),
                restarts: 0,
                cpu_usage: 2.5,
                memory_usage: 128 * 1024 * 1024,
                last_restart: None,
            },
        },
        ProcessInfo {
            id: ProcessId::new(2),
            name: "background-worker-with-long-name".to_string(),
            state: ProcessState::Running,
            stats: ProcessStats {
                pid: Some(12346),
                uptime: Duration::from_secs(7200),
                restarts: 3,
                cpu_usage: 15.8,
                memory_usage: 512 * 1024 * 1024,
                last_restart: Some(std::time::SystemTime::now() - Duration::from_secs(3600)),
            },
        },
        ProcessInfo {
            id: ProcessId::new(3),
            name: "api-service".to_string(),
            state: ProcessState::Restarting,
            stats: ProcessStats {
                pid: None,
                uptime: Duration::from_secs(45),
                restarts: 1,
                cpu_usage: 0.0,
                memory_usage: 64 * 1024 * 1024,
                last_restart: Some(std::time::SystemTime::now() - Duration::from_secs(45)),
            },
        },
        ProcessInfo {
            id: ProcessId::new(4),
            name: "database-backup".to_string(),
            state: ProcessState::Errored,
            stats: ProcessStats {
                pid: None,
                uptime: Duration::from_secs(0),
                restarts: 5,
                cpu_usage: 0.0,
                memory_usage: 0,
                last_restart: Some(std::time::SystemTime::now() - Duration::from_secs(120)),
            },
        },
        ProcessInfo {
            id: ProcessId::new(5),
            name: "cache-cleaner".to_string(),
            state: ProcessState::Stopped,
            stats: ProcessStats {
                pid: None,
                uptime: Duration::from_secs(0),
                restarts: 0,
                cpu_usage: 0.0,
                memory_usage: 0,
                last_restart: None,
            },
        },
    ];

    // Use the actual output module
    use adasa::cli::output;
    output::print_success(&ResponseData::ProcessList(processes.clone()));

    // Demo 2: Detailed Status
    println!("\n2. Detailed Status Output:\n");
    output::print_detailed_status(&processes[1]);

    // Demo 3: Success Messages
    println!("\n3. Success Messages:\n");
    output::print_success(&ResponseData::Started {
        id: ProcessId::new(6),
        name: "new-service".to_string(),
    });

    println!();
    output::print_success(&ResponseData::Stopped {
        id: ProcessId::new(3),
    });

    // Demo 4: Error Messages
    println!("\n4. Error Messages:\n");
    output::print_error("Process not found: 999");
    output::print_error("Failed to spawn process: Permission denied");

    // Demo 5: Daemon Status
    println!("\n5. Daemon Status:\n");
    output::print_success(&ResponseData::DaemonStatus {
        running: true,
        uptime: Duration::from_secs(86400 + 3600),
    });

    // Demo 6: Logs
    println!("\n6. Log Output:\n");
    let log_lines = vec![
        "[2024-01-15 10:30:45] Server started on port 3000".to_string(),
        "[2024-01-15 10:30:46] Connected to database".to_string(),
        "[2024-01-15 10:30:47] Ready to accept connections".to_string(),
        "Unformatted log line without timestamp".to_string(),
    ];
    output::print_success(&ResponseData::Logs(log_lines));

    // Demo 7: Progress Indicator
    println!("\n7. Progress Indicator (simulated):\n");
    let pb = output::create_progress_bar("Starting process...");
    std::thread::sleep(Duration::from_secs(2));
    output::finish_progress_success(pb, "Process started successfully");

    println!("\n=== Demo Complete ===\n");
}
