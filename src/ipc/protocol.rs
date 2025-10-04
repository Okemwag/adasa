// IPC Protocol definitions for client-daemon communication

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// Unique identifier for a process
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProcessId(pub u64);

impl ProcessId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for ProcessId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Process state in the lifecycle
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessState {
    Starting,
    Running,
    Stopping,
    Stopped,
    Errored,
    Restarting,
}

impl std::fmt::Display for ProcessState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessState::Starting => write!(f, "starting"),
            ProcessState::Running => write!(f, "running"),
            ProcessState::Stopping => write!(f, "stopping"),
            ProcessState::Stopped => write!(f, "stopped"),
            ProcessState::Errored => write!(f, "errored"),
            ProcessState::Restarting => write!(f, "restarting"),
        }
    }
}

/// Process statistics and metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStats {
    pub pid: Option<u32>,
    pub uptime: Duration,
    pub restarts: usize,
    pub cpu_usage: f32,
    pub memory_usage: u64,
    pub last_restart: Option<SystemTime>,
}

impl Default for ProcessStats {
    fn default() -> Self {
        Self {
            pid: None,
            uptime: Duration::from_secs(0),
            restarts: 0,
            cpu_usage: 0.0,
            memory_usage: 0,
            last_restart: None,
        }
    }
}

/// Options for starting a process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartOptions {
    pub script: PathBuf,
    pub name: Option<String>,
    pub instances: usize,
    pub env: HashMap<String, String>,
    pub cwd: Option<PathBuf>,
    pub args: Vec<String>,
}

/// Options for stopping a process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopOptions {
    pub id: ProcessId,
    pub force: bool,
}

/// Options for restarting a process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartOptions {
    pub id: ProcessId,
}

/// Options for viewing logs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogOptions {
    pub id: ProcessId,
    pub lines: Option<usize>,
    pub follow: bool,
}

/// Options for deleting a process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteOptions {
    pub id: ProcessId,
}

/// Daemon management commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonCommand {
    Start,
    Stop,
    Status,
}

/// All available commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    Start(StartOptions),
    Stop(StopOptions),
    Restart(RestartOptions),
    List,
    Logs(LogOptions),
    Delete(DeleteOptions),
    Daemon(DaemonCommand),
}

/// Process information returned in responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub id: ProcessId,
    pub name: String,
    pub state: ProcessState,
    pub stats: ProcessStats,
}

/// Response data variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseData {
    /// Process started successfully
    Started { id: ProcessId, name: String },
    /// Process stopped successfully
    Stopped { id: ProcessId },
    /// Process restarted successfully
    Restarted { id: ProcessId },
    /// List of all processes
    ProcessList(Vec<ProcessInfo>),
    /// Log lines
    Logs(Vec<String>),
    /// Process deleted successfully
    Deleted { id: ProcessId },
    /// Daemon status
    DaemonStatus { running: bool, uptime: Duration },
    /// Generic success message
    Success(String),
}

/// Request message from client to daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub command: Command,
}

/// Response message from daemon to client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    pub result: Result<ResponseData, String>,
}

impl Request {
    pub fn new(id: u64, command: Command) -> Self {
        Self { id, command }
    }
}

impl Response {
    pub fn success(id: u64, data: ResponseData) -> Self {
        Self {
            id,
            result: Ok(data),
        }
    }

    pub fn error(id: u64, error: String) -> Self {
        Self {
            id,
            result: Err(error),
        }
    }
}
