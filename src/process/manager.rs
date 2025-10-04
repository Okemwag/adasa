use crate::config::ProcessConfig;
use crate::error::{AdasaError, Result};
use crate::process::spawner::{spawn_process, SpawnedProcess};
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use tokio::process::Child;

/// Unique identifier for a managed process
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProcessId(u64);

impl ProcessId {
    /// Create a new ProcessId
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get the inner u64 value
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
    /// Process is being started
    Starting,
    /// Process is running normally
    Running,
    /// Process is being stopped
    Stopping,
    /// Process has been stopped
    Stopped,
    /// Process encountered an error
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

/// Statistics for a managed process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStats {
    /// Operating system process ID
    pub pid: u32,
    /// Time when the process was started
    pub started_at: SystemTime,
    /// Number of times the process has been restarted
    pub restarts: usize,
    /// CPU usage percentage (0.0 - 100.0)
    pub cpu_usage: f32,
    /// Memory usage in bytes
    pub memory_usage: u64,
    /// Time of last restart (if any)
    pub last_restart: Option<SystemTime>,
}

impl ProcessStats {
    /// Create new process statistics
    pub fn new(pid: u32) -> Self {
        Self {
            pid,
            started_at: SystemTime::now(),
            restarts: 0,
            cpu_usage: 0.0,
            memory_usage: 0,
            last_restart: None,
        }
    }

    /// Calculate uptime duration
    pub fn uptime(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.started_at)
            .unwrap_or(Duration::from_secs(0))
    }
}

/// A managed process with all its metadata
#[derive(Debug)]
pub struct ManagedProcess {
    /// Unique identifier for this process
    pub id: ProcessId,
    /// Process name
    pub name: String,
    /// Process configuration
    pub config: ProcessConfig,
    /// Current state
    pub state: ProcessState,
    /// Child process handle
    pub child: Child,
    /// Process statistics
    pub stats: ProcessStats,
}

impl ManagedProcess {
    /// Create a new managed process
    fn new(id: ProcessId, name: String, config: ProcessConfig, spawned: SpawnedProcess) -> Self {
        Self {
            id,
            name,
            config,
            state: ProcessState::Starting,
            child: spawned.child,
            stats: ProcessStats::new(spawned.pid),
        }
    }

    /// Transition to running state
    fn mark_running(&mut self) {
        self.state = ProcessState::Running;
    }

    /// Transition to stopping state
    fn mark_stopping(&mut self) {
        self.state = ProcessState::Stopping;
    }

    /// Transition to stopped state
    fn mark_stopped(&mut self) {
        self.state = ProcessState::Stopped;
    }

    /// Transition to errored state
    fn mark_errored(&mut self) {
        self.state = ProcessState::Errored;
    }
}

/// Process manager that handles lifecycle of all managed processes
pub struct ProcessManager {
    /// Map of process ID to managed process
    processes: HashMap<ProcessId, ManagedProcess>,
    /// Counter for generating unique process IDs
    next_id: u64,
}

impl ProcessManager {
    /// Create a new process manager
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
            next_id: 1,
        }
    }

    /// Spawn a new process with the given configuration
    ///
    /// # Arguments
    /// * `config` - Process configuration
    ///
    /// # Returns
    /// * `Ok(ProcessId)` - ID of the spawned process
    /// * `Err(AdasaError)` - Failed to spawn process
    pub async fn spawn(&mut self, config: ProcessConfig) -> Result<ProcessId> {
        // Check if a process with this name already exists
        if self.processes.values().any(|p| p.name == config.name) {
            return Err(AdasaError::ProcessAlreadyExists(config.name.clone()));
        }

        // Validate configuration
        config.validate()?;

        // Spawn the process
        let spawned = spawn_process(&config).await?;
        let name = spawned.name.clone();

        // Generate unique ID
        let id = ProcessId::new(self.next_id);
        self.next_id += 1;

        // Create managed process
        let mut managed = ManagedProcess::new(id, name, config, spawned);

        // Mark as running (process successfully started)
        managed.mark_running();

        // Store in map
        self.processes.insert(id, managed);

        Ok(id)
    }

    /// Stop a process by sending SIGTERM, then SIGKILL if necessary
    ///
    /// # Arguments
    /// * `id` - Process ID to stop
    /// * `force` - If true, send SIGKILL immediately
    ///
    /// # Returns
    /// * `Ok(())` - Process stopped successfully
    /// * `Err(AdasaError)` - Failed to stop process
    pub async fn stop(&mut self, id: ProcessId, force: bool) -> Result<()> {
        let process = self
            .processes
            .get_mut(&id)
            .ok_or_else(|| AdasaError::ProcessNotFound(id.to_string()))?;

        // Mark as stopping
        process.mark_stopping();

        let pid = process.stats.pid;
        let nix_pid = Pid::from_raw(pid as i32);

        if force {
            // Send SIGKILL immediately
            signal::kill(nix_pid, Signal::SIGKILL).map_err(|e| {
                AdasaError::StopError(
                    process.name.clone(),
                    format!("Failed to send SIGKILL: {}", e),
                )
            })?;
        } else {
            // Send SIGTERM for graceful shutdown
            signal::kill(nix_pid, Signal::SIGTERM).map_err(|e| {
                AdasaError::StopError(
                    process.name.clone(),
                    format!("Failed to send SIGTERM: {}", e),
                )
            })?;

            // Wait for the configured timeout
            let timeout = process.config.stop_timeout();
            let wait_result = tokio::time::timeout(timeout, process.child.wait()).await;

            match wait_result {
                Ok(Ok(_)) => {
                    // Process exited gracefully
                }
                Ok(Err(e)) => {
                    return Err(AdasaError::StopError(
                        process.name.clone(),
                        format!("Wait failed: {}", e),
                    ));
                }
                Err(_) => {
                    // Timeout - send SIGKILL
                    signal::kill(nix_pid, Signal::SIGKILL).map_err(|e| {
                        AdasaError::StopError(
                            process.name.clone(),
                            format!("Failed to send SIGKILL after timeout: {}", e),
                        )
                    })?;
                }
            }
        }

        // Wait for process to actually exit
        let _ = process.child.wait().await;

        // Mark as stopped
        process.mark_stopped();

        Ok(())
    }

    /// Get the status of a specific process
    ///
    /// # Arguments
    /// * `id` - Process ID
    ///
    /// # Returns
    /// * `Some(&ManagedProcess)` - Process information
    /// * `None` - Process not found
    pub fn get_status(&self, id: ProcessId) -> Option<&ManagedProcess> {
        self.processes.get(&id)
    }

    /// List all managed processes
    ///
    /// # Returns
    /// Vector of references to all managed processes
    pub fn list(&self) -> Vec<&ManagedProcess> {
        self.processes.values().collect()
    }

    /// Get a mutable reference to a process
    pub fn get_mut(&mut self, id: ProcessId) -> Option<&mut ManagedProcess> {
        self.processes.get_mut(&id)
    }

    /// Remove a process from management (after it has stopped)
    pub fn remove(&mut self, id: ProcessId) -> Result<()> {
        self.processes
            .remove(&id)
            .ok_or_else(|| AdasaError::ProcessNotFound(id.to_string()))?;
        Ok(())
    }

    /// Find a process by name
    pub fn find_by_name(&self, name: &str) -> Option<&ManagedProcess> {
        self.processes.values().find(|p| p.name == name)
    }

    /// Find all processes with a given name (for multi-instance support)
    pub fn find_all_by_name(&self, name: &str) -> Vec<&ManagedProcess> {
        self.processes
            .values()
            .filter(|p| p.name == name || p.name.starts_with(&format!("{}-", name)))
            .collect()
    }
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn create_test_config(name: &str) -> ProcessConfig {
        ProcessConfig {
            name: name.to_string(),
            script: PathBuf::from("/bin/sleep"),
            args: vec!["10".to_string()],
            cwd: None,
            env: HashMap::new(),
            instances: 1,
            autorestart: true,
            max_restarts: 10,
            restart_delay_secs: 1,
            max_memory: None,
            stop_signal: "SIGTERM".to_string(),
            stop_timeout_secs: 2,
        }
    }

    #[tokio::test]
    async fn test_process_manager_new() {
        let manager = ProcessManager::new();
        assert_eq!(manager.list().len(), 0);
        assert_eq!(manager.next_id, 1);
    }

    #[tokio::test]
    async fn test_spawn_process() {
        let mut manager = ProcessManager::new();
        let config = create_test_config("test-process");

        let result = manager.spawn(config).await;
        assert!(result.is_ok());

        let id = result.unwrap();
        assert_eq!(id.as_u64(), 1);
        assert_eq!(manager.list().len(), 1);

        let process = manager.get_status(id).unwrap();
        assert_eq!(process.name, "test-process");
        assert_eq!(process.state, ProcessState::Running);

        // Cleanup
        let _ = manager.stop(id, true).await;
    }

    #[tokio::test]
    async fn test_spawn_duplicate_name() {
        let mut manager = ProcessManager::new();
        let config1 = create_test_config("duplicate");
        let config2 = create_test_config("duplicate");

        let result1 = manager.spawn(config1).await;
        assert!(result1.is_ok());

        let result2 = manager.spawn(config2).await;
        assert!(result2.is_err());
        assert!(matches!(result2, Err(AdasaError::ProcessAlreadyExists(_))));

        // Cleanup
        let _ = manager.stop(result1.unwrap(), true).await;
    }

    #[tokio::test]
    async fn test_stop_process_graceful() {
        let mut manager = ProcessManager::new();
        let config = create_test_config("test-stop");

        let id = manager.spawn(config).await.unwrap();

        // Verify process is running
        let process = manager.get_status(id).unwrap();
        assert_eq!(process.state, ProcessState::Running);

        // Stop gracefully
        let result = manager.stop(id, false).await;
        assert!(result.is_ok());

        // Verify process is stopped
        let process = manager.get_status(id).unwrap();
        assert_eq!(process.state, ProcessState::Stopped);
    }

    #[tokio::test]
    async fn test_stop_process_force() {
        let mut manager = ProcessManager::new();
        let config = create_test_config("test-force-stop");

        let id = manager.spawn(config).await.unwrap();

        // Force stop
        let result = manager.stop(id, true).await;
        assert!(result.is_ok());

        // Verify process is stopped
        let process = manager.get_status(id).unwrap();
        assert_eq!(process.state, ProcessState::Stopped);
    }

    #[tokio::test]
    async fn test_stop_nonexistent_process() {
        let mut manager = ProcessManager::new();
        let id = ProcessId::new(999);

        let result = manager.stop(id, false).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(AdasaError::ProcessNotFound(_))));
    }

    #[tokio::test]
    async fn test_get_status() {
        let mut manager = ProcessManager::new();
        let config = create_test_config("test-status");

        let id = manager.spawn(config).await.unwrap();

        let status = manager.get_status(id);
        assert!(status.is_some());

        let process = status.unwrap();
        assert_eq!(process.name, "test-status");
        assert_eq!(process.state, ProcessState::Running);
        assert!(process.stats.pid > 0);

        // Cleanup
        let _ = manager.stop(id, true).await;
    }

    #[tokio::test]
    async fn test_get_status_nonexistent() {
        let manager = ProcessManager::new();
        let id = ProcessId::new(999);

        let status = manager.get_status(id);
        assert!(status.is_none());
    }

    #[tokio::test]
    async fn test_list_processes() {
        let mut manager = ProcessManager::new();

        // Initially empty
        assert_eq!(manager.list().len(), 0);

        // Spawn multiple processes
        let config1 = create_test_config("process-1");
        let config2 = create_test_config("process-2");
        let config3 = create_test_config("process-3");

        let id1 = manager.spawn(config1).await.unwrap();
        let id2 = manager.spawn(config2).await.unwrap();
        let id3 = manager.spawn(config3).await.unwrap();

        // List should contain all processes
        let list = manager.list();
        assert_eq!(list.len(), 3);

        let names: Vec<&str> = list.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"process-1"));
        assert!(names.contains(&"process-2"));
        assert!(names.contains(&"process-3"));

        // Cleanup
        let _ = manager.stop(id1, true).await;
        let _ = manager.stop(id2, true).await;
        let _ = manager.stop(id3, true).await;
    }

    #[tokio::test]
    async fn test_process_state_transitions() {
        let mut manager = ProcessManager::new();
        let config = create_test_config("test-states");

        let id = manager.spawn(config).await.unwrap();

        // Should start in Running state (after successful spawn)
        let process = manager.get_status(id).unwrap();
        assert_eq!(process.state, ProcessState::Running);

        // Stop should transition to Stopped
        manager.stop(id, true).await.unwrap();
        let process = manager.get_status(id).unwrap();
        assert_eq!(process.state, ProcessState::Stopped);
    }

    #[tokio::test]
    async fn test_process_stats() {
        let mut manager = ProcessManager::new();
        let config = create_test_config("test-stats");

        let id = manager.spawn(config).await.unwrap();

        let process = manager.get_status(id).unwrap();
        let stats = &process.stats;

        // Verify stats are initialized
        assert!(stats.pid > 0);
        assert_eq!(stats.restarts, 0);
        assert!(stats.last_restart.is_none());

        // Verify uptime is reasonable
        let uptime = stats.uptime();
        assert!(uptime.as_secs() < 5); // Should be very recent

        // Cleanup
        let _ = manager.stop(id, true).await;
    }

    #[tokio::test]
    async fn test_find_by_name() {
        let mut manager = ProcessManager::new();
        let config = create_test_config("findable");

        let id = manager.spawn(config).await.unwrap();

        let found = manager.find_by_name("findable");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, id);

        let not_found = manager.find_by_name("nonexistent");
        assert!(not_found.is_none());

        // Cleanup
        let _ = manager.stop(id, true).await;
    }

    #[tokio::test]
    async fn test_remove_process() {
        let mut manager = ProcessManager::new();
        let config = create_test_config("removable");

        let id = manager.spawn(config).await.unwrap();
        assert_eq!(manager.list().len(), 1);

        // Stop first
        manager.stop(id, true).await.unwrap();

        // Then remove
        let result = manager.remove(id);
        assert!(result.is_ok());
        assert_eq!(manager.list().len(), 0);
    }

    #[tokio::test]
    async fn test_process_id_display() {
        let id = ProcessId::new(42);
        assert_eq!(format!("{}", id), "42");
        assert_eq!(id.as_u64(), 42);
    }

    #[tokio::test]
    async fn test_process_state_display() {
        assert_eq!(format!("{}", ProcessState::Starting), "starting");
        assert_eq!(format!("{}", ProcessState::Running), "running");
        assert_eq!(format!("{}", ProcessState::Stopping), "stopping");
        assert_eq!(format!("{}", ProcessState::Stopped), "stopped");
        assert_eq!(format!("{}", ProcessState::Errored), "errored");
    }
}
