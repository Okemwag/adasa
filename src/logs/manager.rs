use crate::error::{AdasaError, Result};
use crate::logs::{LogWriter, LogEntry, LogReadOptions, LogStream};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;

/// LogManager handles log capture and routing for all managed processes
pub struct LogManager {
    /// Directory where all log files are stored
    log_dir: PathBuf,
    /// Map of process ID to LogWriter
    writers: HashMap<u64, LogWriter>,
}

impl LogManager {
    /// Create a new LogManager with the specified log directory
    ///
    /// # Arguments
    /// * `log_dir` - Directory where log files will be stored
    ///
    /// # Returns
    /// * `Ok(LogManager)` - Successfully created log manager
    /// * `Err(AdasaError)` - Failed to create log directory
    pub async fn new<P: AsRef<Path>>(log_dir: P) -> Result<Self> {
        let log_dir = log_dir.as_ref().to_path_buf();

        // Create log directory if it doesn't exist
        tokio::fs::create_dir_all(&log_dir)
            .await
            .map_err(|e| AdasaError::LogError(format!("Failed to create log directory: {}", e)))?;

        Ok(Self {
            log_dir,
            writers: HashMap::new(),
        })
    }

    /// Create a new LogWriter for a process
    ///
    /// # Arguments
    /// * `process_id` - Unique process ID
    /// * `process_name` - Name of the process
    ///
    /// # Returns
    /// * `Ok(())` - Successfully created logger
    /// * `Err(AdasaError)` - Failed to create logger
    pub async fn create_logger(&mut self, process_id: u64, process_name: &str) -> Result<()> {
        // Check if logger already exists
        if self.writers.contains_key(&process_id) {
            return Err(AdasaError::LogError(format!(
                "Logger already exists for process {}",
                process_id
            )));
        }

        // Create a new LogWriter
        let writer = LogWriter::new(&self.log_dir, process_name, process_id).await?;

        // Store the writer
        self.writers.insert(process_id, writer);

        Ok(())
    }

    /// Write data to stdout log for a process
    ///
    /// # Arguments
    /// * `process_id` - Process ID
    /// * `data` - Data to write
    ///
    /// # Returns
    /// * `Ok(())` - Successfully wrote data
    /// * `Err(AdasaError)` - Failed to write data or logger not found
    pub async fn write_stdout(&mut self, process_id: u64, data: &[u8]) -> Result<()> {
        let writer = self.writers.get_mut(&process_id).ok_or_else(|| {
            AdasaError::LogError(format!("No logger found for process {}", process_id))
        })?;

        writer.write_stdout(data).await
    }

    /// Write data to stderr log for a process
    ///
    /// # Arguments
    /// * `process_id` - Process ID
    /// * `data` - Data to write
    ///
    /// # Returns
    /// * `Ok(())` - Successfully wrote data
    /// * `Err(AdasaError)` - Failed to write data or logger not found
    pub async fn write_stderr(&mut self, process_id: u64, data: &[u8]) -> Result<()> {
        let writer = self.writers.get_mut(&process_id).ok_or_else(|| {
            AdasaError::LogError(format!("No logger found for process {}", process_id))
        })?;

        writer.write_stderr(data).await
    }

    /// Capture stdout and stderr from a child process and route to LogWriter
    ///
    /// This spawns background tasks that continuously read from the process pipes
    /// and write to the appropriate log files.
    ///
    /// # Arguments
    /// * `process_id` - Process ID
    /// * `process_name` - Process name (for creating log files if needed)
    /// * `child` - Mutable reference to the child process
    ///
    /// # Returns
    /// * `Ok(())` - Successfully started log capture
    /// * `Err(AdasaError)` - Failed to capture logs
    pub async fn capture_logs(
        &mut self,
        process_id: u64,
        process_name: &str,
        child: &mut Child,
    ) -> Result<()> {
        // Ensure logger exists
        if !self.writers.contains_key(&process_id) {
            return Err(AdasaError::LogError(format!(
                "No logger found for process {}",
                process_id
            )));
        }

        // Take stdout pipe from child
        let stdout = child.stdout.take().ok_or_else(|| {
            AdasaError::LogError(format!(
                "No stdout pipe available for process {}",
                process_id
            ))
        })?;

        // Take stderr pipe from child
        let stderr = child.stderr.take().ok_or_else(|| {
            AdasaError::LogError(format!(
                "No stderr pipe available for process {}",
                process_id
            ))
        })?;

        // Spawn task to read stdout
        let stdout_reader = BufReader::new(stdout);
        let log_dir = self.log_dir.clone();
        let process_name = process_name.to_string();
        tokio::spawn(Self::read_stdout_task(
            process_id,
            process_name.clone(),
            stdout_reader,
            log_dir.clone(),
        ));

        // Spawn task to read stderr
        let stderr_reader = BufReader::new(stderr);
        tokio::spawn(Self::read_stderr_task(
            process_id,
            process_name,
            stderr_reader,
            log_dir,
        ));

        Ok(())
    }

    /// Background task to read stdout from a process
    async fn read_stdout_task(
        process_id: u64,
        process_name: String,
        mut reader: BufReader<tokio::process::ChildStdout>,
        log_dir: PathBuf,
    ) {
        // Create a dedicated LogWriter for this task
        let mut writer = match LogWriter::new(&log_dir, &process_name, process_id).await {
            Ok(w) => w,
            Err(_) => return,
        };

        let mut line = String::new();

        loop {
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    // EOF - process closed stdout
                    break;
                }
                Ok(_) => {
                    // Write line to log file
                    let _ = writer.write_stdout(line.as_bytes()).await;
                    line.clear();
                }
                Err(_) => {
                    // Error reading - process may have crashed
                    break;
                }
            }
        }

        // Flush on exit
        let _ = writer.flush().await;
    }

    /// Background task to read stderr from a process
    async fn read_stderr_task(
        process_id: u64,
        process_name: String,
        mut reader: BufReader<tokio::process::ChildStderr>,
        log_dir: PathBuf,
    ) {
        // Create a dedicated LogWriter for this task
        let mut writer = match LogWriter::new(&log_dir, &process_name, process_id).await {
            Ok(w) => w,
            Err(_) => return,
        };

        let mut line = String::new();

        loop {
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    // EOF - process closed stderr
                    break;
                }
                Ok(_) => {
                    // Write line to log file
                    let _ = writer.write_stderr(line.as_bytes()).await;
                    line.clear();
                }
                Err(_) => {
                    // Error reading - process may have crashed
                    break;
                }
            }
        }

        // Flush on exit
        let _ = writer.flush().await;
    }

    /// Remove a logger for a process (when process is deleted)
    ///
    /// # Arguments
    /// * `process_id` - Process ID
    ///
    /// # Returns
    /// * `Ok(())` - Successfully removed logger
    /// * `Err(AdasaError)` - Logger not found
    pub fn remove_logger(&mut self, process_id: u64) -> Result<()> {
        self.writers
            .remove(&process_id)
            .ok_or_else(|| {
                AdasaError::LogError(format!("No logger found for process {}", process_id))
            })
            .map(|_| ())
    }

    /// Get the log directory path
    pub fn log_dir(&self) -> &Path {
        &self.log_dir
    }

    /// Check if a logger exists for a process
    pub fn has_logger(&self, process_id: u64) -> bool {
        self.writers.contains_key(&process_id)
    }

    /// Get the number of active loggers
    pub fn logger_count(&self) -> usize {
        self.writers.len()
    }

    /// Flush all log writers to ensure data is written to disk
    pub async fn flush_all(&mut self) -> Result<()> {
        for writer in self.writers.values_mut() {
            writer.flush().await?;
        }
        Ok(())
    }

    /// Read logs for a process
    ///
    /// # Arguments
    /// * `process_id` - Process ID
    /// * `process_name` - Process name
    /// * `options` - Options for reading logs
    ///
    /// # Returns
    /// * `Ok(Vec<LogEntry>)` - Successfully read log entries
    /// * `Err(AdasaError)` - Failed to read logs
    pub async fn read_logs(
        &self,
        process_id: u64,
        process_name: &str,
        options: &LogReadOptions,
    ) -> Result<Vec<LogEntry>> {
        crate::logs::read_logs(&self.log_dir, process_name, process_id, options).await
    }

    /// Create a log stream for real-time log tailing
    ///
    /// # Arguments
    /// * `process_id` - Process ID
    /// * `process_name` - Process name
    /// * `include_stdout` - Whether to include stdout logs
    /// * `include_stderr` - Whether to include stderr logs
    /// * `filter` - Optional filter pattern
    ///
    /// # Returns
    /// * `Ok(LogStream)` - Successfully created log stream
    /// * `Err(AdasaError)` - Failed to create log stream
    pub async fn stream_logs(
        &self,
        process_id: u64,
        process_name: &str,
        include_stdout: bool,
        include_stderr: bool,
        filter: Option<String>,
    ) -> Result<LogStream> {
        LogStream::new(
            self.log_dir.clone(),
            process_name.to_string(),
            process_id,
            include_stdout,
            include_stderr,
            filter,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_log_manager() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();

        let result = LogManager::new(log_dir).await;
        assert!(result.is_ok());

        let manager = result.unwrap();
        assert_eq!(manager.log_dir(), log_dir);
        assert_eq!(manager.logger_count(), 0);
    }

    #[tokio::test]
    async fn test_create_logger() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();

        let mut manager = LogManager::new(log_dir).await.unwrap();

        let result = manager.create_logger(1, "test-process").await;
        assert!(result.is_ok());
        assert_eq!(manager.logger_count(), 1);
        assert!(manager.has_logger(1));
    }

    #[tokio::test]
    async fn test_create_duplicate_logger() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();

        let mut manager = LogManager::new(log_dir).await.unwrap();

        manager.create_logger(1, "test-process").await.unwrap();

        let result = manager.create_logger(1, "test-process").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_write_stdout() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();

        let mut manager = LogManager::new(log_dir).await.unwrap();
        manager.create_logger(1, "test-process").await.unwrap();

        let result = manager.write_stdout(1, b"Test stdout message").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_write_stderr() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();

        let mut manager = LogManager::new(log_dir).await.unwrap();
        manager.create_logger(1, "test-process").await.unwrap();

        let result = manager.write_stderr(1, b"Test stderr message").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_write_without_logger() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();

        let mut manager = LogManager::new(log_dir).await.unwrap();

        let result = manager.write_stdout(999, b"Test message").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_remove_logger() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();

        let mut manager = LogManager::new(log_dir).await.unwrap();
        manager.create_logger(1, "test-process").await.unwrap();

        assert_eq!(manager.logger_count(), 1);

        let result = manager.remove_logger(1);
        assert!(result.is_ok());
        assert_eq!(manager.logger_count(), 0);
        assert!(!manager.has_logger(1));
    }

    #[tokio::test]
    async fn test_remove_nonexistent_logger() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();

        let mut manager = LogManager::new(log_dir).await.unwrap();

        let result = manager.remove_logger(999);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_multiple_loggers() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();

        let mut manager = LogManager::new(log_dir).await.unwrap();

        manager.create_logger(1, "process-1").await.unwrap();
        manager.create_logger(2, "process-2").await.unwrap();
        manager.create_logger(3, "process-3").await.unwrap();

        assert_eq!(manager.logger_count(), 3);
        assert!(manager.has_logger(1));
        assert!(manager.has_logger(2));
        assert!(manager.has_logger(3));
    }

    #[tokio::test]
    async fn test_flush_all() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();

        let mut manager = LogManager::new(log_dir).await.unwrap();
        manager.create_logger(1, "test-process").await.unwrap();
        manager.write_stdout(1, b"Test message").await.unwrap();

        let result = manager.flush_all().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_capture_logs() {
        use std::process::Stdio;
        use tokio::process::Command;

        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();

        let mut manager = LogManager::new(log_dir).await.unwrap();
        manager.create_logger(1, "test-echo").await.unwrap();

        // Spawn a process that outputs to stdout
        let mut child = Command::new("/bin/echo")
            .arg("Hello from stdout")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        // Capture logs
        let result = manager.capture_logs(1, "test-echo", &mut child).await;
        assert!(result.is_ok());

        // Wait for process to complete
        let _ = child.wait().await;

        // Give some time for logs to be written
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Flush to ensure data is written
        manager.flush_all().await.unwrap();
    }

    #[tokio::test]
    async fn test_capture_logs_without_logger() {
        use std::process::Stdio;
        use tokio::process::Command;

        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();

        let mut manager = LogManager::new(log_dir).await.unwrap();

        // Don't create logger

        let mut child = Command::new("/bin/echo")
            .arg("Test")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        // Should fail because logger doesn't exist
        let result = manager.capture_logs(1, "test", &mut child).await;
        assert!(result.is_err());

        // Cleanup
        let _ = child.wait().await;
    }
}
