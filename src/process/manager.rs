use crate::config::ProcessConfig;
use crate::error::{AdasaError, Result};
use crate::process::monitor::ProcessMonitor;
use crate::process::restart::{RestartPolicy, RestartTracker};
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

    /// Record a restart
    pub fn record_restart(&mut self, new_pid: u32) {
        self.restarts += 1;
        self.last_restart = Some(SystemTime::now());
        self.started_at = SystemTime::now();
        self.pid = new_pid;
        self.cpu_usage = 0.0;
        self.memory_usage = 0;
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
    /// Restart policy for this process
    pub restart_policy: RestartPolicy,
    /// Restart tracker for this process
    pub restart_tracker: RestartTracker,
}

impl ManagedProcess {
    /// Create a new managed process
    fn new(id: ProcessId, name: String, config: ProcessConfig, spawned: SpawnedProcess) -> Self {
        let restart_policy = RestartPolicy::from_config(
            config.autorestart,
            config.max_restarts,
            config.restart_delay_secs,
        );

        Self {
            id,
            name,
            config,
            state: ProcessState::Starting,
            child: spawned.child,
            stats: ProcessStats::new(spawned.pid),
            restart_policy,
            restart_tracker: RestartTracker::new(),
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
    pub(crate) fn mark_errored(&mut self) {
        self.state = ProcessState::Errored;
    }
}

/// Process manager that handles lifecycle of all managed processes
pub struct ProcessManager {
    /// Map of process ID to managed process
    processes: HashMap<ProcessId, ManagedProcess>,
    /// Counter for generating unique process IDs
    next_id: u64,
    /// Process monitor for resource tracking
    monitor: ProcessMonitor,
}

impl ProcessManager {
    /// Create a new process manager
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
            next_id: 1,
            monitor: ProcessMonitor::new(),
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
        let process = self
            .processes
            .remove(&id)
            .ok_or_else(|| AdasaError::ProcessNotFound(id.to_string()))?;

        // Clear monitor cache for this process
        self.monitor.clear_cache(process.stats.pid);

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

    /// Update statistics for all running processes
    ///
    /// This should be called periodically to keep process stats up to date
    ///
    /// # Returns
    /// * `Ok(())` - Statistics updated successfully
    pub fn update_stats(&mut self) -> Result<()> {
        self.monitor.update_all_stats(self.processes.values_mut())
    }

    /// Detect crashed processes and return their IDs
    ///
    /// This checks if processes are still alive and marks crashed ones as errored
    ///
    /// # Returns
    /// Vector of process IDs that have crashed
    pub fn detect_crashes(&mut self) -> Vec<ProcessId> {
        let crashed_pids = self.monitor.detect_crashes(self.processes.values_mut());

        // Convert PIDs to ProcessIds
        crashed_pids
            .into_iter()
            .filter_map(|pid| {
                self.processes
                    .iter()
                    .find(|(_, p)| p.stats.pid == pid)
                    .map(|(id, _)| *id)
            })
            .collect()
    }

    /// Check if a specific process is still alive
    ///
    /// # Arguments
    /// * `id` - Process ID to check
    ///
    /// # Returns
    /// * `true` - Process is alive
    /// * `false` - Process has crashed or doesn't exist
    pub fn is_alive(&mut self, id: ProcessId) -> bool {
        if let Some(process) = self.processes.get(&id) {
            self.monitor.is_process_alive(process.stats.pid)
        } else {
            false
        }
    }

    /// Restart a process (stop and start with same configuration)
    ///
    /// # Arguments
    /// * `id` - Process ID to restart
    ///
    /// # Returns
    /// * `Ok(())` - Process restarted successfully
    /// * `Err(AdasaError)` - Failed to restart process
    pub async fn restart(&mut self, id: ProcessId) -> Result<()> {
        let process = self
            .processes
            .get(&id)
            .ok_or_else(|| AdasaError::ProcessNotFound(id.to_string()))?;

        // Clone the configuration before stopping
        let config = process.config.clone();

        // Stop the process (gracefully)
        self.stop(id, false).await?;

        // Spawn a new process with the same configuration
        let spawned = spawn_process(&config).await?;
        let new_pid = spawned.pid;

        // Get the process again and update it
        let process = self
            .processes
            .get_mut(&id)
            .ok_or_else(|| AdasaError::ProcessNotFound(id.to_string()))?;

        // Update process with new child and stats
        process.child = spawned.child;
        process.stats.record_restart(new_pid);
        process.restart_tracker.record_restart();
        process.state = ProcessState::Running;

        Ok(())
    }

    /// Attempt to restart a crashed process if policy allows
    ///
    /// # Arguments
    /// * `id` - Process ID to restart
    ///
    /// # Returns
    /// * `Ok(true)` - Process was restarted
    /// * `Ok(false)` - Restart was not attempted (policy prevented it)
    /// * `Err(AdasaError)` - Failed to restart process
    pub async fn try_auto_restart(&mut self, id: ProcessId) -> Result<bool> {
        let process = self
            .processes
            .get(&id)
            .ok_or_else(|| AdasaError::ProcessNotFound(id.to_string()))?;

        // Check if restart should be attempted
        if !process
            .restart_policy
            .should_restart(&process.restart_tracker)
        {
            return Ok(false);
        }

        // Calculate delay before restart
        let delay = process
            .restart_policy
            .calculate_delay(&process.restart_tracker);

        // Wait for the backoff delay
        tokio::time::sleep(delay).await;

        // For crashed processes, we don't need to stop them first
        // Just spawn a new process with the same configuration
        let config = process.config.clone();

        // Spawn a new process
        let spawned = spawn_process(&config).await?;
        let new_pid = spawned.pid;

        // Get the process again and update it
        let process = self
            .processes
            .get_mut(&id)
            .ok_or_else(|| AdasaError::ProcessNotFound(id.to_string()))?;

        // Update process with new child and stats
        process.child = spawned.child;
        process.stats.record_restart(new_pid);
        process.restart_tracker.record_restart();
        process.state = ProcessState::Running;

        Ok(true)
    }

    /// Get restart information for a process
    ///
    /// # Arguments
    /// * `id` - Process ID
    ///
    /// # Returns
    /// * `Some((restart_count, should_restart))` - Restart info
    /// * `None` - Process not found
    pub fn get_restart_info(&self, id: ProcessId) -> Option<(usize, bool)> {
        self.processes.get(&id).map(|p| {
            let count = p.restart_tracker.restart_count();
            let should_restart = p.restart_policy.should_restart(&p.restart_tracker);
            (count, should_restart)
        })
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

    #[tokio::test]
    async fn test_update_stats() {
        let mut manager = ProcessManager::new();
        let config = create_test_config("test-update-stats");

        let id = manager.spawn(config).await.unwrap();

        // Give process time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Update stats
        let result = manager.update_stats();
        assert!(result.is_ok());

        // Check that stats were updated
        let process = manager.get_status(id).unwrap();
        // Memory should be > 0 for a running process
        assert!(process.stats.memory_usage > 0);

        // Cleanup
        let _ = manager.stop(id, true).await;
    }

    #[tokio::test]
    async fn test_detect_crashes() {
        let mut manager = ProcessManager::new();

        // Spawn a process that will exit immediately
        let config = ProcessConfig {
            name: "crash-test".to_string(),
            script: PathBuf::from("/bin/sh"),
            args: vec!["-c".to_string(), "exit 1".to_string()],
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

        let id = manager.spawn(config).await.unwrap();

        // Get the process and wait for it to exit
        let process = manager.get_mut(id).unwrap();
        let _ = process.child.wait().await;

        // Wait for system to update
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Detect crashes
        let crashed = manager.detect_crashes();

        // Process should be detected as crashed
        assert_eq!(
            crashed.len(),
            1,
            "Expected 1 crashed process, found {}",
            crashed.len()
        );
        assert_eq!(crashed[0], id);

        // Process state should be errored
        let process = manager.get_status(id).unwrap();
        assert_eq!(process.state, ProcessState::Errored);
    }

    #[tokio::test]
    async fn test_is_alive() {
        let mut manager = ProcessManager::new();
        let config = create_test_config("test-alive");

        let id = manager.spawn(config).await.unwrap();

        // Process should be alive
        assert!(manager.is_alive(id));

        // Stop the process
        manager.stop(id, true).await.unwrap();

        // Give system time to update
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Process should not be alive
        assert!(!manager.is_alive(id));
    }

    #[tokio::test]
    async fn test_is_alive_nonexistent() {
        let mut manager = ProcessManager::new();
        let id = ProcessId::new(999);

        // Nonexistent process should not be alive
        assert!(!manager.is_alive(id));
    }

    #[tokio::test]
    async fn test_restart_process() {
        let mut manager = ProcessManager::new();
        let config = create_test_config("test-restart");

        let id = manager.spawn(config).await.unwrap();

        // Get initial PID
        let initial_pid = manager.get_status(id).unwrap().stats.pid;
        let initial_restarts = manager.get_status(id).unwrap().stats.restarts;

        // Restart the process
        let result = manager.restart(id).await;
        assert!(result.is_ok());

        // Verify process was restarted
        let process = manager.get_status(id).unwrap();
        assert_eq!(process.state, ProcessState::Running);
        assert_ne!(process.stats.pid, initial_pid); // New PID
        assert_eq!(process.stats.restarts, initial_restarts + 1);
        assert!(process.stats.last_restart.is_some());

        // Cleanup
        let _ = manager.stop(id, true).await;
    }

    #[tokio::test]
    async fn test_restart_nonexistent() {
        let mut manager = ProcessManager::new();
        let id = ProcessId::new(999);

        let result = manager.restart(id).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(AdasaError::ProcessNotFound(_))));
    }

    #[tokio::test]
    async fn test_auto_restart_allowed() {
        let mut manager = ProcessManager::new();
        let config = create_test_config("test-auto-restart");

        let id = manager.spawn(config).await.unwrap();

        // Get initial PID
        let initial_pid = manager.get_status(id).unwrap().stats.pid;

        // Simulate a crash by marking as errored
        manager.get_mut(id).unwrap().mark_errored();

        // Try auto restart
        let result = manager.try_auto_restart(id).await;
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should have restarted

        // Verify process was restarted
        let process = manager.get_status(id).unwrap();
        assert_eq!(process.state, ProcessState::Running);
        assert_ne!(process.stats.pid, initial_pid);
        assert_eq!(process.stats.restarts, 1);

        // Cleanup
        let _ = manager.stop(id, true).await;
    }

    #[tokio::test]
    async fn test_auto_restart_max_limit() {
        let mut manager = ProcessManager::new();

        // Create config with low max_restarts
        let config = ProcessConfig {
            name: "test-max-restart".to_string(),
            script: PathBuf::from("/bin/sleep"),
            args: vec!["10".to_string()],
            cwd: None,
            env: HashMap::new(),
            instances: 1,
            autorestart: true,
            max_restarts: 2,       // Only allow 2 restarts
            restart_delay_secs: 0, // No delay for faster test
            max_memory: None,
            stop_signal: "SIGTERM".to_string(),
            stop_timeout_secs: 2,
        };

        let id = manager.spawn(config).await.unwrap();

        // First restart should succeed
        let result1 = manager.try_auto_restart(id).await;
        assert!(result1.is_ok());
        assert!(result1.unwrap());

        // Second restart should succeed
        let result2 = manager.try_auto_restart(id).await;
        assert!(result2.is_ok());
        assert!(result2.unwrap());

        // Third restart should be blocked
        let result3 = manager.try_auto_restart(id).await;
        assert!(result3.is_ok());
        assert!(!result3.unwrap()); // Should NOT have restarted

        // Cleanup
        let _ = manager.stop(id, true).await;
    }

    #[tokio::test]
    async fn test_get_restart_info() {
        let mut manager = ProcessManager::new();
        let config = create_test_config("test-restart-info");

        let id = manager.spawn(config).await.unwrap();

        // Initial state
        let (count, should_restart) = manager.get_restart_info(id).unwrap();
        assert_eq!(count, 0);
        assert!(should_restart);

        // After one restart
        manager.restart(id).await.unwrap();
        let (count, should_restart) = manager.get_restart_info(id).unwrap();
        assert_eq!(count, 1);
        assert!(should_restart);

        // Cleanup
        let _ = manager.stop(id, true).await;
    }

    #[tokio::test]
    async fn test_restart_preserves_config() {
        let mut manager = ProcessManager::new();

        let config = ProcessConfig {
            name: "test-preserve".to_string(),
            script: PathBuf::from("/bin/sleep"),
            args: vec!["10".to_string()],
            cwd: Some(PathBuf::from("/tmp")),
            env: {
                let mut env = HashMap::new();
                env.insert("TEST_VAR".to_string(), "test_value".to_string());
                env
            },
            instances: 1,
            autorestart: true,
            max_restarts: 10,
            restart_delay_secs: 1,
            max_memory: None,
            stop_signal: "SIGTERM".to_string(),
            stop_timeout_secs: 2,
        };

        let id = manager.spawn(config.clone()).await.unwrap();

        // Restart the process
        manager.restart(id).await.unwrap();

        // Verify configuration is preserved
        let process = manager.get_status(id).unwrap();
        assert_eq!(process.config.name, config.name);
        assert_eq!(process.config.script, config.script);
        assert_eq!(process.config.args, config.args);
        assert_eq!(process.config.cwd, config.cwd);
        assert_eq!(process.config.env, config.env);

        // Cleanup
        let _ = manager.stop(id, true).await;
    }
}
