// CLI module - User-facing command-line interface

mod commands;
mod output;

use crate::error::{AdasaError, Result};
use crate::ipc::client::IpcClient;
use crate::ipc::protocol::{
    Command, DaemonCommand, DeleteOptions, LogOptions, ProcessId, RestartOptions, StartOptions,
    StopOptions,
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
    List,

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
        // Convert CLI command to IPC command
        let command = self.build_command()?;

        // Create IPC client and send command
        let client = IpcClient::new();
        let response = client.send_command(command)?;

        // Handle the response
        match response.result {
            Ok(data) => {
                output::print_success(&data);
                Ok(())
            }
            Err(error_msg) => {
                output::print_error(&error_msg);
                Err(AdasaError::Other(error_msg))
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

            Commands::List => Ok(Command::List),

            Commands::Logs { id, lines, follow } => Ok(Command::Logs(LogOptions {
                id: ProcessId::new(*id),
                lines: *lines,
                follow: *follow,
            })),

            Commands::Delete { id } => Ok(Command::Delete(DeleteOptions {
                id: ProcessId::new(*id),
            })),

            Commands::Daemon { command } => {
                let daemon_cmd = match command {
                    DaemonCommands::Start => DaemonCommand::Start,
                    DaemonCommands::Stop => DaemonCommand::Stop,
                    DaemonCommands::Status => DaemonCommand::Status,
                };
                Ok(Command::Daemon(daemon_cmd))
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
