// Output formatting and display for CLI

use crate::ipc::protocol::{ProcessInfo, ProcessState, ResponseData};
use chrono::{DateTime, Local};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;
use tabled::{
    settings::{object::Rows, Alignment, Modify, Style},
    Table, Tabled,
};

/// Print a success response to stdout
pub fn print_success(data: &ResponseData) {
    match data {
        ResponseData::Started { id, name } => {
            println!("{}", "✓ Process started successfully".green().bold());
            println!("  {}: {}", "ID".bold(), id);
            println!("  {}: {}", "Name".bold(), name.cyan());
        }

        ResponseData::Stopped { id } => {
            println!(
                "{}",
                format!("✓ Process {} stopped successfully", id)
                    .green()
                    .bold()
            );
        }

        ResponseData::Restarted { id } => {
            println!(
                "{}",
                format!("✓ Process {} restarted successfully", id)
                    .green()
                    .bold()
            );
        }

        ResponseData::ProcessList(processes) => {
            if processes.is_empty() {
                println!("{}", "No processes are currently running".yellow());
            } else {
                print_process_table(processes);
            }
        }

        ResponseData::Logs(lines) => {
            print_logs(lines);
        }

        ResponseData::Deleted { id } => {
            println!(
                "{}",
                format!("✓ Process {} deleted successfully", id)
                    .green()
                    .bold()
            );
        }

        ResponseData::DaemonStatus { running, uptime } => {
            if *running {
                println!("{}", "✓ Daemon is running".green().bold());
                println!("  {}: {}", "Uptime".bold(), format_duration(uptime));
            } else {
                println!("{}", "✗ Daemon is not running".red().bold());
            }
        }

        ResponseData::Success(message) => {
            println!("{} {}", "✓".green().bold(), message);
        }
    }
}

/// Print an error message to stderr
pub fn print_error(error: &str) {
    eprintln!("{} {}", "✗ Error:".red().bold(), error);
}

/// Print an info message
pub fn print_info(message: &str) {
    println!("{} {}", "ℹ".blue().bold(), message);
}

/// Print a success message
pub fn print_success_msg(message: &str) {
    println!("{} {}", "✓".green().bold(), message);
}

/// Print a formatted table of processes
fn print_process_table(processes: &[ProcessInfo]) {
    #[derive(Tabled)]
    struct ProcessRow {
        #[tabled(rename = "ID")]
        id: String,
        #[tabled(rename = "Name")]
        name: String,
        #[tabled(rename = "State")]
        state: String,
        #[tabled(rename = "PID")]
        pid: String,
        #[tabled(rename = "CPU")]
        cpu: String,
        #[tabled(rename = "Memory")]
        memory: String,
        #[tabled(rename = "Uptime")]
        uptime: String,
        #[tabled(rename = "Restarts")]
        restarts: String,
    }

    let rows: Vec<ProcessRow> = processes
        .iter()
        .map(|p| ProcessRow {
            id: p.id.to_string(),
            name: truncate(&p.name, 20),
            state: format_state_colored(&p.state),
            pid: p
                .stats
                .pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "-".to_string()),
            cpu: format!("{:.1}%", p.stats.cpu_usage),
            memory: format_memory(p.stats.memory_usage),
            uptime: format_duration(&p.stats.uptime),
            restarts: p.stats.restarts.to_string(),
        })
        .collect();

    let mut table = Table::new(rows);
    table
        .with(Style::rounded())
        .with(Modify::new(Rows::first()).with(Alignment::center()));

    println!("\n{}\n", table);
    println!(
        "{}",
        format!("Total: {} process(es)", processes.len())
            .dimmed()
            .italic()
    );
}

/// Print detailed status view for a single process
pub fn print_detailed_status(process: &ProcessInfo) {
    println!("\n{}", "Process Details".bold().underline());
    println!();
    println!("  {:<15} {}", "ID:".bold(), process.id);
    println!("  {:<15} {}", "Name:".bold(), process.name.cyan());
    println!(
        "  {:<15} {}",
        "State:".bold(),
        format_state_colored(&process.state)
    );

    if let Some(pid) = process.stats.pid {
        println!("  {:<15} {}", "PID:".bold(), pid);
    }

    println!(
        "  {:<15} {:.1}%",
        "CPU Usage:".bold(),
        process.stats.cpu_usage
    );
    println!(
        "  {:<15} {}",
        "Memory:".bold(),
        format_memory(process.stats.memory_usage)
    );
    println!(
        "  {:<15} {}",
        "Uptime:".bold(),
        format_duration(&process.stats.uptime)
    );
    println!("  {:<15} {}", "Restarts:".bold(), process.stats.restarts);

    if let Some(last_restart) = process.stats.last_restart {
        let datetime: DateTime<Local> = last_restart.into();
        println!(
            "  {:<15} {}",
            "Last Restart:".bold(),
            datetime.format("%Y-%m-%d %H:%M:%S")
        );
    }

    println!();
}

/// Print logs with timestamps
fn print_logs(lines: &[String]) {
    if lines.is_empty() {
        println!("{}", "No logs available".yellow());
        return;
    }

    println!("\n{}", "Logs".bold().underline());
    println!();

    for line in lines {
        // Check if line already has a timestamp
        if line.starts_with('[') {
            // Already formatted with timestamp
            println!("{}", line);
        } else {
            // Add timestamp
            let now = Local::now();
            println!(
                "{} {}",
                format!("[{}]", now.format("%H:%M:%S")).dimmed(),
                line
            );
        }
    }

    println!();
}

/// Format a process state with color coding
fn format_state_colored(state: &ProcessState) -> String {
    match state {
        ProcessState::Running => state.to_string().green().to_string(),
        ProcessState::Starting => state.to_string().yellow().to_string(),
        ProcessState::Restarting => state.to_string().yellow().to_string(),
        ProcessState::Stopping => state.to_string().yellow().to_string(),
        ProcessState::Stopped => state.to_string().bright_black().to_string(),
        ProcessState::Errored => state.to_string().red().bold().to_string(),
    }
}

/// Format a duration in human-readable format
fn format_duration(duration: &Duration) -> String {
    let secs = duration.as_secs();

    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        let mins = secs / 60;
        let secs = secs % 60;
        if secs > 0 {
            format!("{}m {}s", mins, secs)
        } else {
            format!("{}m", mins)
        }
    } else if secs < 86400 {
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        if mins > 0 {
            format!("{}h {}m", hours, mins)
        } else {
            format!("{}h", hours)
        }
    } else {
        let days = secs / 86400;
        let hours = (secs % 86400) / 3600;
        if hours > 0 {
            format!("{}d {}h", days, hours)
        } else {
            format!("{}d", days)
        }
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
        format!("{:.1}KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.1}MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.2}GB", bytes as f64 / GB as f64)
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

/// Create a progress bar for long operations
pub fn create_progress_bar(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb
}

/// Finish a progress bar with success
pub fn finish_progress_success(pb: ProgressBar, message: &str) {
    pb.finish_with_message(format!("{} {}", "✓".green(), message));
}

/// Finish a progress bar with error
pub fn finish_progress_error(pb: ProgressBar, message: &str) {
    pb.finish_with_message(format!("{} {}", "✗".red(), message));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(&Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(&Duration::from_secs(90)), "1m 30s");
        assert_eq!(format_duration(&Duration::from_secs(3700)), "1h 1m");
        assert_eq!(format_duration(&Duration::from_secs(90000)), "1d 1h");
    }

    #[test]
    fn test_format_memory() {
        assert_eq!(format_memory(512), "512B");
        assert_eq!(format_memory(2048), "2.0KB");
        assert_eq!(format_memory(2 * 1024 * 1024), "2.0MB");
        assert_eq!(format_memory(3 * 1024 * 1024 * 1024), "3.00GB");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("this is a very long string", 10), "this is...");
    }
}
