use crate::error::Result;
use crate::process::{ManagedProcess, ProcessState};
use std::collections::HashMap;
use sysinfo::{Pid, ProcessRefreshKind, System};

/// Process monitor for collecting resource usage statistics
pub struct ProcessMonitor {
    /// System information collector
    system: System,
    /// Cache of previous CPU measurements for accurate calculation
    cpu_cache: HashMap<u32, f32>,
}

impl ProcessMonitor {
    /// Create a new process monitor
    pub fn new() -> Self {
        Self {
            system: System::new_all(),
            cpu_cache: HashMap::new(),
        }
    }

    /// Update statistics for a single managed process
    ///
    /// # Arguments
    /// * `process` - The managed process to update
    ///
    /// # Returns
    /// * `Ok(())` - Statistics updated successfully
    /// * `Err(AdasaError)` - Failed to update statistics
    pub fn update_process_stats(&mut self, process: &mut ManagedProcess) -> Result<()> {
        let pid = process.stats.pid;
        let sys_pid = Pid::from_u32(pid);

        // Refresh specific process information
        self.system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::Some(&[sys_pid]),
            true,
            ProcessRefreshKind::everything(),
        );

        // Get process information from sysinfo
        if let Some(sys_process) = self.system.process(sys_pid) {
            // Update CPU usage
            let cpu_usage = sys_process.cpu_usage();
            process.stats.cpu_usage = cpu_usage;
            self.cpu_cache.insert(pid, cpu_usage);

            // Update memory usage (in bytes)
            process.stats.memory_usage = sys_process.memory();

            // Process is still running
            Ok(())
        } else {
            // Process not found in system - it has crashed or exited
            process.mark_errored();
            self.cpu_cache.remove(&pid);
            Ok(())
        }
    }

    /// Update statistics for multiple managed processes
    ///
    /// # Arguments
    /// * `processes` - Iterator of mutable references to managed processes
    ///
    /// # Returns
    /// * `Ok(())` - All statistics updated successfully
    pub fn update_all_stats<'a, I>(&mut self, processes: I) -> Result<()>
    where
        I: Iterator<Item = &'a mut ManagedProcess>,
    {
        // Refresh all processes at once for efficiency
        self.system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::everything(),
        );

        for process in processes {
            // Only update stats for running processes
            if process.state == ProcessState::Running {
                let _ = self.update_process_stats(process);
            }
        }

        Ok(())
    }

    /// Check if a process is still alive in the system
    ///
    /// # Arguments
    /// * `pid` - Process ID to check
    ///
    /// # Returns
    /// * `true` - Process is alive
    /// * `false` - Process has exited or crashed
    pub fn is_process_alive(&mut self, pid: u32) -> bool {
        let sys_pid = Pid::from_u32(pid);
        self.system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::Some(&[sys_pid]),
            true,
            ProcessRefreshKind::everything(),
        );
        self.system.process(sys_pid).is_some()
    }

    /// Detect crashed processes and mark them as errored
    ///
    /// # Arguments
    /// * `processes` - Iterator of mutable references to managed processes
    ///
    /// # Returns
    /// Vector of PIDs that have crashed
    pub fn detect_crashes<'a, I>(&mut self, processes: I) -> Vec<u32>
    where
        I: Iterator<Item = &'a mut ManagedProcess>,
    {
        let mut crashed = Vec::new();

        for process in processes {
            // Only check running processes
            if process.state == ProcessState::Running {
                let pid = process.stats.pid;
                if !self.is_process_alive(pid) {
                    process.mark_errored();
                    self.cpu_cache.remove(&pid);
                    crashed.push(pid);
                }
            }
        }

        crashed
    }

    /// Clear cached data for a process (call when process is removed)
    ///
    /// # Arguments
    /// * `pid` - Process ID to clear from cache
    pub fn clear_cache(&mut self, pid: u32) {
        self.cpu_cache.remove(&pid);
    }
}

impl Default for ProcessMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProcessConfig;
    use crate::ipc::protocol::ProcessId;
    use crate::process::ProcessStats;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tokio::process::Command;

    fn create_test_config(name: &str) -> ProcessConfig {
        use crate::config::LimitAction;
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
            max_cpu: None,
            limit_action: LimitAction::Log,
            stop_signal: "SIGTERM".to_string(),
            stop_timeout_secs: 2,
        }
    }

    #[tokio::test]
    async fn test_monitor_new() {
        let monitor = ProcessMonitor::new();
        assert_eq!(monitor.cpu_cache.len(), 0);
    }

    #[tokio::test]
    async fn test_is_process_alive() {
        let mut monitor = ProcessMonitor::new();

        // Spawn a real process
        let mut child = Command::new("/bin/sleep")
            .arg("5")
            .spawn()
            .expect("Failed to spawn process");

        let pid = child.id().expect("Failed to get PID");

        // Process should be alive
        assert!(monitor.is_process_alive(pid));

        // Kill the process
        child.kill().await.expect("Failed to kill process");
        let _ = child.wait().await;

        // Give system time to update
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Process should be dead
        assert!(!monitor.is_process_alive(pid));
    }

    #[tokio::test]
    async fn test_update_process_stats() {
        let mut monitor = ProcessMonitor::new();

        // Spawn a real process
        let child = Command::new("/bin/sleep")
            .arg("5")
            .spawn()
            .expect("Failed to spawn process");

        let pid = child.id().expect("Failed to get PID");

        // Create a mock managed process
        let config = create_test_config("test-monitor");
        let mut process = ManagedProcess {
            id: ProcessId::new(1),
            name: "test-monitor".to_string(),
            config: config.clone(),
            state: ProcessState::Running,
            child,
            stats: ProcessStats::new(pid),
            restart_policy: crate::process::RestartPolicy::from_config(
                config.autorestart,
                config.max_restarts,
                config.restart_delay_secs,
            ),
            restart_tracker: crate::process::RestartTracker::new(),
            cgroup_manager: None,
        };

        // Update stats
        let result = monitor.update_process_stats(&mut process);
        assert!(result.is_ok());

        // Stats should be updated
        // CPU usage might be 0 for a sleeping process, but memory should be > 0
        assert!(process.stats.memory_usage > 0);

        // Cleanup
        let _ = process.child.kill().await;
    }

    #[tokio::test]
    async fn test_detect_crashes() {
        let mut monitor = ProcessMonitor::new();

        // Spawn a process that will exit quickly
        let child = Command::new("/bin/sh")
            .arg("-c")
            .arg("exit 1")
            .spawn()
            .expect("Failed to spawn process");

        let pid = child.id().expect("Failed to get PID");

        let config = create_test_config("test-crash");
        let mut process = ManagedProcess {
            id: ProcessId::new(1),
            name: "test-crash".to_string(),
            config: config.clone(),
            state: ProcessState::Running,
            child,
            stats: ProcessStats::new(pid),
            restart_policy: crate::process::RestartPolicy::from_config(
                config.autorestart,
                config.max_restarts,
                config.restart_delay_secs,
            ),
            restart_tracker: crate::process::RestartTracker::new(),
            cgroup_manager: None,
        };

        // Wait for process to exit
        let _ = process.child.wait().await;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Detect crashes
        let mut processes = vec![&mut process];
        let crashed = monitor.detect_crashes(processes.iter_mut().map(|p| &mut **p));

        // Process should be detected as crashed
        assert_eq!(crashed.len(), 1);
        assert_eq!(crashed[0], pid);
        assert_eq!(process.state, ProcessState::Errored);
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let mut monitor = ProcessMonitor::new();

        // Add some data to cache
        monitor.cpu_cache.insert(123, 50.0);
        monitor.cpu_cache.insert(456, 75.0);

        assert_eq!(monitor.cpu_cache.len(), 2);

        // Clear one entry
        monitor.clear_cache(123);
        assert_eq!(monitor.cpu_cache.len(), 1);
        assert!(!monitor.cpu_cache.contains_key(&123));
        assert!(monitor.cpu_cache.contains_key(&456));

        // Clear another
        monitor.clear_cache(456);
        assert_eq!(monitor.cpu_cache.len(), 0);
    }

    #[test]
    fn test_monitor_default() {
        let monitor = ProcessMonitor::default();
        assert_eq!(monitor.cpu_cache.len(), 0);
    }
}
