// PID file management for daemon process

use crate::error::{AdasaError, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Default PID file location
const DEFAULT_PID_FILE: &str = "/tmp/adasa.pid";

/// Manages the daemon PID file
pub struct PidFile {
    path: PathBuf,
}

impl PidFile {
    /// Create a new PID file manager with default path
    pub fn new() -> Self {
        Self {
            path: PathBuf::from(DEFAULT_PID_FILE),
        }
    }

    /// Create a new PID file manager with custom path
    pub fn with_path<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Write the current process PID to the file
    pub fn write(&self) -> Result<()> {
        let pid = std::process::id();
        fs::write(&self.path, pid.to_string())
            .map_err(|e| AdasaError::StateError(format!("Failed to write PID file: {}", e)))?;
        Ok(())
    }

    /// Read the PID from the file
    pub fn read(&self) -> Result<u32> {
        let content = fs::read_to_string(&self.path)
            .map_err(|e| AdasaError::StateError(format!("Failed to read PID file: {}", e)))?;

        content
            .trim()
            .parse::<u32>()
            .map_err(|e| AdasaError::StateError(format!("Invalid PID in file: {}", e)))
    }

    /// Check if the PID file exists
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Remove the PID file
    pub fn remove(&self) -> Result<()> {
        if self.exists() {
            fs::remove_file(&self.path)
                .map_err(|e| AdasaError::StateError(format!("Failed to remove PID file: {}", e)))?;
        }
        Ok(())
    }

    /// Check if the daemon is running by checking if the PID exists and is alive
    pub fn is_daemon_running(&self) -> bool {
        if !self.exists() {
            return false;
        }

        match self.read() {
            Ok(pid) => self.is_process_alive(pid),
            Err(_) => false,
        }
    }

    /// Check if a process with the given PID is alive
    #[cfg(unix)]
    fn is_process_alive(&self, pid: u32) -> bool {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;

        // Send signal 0 to check if process exists
        match kill(Pid::from_raw(pid as i32), Signal::SIGCONT) {
            Ok(_) => true,
            Err(nix::errno::Errno::ESRCH) => false, // Process doesn't exist
            Err(nix::errno::Errno::EPERM) => true,  // Process exists but we don't have permission
            Err(_) => false,
        }
    }

    #[cfg(not(unix))]
    fn is_process_alive(&self, _pid: u32) -> bool {
        // On non-Unix systems, we can't easily check if a process is alive
        // For now, just assume it is if the PID file exists
        true
    }

    /// Get the path to the PID file
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Default for PidFile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_write_and_read_pid() {
        let temp_file = NamedTempFile::new().unwrap();
        let pid_file = PidFile::with_path(temp_file.path());

        // Write PID
        pid_file.write().unwrap();

        // Read PID
        let pid = pid_file.read().unwrap();
        assert_eq!(pid, std::process::id());
    }

    #[test]
    fn test_exists() {
        let temp_file = NamedTempFile::new().unwrap();
        let pid_file = PidFile::with_path(temp_file.path());

        // Initially doesn't exist (we haven't written to it)
        fs::remove_file(temp_file.path()).ok();
        assert!(!pid_file.exists());

        // Write and check existence
        pid_file.write().unwrap();
        assert!(pid_file.exists());
    }

    #[test]
    fn test_remove() {
        let temp_file = NamedTempFile::new().unwrap();
        let pid_file = PidFile::with_path(temp_file.path());

        // Write PID
        pid_file.write().unwrap();
        assert!(pid_file.exists());

        // Remove
        pid_file.remove().unwrap();
        assert!(!pid_file.exists());
    }

    #[test]
    fn test_is_daemon_running_current_process() {
        let temp_file = NamedTempFile::new().unwrap();
        let pid_file = PidFile::with_path(temp_file.path());

        // Write current process PID
        pid_file.write().unwrap();

        // Should be running (current process)
        assert!(pid_file.is_daemon_running());
    }
}
