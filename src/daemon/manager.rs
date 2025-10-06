// Daemon lifecycle management

use super::pid::PidFile;
use crate::error::{AdasaError, Result};
use std::time::Duration;

#[cfg(unix)]
use nix::sys::signal::{kill, Signal};
#[cfg(unix)]
use nix::unistd::Pid;

/// Daemon manager for controlling daemon lifecycle
pub struct DaemonManager {
    pid_file: PidFile,
}

impl DaemonManager {
    /// Create a new daemon manager with default PID file
    pub fn new() -> Self {
        Self {
            pid_file: PidFile::new(),
        }
    }

    /// Create a new daemon manager with custom PID file path
    pub fn with_pid_file(pid_file: PidFile) -> Self {
        Self { pid_file }
    }

    /// Check if the daemon is currently running
    pub fn is_running(&self) -> bool {
        self.pid_file.is_daemon_running()
    }

    /// Get the PID of the running daemon, if any
    pub fn get_pid(&self) -> Option<u32> {
        if self.is_running() {
            self.pid_file.read().ok()
        } else {
            None
        }
    }

    /// Start the daemon (called from within the daemon process)
    pub fn register_daemon(&self) -> Result<()> {
        // Check if daemon is already running
        if self.is_running() {
            return Err(AdasaError::Other("Daemon is already running".to_string()));
        }

        // Clean up stale PID file if it exists
        if self.pid_file.exists() {
            self.pid_file.remove()?;
        }

        // Write current process PID
        self.pid_file.write()?;

        Ok(())
    }

    /// Stop the daemon by sending SIGTERM
    #[cfg(unix)]
    pub fn stop_daemon(&self, timeout_secs: u64) -> Result<()> {
        let pid = self.get_pid().ok_or_else(|| AdasaError::DaemonNotRunning)?;

        println!("Stopping daemon (PID: {})...", pid);

        // Send SIGTERM
        let pid_t = Pid::from_raw(pid as i32);
        kill(pid_t, Signal::SIGTERM)
            .map_err(|e| AdasaError::Other(format!("Failed to send SIGTERM: {}", e)))?;

        // Wait for daemon to stop
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(timeout_secs);

        while start.elapsed() < timeout {
            if !self.is_running() {
                println!("Daemon stopped successfully");
                self.pid_file.remove()?;
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        // If still running, send SIGKILL
        if self.is_running() {
            println!("Daemon did not stop gracefully, sending SIGKILL...");
            kill(pid_t, Signal::SIGKILL)
                .map_err(|e| AdasaError::Other(format!("Failed to send SIGKILL: {}", e)))?;

            // Wait a bit more
            std::thread::sleep(Duration::from_secs(1));

            if !self.is_running() {
                println!("Daemon force-stopped");
                self.pid_file.remove()?;
                return Ok(());
            }

            return Err(AdasaError::Other(
                "Failed to stop daemon even with SIGKILL".to_string(),
            ));
        }

        Ok(())
    }

    #[cfg(not(unix))]
    pub fn stop_daemon(&self, _timeout_secs: u64) -> Result<()> {
        Err(AdasaError::Other(
            "Daemon stop is only supported on Unix systems".to_string(),
        ))
    }

    /// Unregister the daemon (called during daemon shutdown)
    pub fn unregister_daemon(&self) -> Result<()> {
        self.pid_file.remove()
    }

    /// Get daemon status information
    pub fn get_status(&self) -> DaemonStatus {
        if let Some(pid) = self.get_pid() {
            DaemonStatus {
                running: true,
                pid: Some(pid),
                pid_file: self.pid_file.path().to_path_buf(),
            }
        } else {
            DaemonStatus {
                running: false,
                pid: None,
                pid_file: self.pid_file.path().to_path_buf(),
            }
        }
    }
}

impl Default for DaemonManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Daemon status information
#[derive(Debug, Clone)]
pub struct DaemonStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub pid_file: std::path::PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_daemon_manager_not_running() {
        let temp_file = NamedTempFile::new().unwrap();
        std::fs::remove_file(temp_file.path()).ok();

        let pid_file = PidFile::with_path(temp_file.path());
        let manager = DaemonManager::with_pid_file(pid_file);

        assert!(!manager.is_running());
        assert!(manager.get_pid().is_none());
    }

    #[test]
    fn test_register_daemon() {
        let temp_file = NamedTempFile::new().unwrap();
        std::fs::remove_file(temp_file.path()).ok();

        let pid_file = PidFile::with_path(temp_file.path());
        let manager = DaemonManager::with_pid_file(pid_file);

        // Register daemon
        manager.register_daemon().unwrap();

        // Should be running now
        assert!(manager.is_running());
        assert_eq!(manager.get_pid(), Some(std::process::id()));

        // Clean up
        manager.unregister_daemon().unwrap();
    }

    #[test]
    fn test_get_status() {
        let temp_file = NamedTempFile::new().unwrap();
        std::fs::remove_file(temp_file.path()).ok();

        let pid_file = PidFile::with_path(temp_file.path());
        let manager = DaemonManager::with_pid_file(pid_file);

        // Not running
        let status = manager.get_status();
        assert!(!status.running);
        assert!(status.pid.is_none());

        // Register and check again
        manager.register_daemon().unwrap();
        let status = manager.get_status();
        assert!(status.running);
        assert_eq!(status.pid, Some(std::process::id()));

        // Clean up
        manager.unregister_daemon().unwrap();
    }
}
