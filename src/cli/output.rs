// Output formatting and display for CLI

use crate::ipc::protocol::{ProcessInfo, ProcessState, ResponseData};
use std::time::Duration;

/// Print a success response to stdout
pub fn print_success(data: &ResponseData) {
    match data {
        ResponseData::Started { id, name } => {
            println!("✓ Process started successfully");
            println!("  ID:   {}", id);
            println!("  Name: {}", name);
        }

        ResponseData::Stopped { id } => {
            println!("✓ Process {} stopped successfully", id);
        }

        ResponseData::Restarted { id } => {
            println!("✓ Process {} restarted successfully", id);
        }

        ResponseData::ProcessList(processes) => {
            if processes.is_empty() {
                println!("No processes are currently running");
            } else {
                print_process_table(processes);
            }
        }

        ResponseData::Logs(lines) => {
            for line in lines {
                println!("{}", line);
            }
        }

        ResponseData::Deleted { id } => {
            println!("✓ Process {} deleted successfully", id);
        }

        ResponseData::DaemonStatus { running, uptime } => {
            if *running {
                println!("✓ Daemon is running");
                println!("  Uptime: {}", format_duration(uptime));
            } else {
                println!("✗ Daemon is not running");
            }
        }

        ResponseData::Success(message) => {
            println!("✓ {}", message);
        }
    }
}

/// Print an error message to stderr
pub fn print_error(error: &str) {
    eprintln!("✗ Error: {}", error);
}

/// Print a formatted table of processes
fn print_process_table(processes: &[ProcessInfo]) {
    // Print header
    println!(
        "{:<6} {:<20} {:<12} {:<8} {:<10} {:<12} {:<10}",
        "ID", "Name", "State", "PID", "Uptime", "Restarts", "Memory"
    );
    println!("{}", "─".repeat(88));

    // Print each process
    for process in processes {
        let pid_str = process
            .stats
            .pid
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".to_string());

        let uptime_str = format_duration(&process.stats.uptime);
        let memory_str = format_memory(process.stats.memory_usage);
        let state_str = format_state(&process.state);

        println!(
            "{:<6} {:<20} {:<12} {:<8} {:<10} {:<12} {:<10}",
            process.id,
            truncate(&process.name, 20),
            state_str,
            pid_str,
            uptime_str,
            process.stats.restarts,
            memory_str,
        );
    }

    println!();
    println!("Total: {} process(es)", processes.len());
}

/// Format a process state with color coding
fn format_state(state: &ProcessState) -> String {
    match state {
        ProcessState::Running => format!("\x1b[32m{}\x1b[0m", state), // Green
        ProcessState::Starting => format!("\x1b[33m{}\x1b[0m", state), // Yellow
        ProcessState::Restarting => format!("\x1b[33m{}\x1b[0m", state), // Yellow
        ProcessState::Stopping => format!("\x1b[33m{}\x1b[0m", state), // Yellow
        ProcessState::Stopped => format!("\x1b[90m{}\x1b[0m", state), // Gray
        ProcessState::Errored => format!("\x1b[31m{}\x1b[0m", state), // Red
    }
}

/// Format a duration in human-readable format
fn format_duration(duration: &Duration) -> String {
    let secs = duration.as_secs();

    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}

/// Format memory usage in human-readable format
fn format_memory(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes < KB {
        format!("{}B", bytes)
    } else if bytes < MB {
        format!("{}KB", bytes / KB)
    } else if bytes < GB {
        format!("{}MB", bytes / MB)
    } else {
        format!("{}GB", bytes / GB)
    }
}

/// Truncate a string to a maximum length
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(&Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(&Duration::from_secs(90)), "1m");
        assert_eq!(format_duration(&Duration::from_secs(3700)), "1h");
        assert_eq!(format_duration(&Duration::from_secs(90000)), "1d");
    }

    #[test]
    fn test_format_memory() {
        assert_eq!(format_memory(512), "512B");
        assert_eq!(format_memory(2048), "2KB");
        assert_eq!(format_memory(2 * 1024 * 1024), "2MB");
        assert_eq!(format_memory(3 * 1024 * 1024 * 1024), "3GB");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("this is a very long string", 10), "this is...");
    }
}
