use crate::error::{AdasaError, Result};
use crate::ipc::protocol::ProcessId;
use crate::process::{ProcessManager, ProcessState};
use std::collections::HashSet;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Supervisor configuration
#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    /// How often to check process health (in seconds)
    pub check_interval_secs: u64,
    /// Whether supervisor is enabled
    pub enabled: bool,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            check_interval_secs: 5,
            enabled: true,
        }
    }
}

/// Process supervisor that monitors process health and handles automatic restarts
pub struct ProcessSupervisor {
    /// Configuration for the supervisor
    config: SupervisorConfig,
    /// Set of processes that are currently being restarted (to avoid duplicate restart attempts)
    restarting: HashSet<ProcessId>,
}

impl ProcessSupervisor {
    /// Create a new process supervisor
    pub fn new(config: SupervisorConfig) -> Self {
        Self {
            config,
            restarting: HashSet::new(),
        }
    }

    /// Create a supervisor with default configuration
    pub fn with_defaults() -> Self {
        Self::new(SupervisorConfig::default())
    }

    /// Run the supervisor loop
    ///
    /// This continuously monitors all processes and handles crashes
    ///
    /// # Arguments
    /// * `manager` - Process manager to supervise
    ///
    /// # Returns
    /// Never returns under normal operation
    pub async fn run(&mut self, manager: &mut ProcessManager) -> Result<()> {
        if !self.config.enabled {
            info!("Process supervisor is disabled");
            return Ok(());
        }

        info!(
            "Starting process supervisor (check interval: {}s)",
            self.config.check_interval_secs
        );

        let check_interval = Duration::from_secs(self.config.check_interval_secs);

        loop {
            // Perform health check
            if let Err(e) = self.check_health(manager).await {
                error!("Error during health check: {}", e);
            }

            // Wait before next check
            sleep(check_interval).await;
        }
    }

    /// Perform a single health check cycle
    ///
    /// This checks all processes for crashes and attempts restarts as needed
    ///
    /// # Arguments
    /// * `manager` - Process manager to check
    ///
    /// # Returns
    /// * `Ok(())` - Health check completed
    /// * `Err(AdasaError)` - Error during health check
    pub async fn check_health(&mut self, manager: &mut ProcessManager) -> Result<()> {
        debug!("Performing health check");

        // Detect crashed processes BEFORE updating stats
        // (update_stats will mark crashed processes as Errored, which prevents detect_crashes from finding them)
        let crashed_ids = manager.detect_crashes();

        if !crashed_ids.is_empty() {
            info!("Detected {} crashed process(es)", crashed_ids.len());
        }

        // Handle each crashed process
        for process_id in crashed_ids {
            if let Err(e) = self.handle_crash(manager, process_id).await {
                error!("Failed to handle crash for process {}: {}", process_id, e);
            }
        }

        // Update process statistics (for processes that are still running)
        manager.update_stats()?;

        // Clean up completed restarts
        self.cleanup_restarting(manager);

        Ok(())
    }

    /// Handle a crashed process
    ///
    /// This attempts to restart the process according to its restart policy
    ///
    /// # Arguments
    /// * `manager` - Process manager
    /// * `process_id` - ID of the crashed process
    ///
    /// # Returns
    /// * `Ok(())` - Crash handled successfully
    /// * `Err(AdasaError)` - Error handling crash
    async fn handle_crash(
        &mut self,
        manager: &mut ProcessManager,
        process_id: ProcessId,
    ) -> Result<()> {
        // Check if already restarting
        if self.restarting.contains(&process_id) {
            debug!("Process {} is already being restarted", process_id);
            return Ok(());
        }

        // Get process information
        let process = manager
            .get_status(process_id)
            .ok_or_else(|| AdasaError::ProcessNotFound(process_id.to_string()))?;

        let process_name = process.name.clone();
        let restart_count = process.restart_tracker.restart_count();

        info!(
            "Process '{}' (id: {}) crashed (restart count: {})",
            process_name, process_id, restart_count
        );

        // Check if restart should be attempted
        let (_, should_restart) = manager
            .get_restart_info(process_id)
            .ok_or_else(|| AdasaError::ProcessNotFound(process_id.to_string()))?;

        if !should_restart {
            warn!(
                "Process '{}' (id: {}) has exceeded restart limit, not restarting",
                process_name, process_id
            );
            return Err(AdasaError::RestartLimitExceeded(process_name));
        }

        // Mark as restarting
        self.restarting.insert(process_id);

        info!(
            "Attempting to restart process '{}' (id: {})",
            process_name, process_id
        );

        // Attempt restart (try_auto_restart handles backoff delay internally)
        match manager.try_auto_restart(process_id).await {
            Ok(true) => {
                info!(
                    "Successfully restarted process '{}' (id: {})",
                    process_name, process_id
                );
                Ok(())
            }
            Ok(false) => {
                warn!(
                    "Restart not attempted for process '{}' (id: {}) - policy prevented it",
                    process_name, process_id
                );
                self.restarting.remove(&process_id);
                Ok(())
            }
            Err(e) => {
                error!(
                    "Failed to restart process '{}' (id: {}): {}",
                    process_name, process_id, e
                );
                self.restarting.remove(&process_id);
                Err(e)
            }
        }
    }

    /// Clean up processes that are no longer in restarting state
    ///
    /// This removes process IDs from the restarting set if they are now running
    ///
    /// # Arguments
    /// * `manager` - Process manager
    fn cleanup_restarting(&mut self, manager: &ProcessManager) {
        let mut to_remove = Vec::new();

        for &process_id in &self.restarting {
            if let Some(process) = manager.get_status(process_id) {
                // If process is running, remove from restarting set
                if process.state == ProcessState::Running {
                    to_remove.push(process_id);
                }
            } else {
                // Process no longer exists, remove from restarting set
                to_remove.push(process_id);
            }
        }

        for process_id in to_remove {
            debug!("Removing process {} from restarting set", process_id);
            self.restarting.remove(&process_id);
        }
    }

    /// Check if a process is currently being restarted
    ///
    /// # Arguments
    /// * `process_id` - Process ID to check
    ///
    /// # Returns
    /// * `true` - Process is being restarted
    /// * `false` - Process is not being restarted
    pub fn is_restarting(&self, process_id: ProcessId) -> bool {
        self.restarting.contains(&process_id)
    }

    /// Get the number of processes currently being restarted
    pub fn restarting_count(&self) -> usize {
        self.restarting.len()
    }

    /// Manually trigger a health check (useful for testing)
    ///
    /// # Arguments
    /// * `manager` - Process manager to check
    ///
    /// # Returns
    /// * `Ok(())` - Health check completed
    /// * `Err(AdasaError)` - Error during health check
    pub async fn trigger_check(&mut self, manager: &mut ProcessManager) -> Result<()> {
        self.check_health(manager).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProcessConfig;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn create_test_config(name: &str, autorestart: bool, max_restarts: usize) -> ProcessConfig {
        ProcessConfig {
            name: name.to_string(),
            script: PathBuf::from("/bin/sh"),
            args: vec!["-c".to_string(), "exit 1".to_string()],
            cwd: None,
            env: HashMap::new(),
            instances: 1,
            autorestart,
            max_restarts,
            restart_delay_secs: 0, // No delay for faster tests
            max_memory: None,
            stop_signal: "SIGTERM".to_string(),
            stop_timeout_secs: 2,
        }
    }

    #[tokio::test]
    async fn test_supervisor_new() {
        let config = SupervisorConfig::default();
        let supervisor = ProcessSupervisor::new(config);

        assert_eq!(supervisor.restarting_count(), 0);
    }

    #[tokio::test]
    async fn test_supervisor_with_defaults() {
        let supervisor = ProcessSupervisor::with_defaults();
        assert_eq!(supervisor.restarting_count(), 0);
    }

    #[tokio::test]
    async fn test_supervisor_detect_and_restart() {
        let mut manager = ProcessManager::new();
        let mut supervisor = ProcessSupervisor::with_defaults();

        // Spawn a process that will run (sleep) so it doesn't immediately crash again
        let config = ProcessConfig {
            name: "crash-test".to_string(),
            script: PathBuf::from("/bin/sleep"),
            args: vec!["10".to_string()],
            cwd: None,
            env: HashMap::new(),
            instances: 1,
            autorestart: true,
            max_restarts: 10,
            restart_delay_secs: 0,
            max_memory: None,
            stop_signal: "SIGTERM".to_string(),
            stop_timeout_secs: 2,
        };
        let id = manager.spawn(config).await.unwrap();

        // Verify initial state
        assert_eq!(manager.get_status(id).unwrap().state, ProcessState::Running);
        assert_eq!(manager.get_status(id).unwrap().stats.restarts, 0);

        // Kill the process to simulate a crash
        let process = manager.get_mut(id).unwrap();
        let _ = process.child.kill().await;
        let _ = process.child.wait().await;

        // Wait for system to update
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Trigger health check (this will detect the crash and restart)
        let result = supervisor.trigger_check(&mut manager).await;
        assert!(result.is_ok());

        // Process should have been restarted
        let process = manager.get_status(id).unwrap();
        assert_eq!(
            process.state,
            ProcessState::Running,
            "Process should be running after restart"
        );
        assert_eq!(process.stats.restarts, 1, "Restart count should be 1");

        // Cleanup
        let _ = manager.stop(id, true).await;
    }

    #[tokio::test]
    async fn test_supervisor_respects_restart_limit() {
        let mut manager = ProcessManager::new();
        let mut supervisor = ProcessSupervisor::with_defaults();

        // Spawn a process with low restart limit
        let config = ProcessConfig {
            name: "limited-restart".to_string(),
            script: PathBuf::from("/bin/sleep"),
            args: vec!["10".to_string()],
            cwd: None,
            env: HashMap::new(),
            instances: 1,
            autorestart: true,
            max_restarts: 2,
            restart_delay_secs: 0,
            max_memory: None,
            stop_signal: "SIGTERM".to_string(),
            stop_timeout_secs: 2,
        };
        let id = manager.spawn(config).await.unwrap();

        // Simulate multiple crashes by killing and restarting
        for i in 0..3 {
            // Kill the process to simulate a crash
            if let Some(process) = manager.get_mut(id) {
                let _ = process.child.kill().await;
                let _ = process.child.wait().await;
            }

            // Wait for system to update
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Trigger health check
            let result = supervisor.trigger_check(&mut manager).await;

            if i < 2 {
                // First two restarts should succeed
                assert!(result.is_ok());
                let process = manager.get_status(id).unwrap();
                assert_eq!(process.state, ProcessState::Running);
            } else {
                // Third restart should not be attempted due to limit
                // The health check itself succeeds, but the restart is not attempted
                assert!(result.is_ok());
                let process = manager.get_status(id).unwrap();
                assert_eq!(process.state, ProcessState::Errored);
            }
        }

        // After exceeding limit, process should remain in errored state
        let process = manager.get_status(id).unwrap();
        assert_eq!(process.state, ProcessState::Errored);
    }

    #[tokio::test]
    async fn test_supervisor_disabled_autorestart() {
        let mut manager = ProcessManager::new();
        let mut supervisor = ProcessSupervisor::with_defaults();

        // Spawn a process with autorestart disabled
        let config = create_test_config("no-restart", false, 10);
        let id = manager.spawn(config).await.unwrap();

        // Get the process and wait for it to exit
        let process = manager.get_mut(id).unwrap();
        let _ = process.child.wait().await;

        // Wait for system to update
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Trigger health check
        let _ = supervisor.trigger_check(&mut manager).await;

        // Process should remain in errored state (not restarted)
        let process = manager.get_status(id).unwrap();
        assert_eq!(process.state, ProcessState::Errored);
        assert_eq!(process.stats.restarts, 0);
    }

    #[tokio::test]
    async fn test_supervisor_is_restarting() {
        let mut manager = ProcessManager::new();
        let mut supervisor = ProcessSupervisor::with_defaults();

        let config = create_test_config("restart-check", true, 10);
        let id = manager.spawn(config).await.unwrap();

        // Initially not restarting
        assert!(!supervisor.is_restarting(id));

        // Get the process and wait for it to exit
        let process = manager.get_mut(id).unwrap();
        let _ = process.child.wait().await;

        // Wait for system to update
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Trigger health check (this will mark as restarting)
        let _ = supervisor.trigger_check(&mut manager).await;

        // After restart completes, should no longer be in restarting set
        assert!(!supervisor.is_restarting(id));
    }

    #[tokio::test]
    async fn test_supervisor_cleanup_restarting() {
        let supervisor = ProcessSupervisor::with_defaults();
        let _manager = ProcessManager::new();

        // Initially no processes restarting
        assert_eq!(supervisor.restarting_count(), 0);
    }
}
