// CLI module - User-facing command-line interface

mod commands;
pub mod output;

use crate::error::{AdasaError, Result};
use crate::ipc::client::IpcClient;
use crate::ipc::protocol::{
    Command, DeleteOptions, LogOptions, ProcessId, RestartOptions, StartOptions, StopOptions,
};
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::path::PathBuf;

/// Adasa - A fast, open-source process manager
#[derive(Parser)]
#[command(name = "adasa")]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a new process
    Start {
        /// Path to the script or executable to run
        script: PathBuf,

        /// Name for the process (defaults to script name)
        #[arg(short, long)]
        name: Option<String>,

        /// Number of instances to start
        #[arg(short, long, default_value = "1")]
        instances: usize,

        /// Working directory for the process
        #[arg(short, long)]
        cwd: Option<PathBuf>,

        /// Environment variables (KEY=VALUE format)
        #[arg(short, long)]
        env: Vec<String>,

        /// Arguments to pass to the script
        #[arg(last = true)]
        args: Vec<String>,
    },

    /// Stop a running process
    Stop {
        /// Process ID to stop
        id: u64,

        /// Force kill the process (SIGKILL)
        #[arg(short, long)]
        force: bool,
    },

    /// Restart a process
    Restart {
        /// Process ID or name to restart
        id: String,

        /// Perform rolling restart for multi-instance processes
        #[arg(short, long)]
        rolling: bool,
    },

    /// List all managed processes
    List {
        /// Show detailed information for each process
        #[arg(short, long)]
        detailed: bool,
    },

    /// View process logs
    Logs {
        /// Process ID to view logs for
        id: u64,

        /// Number of lines to display
        #[arg(short, long)]
        lines: Option<usize>,

        /// Follow log output (stream)
        #[arg(short, long)]
        follow: bool,
    },

    /// Delete a stopped process
    Delete {
        /// Process ID to delete
        id: u64,
    },

    /// Manage the daemon
    Daemon {
        #[command(subcommand)]
        command: DaemonCommands,
    },
}

#[derive(Subcommand)]
enum DaemonCommands {
    /// Start the daemon
    Start,
    /// Stop the daemon
    Stop,
    /// Check daemon status
    Status,
}

impl Cli {
    /// Run the CLI application
    pub fn run() -> Result<()> {
        let cli = Cli::parse();
        cli.execute()
    }

    /// Execute the parsed command
    fn execute(&self) -> Result<()> {
        // Handle daemon commands specially (they don't require IPC)
        if let Commands::Daemon { command } = &self.command {
            return self.handle_daemon_command(command);
        }

        // Check if this is a long-running operation
        let is_long_operation = matches!(
            &self.command,
            Commands::Start { .. } | Commands::Restart { .. }
        );

        // Show progress indicator for long operations
        let progress = if is_long_operation {
            Some(output::create_progress_bar("Processing..."))
        } else {
            None
        };

        // Convert CLI command to IPC command
        let command = self.build_command()?;

        // Create IPC client and send command
        let client = IpcClient::new();
        let response = client.send_command(command);

        // Clear progress indicator
        if let Some(pb) = progress {
            match &response {
                Ok(_) => output::finish_progress_success(pb, "Done"),
                Err(_) => output::finish_progress_error(pb, "Failed"),
            }
        }

        // Handle the response
        match response {
            Ok(response) => match response.result {
                Ok(data) => {
                    // Check if we need detailed output
                    let show_detailed =
                        matches!(&self.command, Commands::List { detailed } if *detailed);

                    if show_detailed {
                        if let crate::ipc::protocol::ResponseData::ProcessList(processes) = &data {
                            for process in processes {
                                output::print_detailed_status(process);
                            }
                        } else {
                            output::print_success(&data);
                        }
                    } else {
                        output::print_success(&data);
                    }
                    Ok(())
                }
                Err(error_msg) => {
                    output::print_error(&error_msg);
                    Err(AdasaError::Other(error_msg))
                }
            },
            Err(e) => {
                output::print_error(&e.to_string());
                Err(e)
            }
        }
    }

    /// Handle daemon management commands
    fn handle_daemon_command(&self, command: &DaemonCommands) -> Result<()> {
        use crate::daemon::DaemonManager;
        use std::process::Command;

        let manager = DaemonManager::new();

        match command {
            DaemonCommands::Start => {
                // Check if daemon is already running
                if manager.is_running() {
                    output::print_info("Daemon is already running");
                    return Ok(());
                }

                output::print_info("Starting daemon...");

                // Get the path to the daemon binary
                let daemon_binary = std::env::current_exe()
                    .map_err(|e| {
                        AdasaError::Other(format!("Failed to get current executable: {}", e))
                    })?
                    .parent()
                    .ok_or_else(|| {
                        AdasaError::Other("Failed to get executable directory".to_string())
                    })?
                    .join("adasa-daemon");

                // Check if daemon binary exists
                if !daemon_binary.exists() {
                    return Err(AdasaError::Other(format!(
                        "Daemon binary not found at: {}",
                        daemon_binary.display()
                    )));
                }

                // Spawn the daemon process with --daemonize flag
                let _child = Command::new(&daemon_binary)
                    .arg("--daemonize")
                    .spawn()
                    .map_err(|e| AdasaError::Other(format!("Failed to start daemon: {}", e)))?;

                // Wait a moment for daemon to start
                std::thread::sleep(std::time::Duration::from_millis(500));

                // Check if daemon is now running
                if manager.is_running() {
                    output::print_success_msg(&format!(
                        "Daemon started successfully (PID: {})",
                        manager.get_pid().unwrap()
                    ));
                    Ok(())
                } else {
                    Err(AdasaError::Other("Daemon failed to start".to_string()))
                }
            }

            DaemonCommands::Stop => {
                // Check if daemon is running
                if !manager.is_running() {
                    output::print_info("Daemon is not running");
                    return Ok(());
                }

                output::print_info("Stopping daemon...");

                // Stop the daemon with 10 second timeout
                manager.stop_daemon(10)?;

                output::print_success_msg("Daemon stopped successfully");
                Ok(())
            }

            DaemonCommands::Status => {
                let status = manager.get_status();

                if status.running {
                    output::print_success_msg(&format!(
                        "Daemon is running (PID: {})\nPID file: {}",
                        status.pid.unwrap(),
                        status.pid_file.display()
                    ));
                } else {
                    output::print_info(&format!(
                        "Daemon is not running\nPID file: {}",
                        status.pid_file.display()
                    ));
                }

                Ok(())
            }
        }
    }

    /// Build an IPC command from the CLI arguments
    fn build_command(&self) -> Result<Command> {
        match &self.command {
            Commands::Start {
                script,
                name,
                instances,
                cwd,
                env,
                args,
            } => {
                // Parse environment variables
                let env_map = parse_env_vars(env)?;

                Ok(Command::Start(StartOptions {
                    script: script.clone(),
                    name: name.clone(),
                    instances: *instances,
                    env: env_map,
                    cwd: cwd.clone(),
                    args: args.clone(),
                }))
            }

            Commands::Stop { id, force } => Ok(Command::Stop(StopOptions {
                id: ProcessId::new(*id),
                force: *force,
            })),

            Commands::Restart { id, rolling } => Ok(Command::Restart(RestartOptions {
                target: id.clone(),
                rolling: *rolling,
            })),

            Commands::List { detailed } => {
                // For now, we'll just use the List command
                // The detailed flag can be used in future enhancements
                let _ = detailed;
                Ok(Command::List)
            }

            Commands::Logs { id, lines, follow } => Ok(Command::Logs(LogOptions {
                id: ProcessId::new(*id),
                lines: *lines,
                follow: *follow,
            })),

            Commands::Delete { id } => Ok(Command::Delete(DeleteOptions {
                id: ProcessId::new(*id),
            })),

            Commands::Daemon { .. } => {
                // Daemon commands are handled separately, not via IPC
                unreachable!("Daemon commands should be handled by handle_daemon_command")
            }
        }
    }
}

/// Parse environment variables from KEY=VALUE format
fn parse_env_vars(env_vars: &[String]) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();

    for env_str in env_vars {
        if let Some((key, value)) = env_str.split_once('=') {
            map.insert(key.to_string(), value.to_string());
        } else {
            return Err(AdasaError::ConfigError(format!(
                "Invalid environment variable format: '{}'. Expected KEY=VALUE",
                env_str
            )));
        }
    }

    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_env_vars() {
        let env_vars = vec!["NODE_ENV=production".to_string(), "PORT=3000".to_string()];
        let result = parse_env_vars(&env_vars).unwrap();
        assert_eq!(result.get("NODE_ENV"), Some(&"production".to_string()));
        assert_eq!(result.get("PORT"), Some(&"3000".to_string()));
    }

    #[test]
    fn test_parse_env_vars_invalid() {
        let env_vars = vec!["INVALID".to_string()];
        let result = parse_env_vars(&env_vars);
        assert!(result.is_err());
    }
}
