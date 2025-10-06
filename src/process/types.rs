use crate::config::ProcessConfig;
use crate::ipc::protocol::ProcessId;
use crate::process::restart::{RestartPolicy, RestartTracker};
use crate::process::spawner::SpawnedProcess;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use tokio::process::Child;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessState {
    Starting,
    Running,
    Stopping,
    Stopped,
    Errored,
}

impl std::fmt::Display for ProcessState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessState::Starting => write!(f, "starting"),
            ProcessState::Running => write!(f, "running"),
            ProcessState::Stopping => write!(f, "stopping"),
            ProcessState::Stopped => write!(f, "stopped"),
            ProcessState::Errored => write!(f, "errored"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStats {
    pub pid: u32,
    pub started_at: SystemTime,
    pub restarts: usize,
    pub cpu_usage: f32,
    pub memory_usage: u64,
    pub last_restart: Option<SystemTime>,
    pub memory_violations: usize,
    pub cpu_violations: usize,
}

impl ProcessStats {
    pub fn new(pid: u32) -> Self {
        Self {
            pid,
            started_at: SystemTime::now(),
            restarts: 0,
            cpu_usage: 0.0,
            memory_usage: 0,
            last_restart: None,
            memory_violations: 0,
            cpu_violations: 0,
        }
    }

    pub fn uptime(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.started_at)
            .unwrap_or(Duration::from_secs(0))
    }

    pub fn record_restart(&mut self, new_pid: u32) {
        self.restarts += 1;
        self.last_restart = Some(SystemTime::now());
        self.started_at = SystemTime::now();
        self.pid = new_pid;
        self.cpu_usage = 0.0;
        self.memory_usage = 0;
    }

    pub fn record_memory_violation(&mut self) {
        self.memory_violations += 1;
    }

    pub fn record_cpu_violation(&mut self) {
        self.cpu_violations += 1;
    }
}

#[derive(Debug)]
pub struct ManagedProcess {
    pub id: ProcessId,
    pub name: String,
    pub config: ProcessConfig,
    pub state: ProcessState,
    pub child: Child,
    pub stats: ProcessStats,
    pub restart_policy: RestartPolicy,
    pub restart_tracker: RestartTracker,
    pub cgroup_manager: Option<crate::process::limits::cgroup::CGroupManager>,
}

impl ManagedProcess {
    pub fn new(
        id: ProcessId,
        name: String,
        config: ProcessConfig,
        spawned: SpawnedProcess,
    ) -> Self {
        let restart_policy = RestartPolicy::from_config(
            config.autorestart,
            config.max_restarts,
            config.restart_delay_secs,
        );

        let cgroup_manager = if config.max_cpu.is_some() {
            Some(crate::process::limits::cgroup::CGroupManager::new(
                name.clone(),
            ))
        } else {
            None
        };

        Self {
            id,
            name,
            config,
            state: ProcessState::Starting,
            child: spawned.child,
            stats: ProcessStats::new(spawned.pid),
            restart_policy,
            restart_tracker: RestartTracker::new(),
            cgroup_manager,
        }
    }

    pub fn mark_running(&mut self) {
        self.state = ProcessState::Running;
    }

    pub fn mark_stopping(&mut self) {
        self.state = ProcessState::Stopping;
    }

    pub fn mark_stopped(&mut self) {
        self.state = ProcessState::Stopped;
    }

    pub(crate) fn mark_errored(&mut self) {
        self.state = ProcessState::Errored;
    }
}
