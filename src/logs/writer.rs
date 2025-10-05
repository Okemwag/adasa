use crate::error::{AdasaError, Result};
use chrono::{DateTime, Local};
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tokio::fs::File as TokioFile;

/// Default maximum log file size before rotation (10MB)
const DEFAULT_MAX_LOG_SIZE: u64 = 10 * 1024 * 1024;

/// LogWriter handles writing stdout and stderr logs for a single process
/// with automatic rotation based on file size
pub struct LogWriter {
    /// Path to stdout log file
    stdout_path: PathBuf,
    /// Path to stderr log file
    stderr_path: PathBuf,
    /// Async file handle for stdout
    stdout_file: TokioFile,
    /// Async file handle for stderr
    stderr_file: TokioFile,
    /// Maximum size in bytes before rotation
    max_size: u64,
    /// Current size of stdout file
    stdout_size: u64,
    /// Current size of stderr file
    stderr_size: u64,
}

impl LogWriter {
    /// Create a new LogWriter for a process
    ///
    /// # Arguments
    /// * `log_dir` - Directory where log files will be stored
    /// * `process_name` - Name of the process (used for log file naming)
    /// * `process_id` - ID of the process (used for log file naming)
    ///
    /// # Returns
    /// * `Ok(LogWriter)` - Successfully created log writer
    /// * `Err(AdasaError)` - Failed to create log files
    pub async fn new(
        log_dir: &Path,
        process_name: &str,
        process_id: u64,
    ) -> Result<Self> {
        Self::with_max_size(log_dir, process_name, process_id, DEFAULT_MAX_LOG_SIZE).await
    }

    /// Create a new LogWriter with custom maximum log size
    ///
    /// # Arguments
    /// * `log_dir` - Directory where log files will be stored
    /// * `process_name` - Name of the process
    /// * `process_id` - ID of the process
    /// * `max_size` - Maximum size in bytes before rotation
    pub async fn with_max_size(
        log_dir: &Path,
        process_name: &str,
        process_id: u64,
        max_size: u64,
    ) -> Result<Self> {
        // Create log directory if it doesn't exist
        tokio::fs::create_dir_all(log_dir)
            .await
            .map_err(|e| AdasaError::LogError(format!("Failed to create log directory: {}", e)))?;

        // Generate log file paths
        let stdout_path = log_dir.join(format!("{}-{}-out.log", process_name, process_id));
        let stderr_path = log_dir.join(format!("{}-{}-err.log", process_name, process_id));

        // Open log files in append mode
        let stdout_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&stdout_path)
            .map_err(|e| AdasaError::LogFileError(format!("Failed to open stdout log: {}", e)))?;

        let stderr_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&stderr_path)
            .map_err(|e| AdasaError::LogFileError(format!("Failed to open stderr log: {}", e)))?;

        // Get current file sizes
        let stdout_size = stdout_file
            .metadata()
            .map(|m| m.len())
            .unwrap_or(0);
        let stderr_size = stderr_file
            .metadata()
            .map(|m| m.len())
            .unwrap_or(0);

        // Convert to tokio files
        let stdout_file = TokioFile::from_std(stdout_file);
        let stderr_file = TokioFile::from_std(stderr_file);

        Ok(Self {
            stdout_path,
            stderr_path,
            stdout_file,
            stderr_file,
            max_size,
            stdout_size,
            stderr_size,
        })
    }

    /// Write data to stdout log with timestamp
    ///
    /// # Arguments
    /// * `data` - Raw bytes to write
    ///
    /// # Returns
    /// * `Ok(())` - Successfully wrote data
    /// * `Err(AdasaError)` - Failed to write data
    pub async fn write_stdout(&mut self, data: &[u8]) -> Result<()> {
        // Check if rotation is needed before writing
        if self.stdout_size >= self.max_size {
            let stdout_path = self.stdout_path.clone();
            self.rotate_log(&stdout_path, "out").await?;
            // Reopen the file after rotation
            self.stdout_file = self.reopen_file(&stdout_path).await?;
            self.stdout_size = 0;
        }

        // Create timestamped entry
        let timestamp = Local::now();
        let timestamped_data = self.format_log_entry(&timestamp, data);

        // Write to file
        self.stdout_file.write_all(&timestamped_data)
            .await
            .map_err(|e| AdasaError::LogError(format!("Failed to write to log: {}", e)))?;

        // Flush to ensure data is written
        self.stdout_file.flush()
            .await
            .map_err(|e| AdasaError::LogError(format!("Failed to flush log: {}", e)))?;

        // Update size
        self.stdout_size += timestamped_data.len() as u64;

        Ok(())
    }

    /// Write data to stderr log with timestamp
    ///
    /// # Arguments
    /// * `data` - Raw bytes to write
    ///
    /// # Returns
    /// * `Ok(())` - Successfully wrote data
    /// * `Err(AdasaError)` - Failed to write data
    pub async fn write_stderr(&mut self, data: &[u8]) -> Result<()> {
        // Check if rotation is needed before writing
        if self.stderr_size >= self.max_size {
            let stderr_path = self.stderr_path.clone();
            self.rotate_log(&stderr_path, "err").await?;
            // Reopen the file after rotation
            self.stderr_file = self.reopen_file(&stderr_path).await?;
            self.stderr_size = 0;
        }

        // Create timestamped entry
        let timestamp = Local::now();
        let timestamped_data = self.format_log_entry(&timestamp, data);

        // Write to file
        self.stderr_file.write_all(&timestamped_data)
            .await
            .map_err(|e| AdasaError::LogError(format!("Failed to write to log: {}", e)))?;

        // Flush to ensure data is written
        self.stderr_file.flush()
            .await
            .map_err(|e| AdasaError::LogError(format!("Failed to flush log: {}", e)))?;

        // Update size
        self.stderr_size += timestamped_data.len() as u64;

        Ok(())
    }

    /// Format a log entry with timestamp
    fn format_log_entry(&self, timestamp: &DateTime<Local>, data: &[u8]) -> Vec<u8> {
        let timestamp_str = timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string();
        let mut entry = Vec::with_capacity(timestamp_str.len() + 3 + data.len());
        
        // Format: [YYYY-MM-DD HH:MM:SS.mmm] <data>
        entry.extend_from_slice(b"[");
        entry.extend_from_slice(timestamp_str.as_bytes());
        entry.extend_from_slice(b"] ");
        entry.extend_from_slice(data);
        
        // Ensure newline at end if not present
        if !data.ends_with(b"\n") {
            entry.push(b'\n');
        }
        
        entry
    }

    /// Rotate a log file by renaming it with a timestamp
    async fn rotate_log(&self, file_path: &Path, _log_type: &str) -> Result<()> {
        let timestamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
        let parent = file_path.parent().ok_or_else(|| {
            AdasaError::LogRotationError("Invalid log file path".to_string())
        })?;
        
        let file_stem = file_path.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
            AdasaError::LogRotationError("Invalid log file name".to_string())
        })?;
        
        let rotated_path = parent.join(format!("{}-{}.log", file_stem, timestamp));
        
        // Rename the current log file
        tokio::fs::rename(file_path, &rotated_path)
            .await
            .map_err(|e| AdasaError::LogRotationError(format!("Failed to rotate log: {}", e)))?;
        
        Ok(())
    }

    /// Reopen a log file after rotation
    async fn reopen_file(&self, file_path: &Path) -> Result<TokioFile> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)
            .map_err(|e| AdasaError::LogFileError(format!("Failed to reopen log file: {}", e)))?;
        
        Ok(TokioFile::from_std(file))
    }

    /// Get the path to the stdout log file
    pub fn stdout_path(&self) -> &Path {
        &self.stdout_path
    }

    /// Get the path to the stderr log file
    pub fn stderr_path(&self) -> &Path {
        &self.stderr_path
    }

    /// Get the current size of the stdout log file
    pub fn stdout_size(&self) -> u64 {
        self.stdout_size
    }

    /// Get the current size of the stderr log file
    pub fn stderr_size(&self) -> u64 {
        self.stderr_size
    }

    /// Get the maximum log file size before rotation
    pub fn max_size(&self) -> u64 {
        self.max_size
    }

    /// Flush both log files to ensure all data is written
    pub async fn flush(&mut self) -> Result<()> {
        self.stdout_file
            .flush()
            .await
            .map_err(|e| AdasaError::LogError(format!("Failed to flush stdout: {}", e)))?;
        
        self.stderr_file
            .flush()
            .await
            .map_err(|e| AdasaError::LogError(format!("Failed to flush stderr: {}", e)))?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_log_writer() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();

        let writer = LogWriter::new(log_dir, "test-process", 1).await;
        assert!(writer.is_ok());

        let writer = writer.unwrap();
        assert_eq!(writer.max_size(), DEFAULT_MAX_LOG_SIZE);
        assert!(writer.stdout_path().exists());
        assert!(writer.stderr_path().exists());
    }

    #[tokio::test]
    async fn test_write_stdout() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();

        let mut writer = LogWriter::new(log_dir, "test-process", 1).await.unwrap();
        
        let result = writer.write_stdout(b"Hello, stdout!").await;
        assert!(result.is_ok());

        // Flush to ensure data is written
        writer.flush().await.unwrap();

        // Read the file and verify content
        let content = tokio::fs::read_to_string(writer.stdout_path()).await.unwrap();
        assert!(content.contains("Hello, stdout!"));
        assert!(content.starts_with("["));
    }

    #[tokio::test]
    async fn test_write_stderr() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();

        let mut writer = LogWriter::new(log_dir, "test-process", 1).await.unwrap();
        
        let result = writer.write_stderr(b"Error message").await;
        assert!(result.is_ok());

        writer.flush().await.unwrap();

        let content = tokio::fs::read_to_string(writer.stderr_path()).await.unwrap();
        assert!(content.contains("Error message"));
        assert!(content.starts_with("["));
    }

    #[tokio::test]
    async fn test_log_rotation() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();

        // Create writer with small max size to trigger rotation
        let mut writer = LogWriter::with_max_size(log_dir, "test-process", 1, 100).await.unwrap();
        
        // Write enough data to trigger rotation
        for _ in 0..10 {
            writer.write_stdout(b"This is a test log entry").await.unwrap();
        }

        writer.flush().await.unwrap();

        // Check that rotation occurred by looking for rotated files
        let entries: Vec<_> = std::fs::read_dir(log_dir).unwrap().collect();
        let log_files: Vec<_> = entries
            .iter()
            .filter_map(|e| e.as_ref().ok())
            .filter(|e| {
                e.path()
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.contains("test-process") && n.ends_with(".log"))
                    .unwrap_or(false)
            })
            .collect();

        // Should have at least 2 files (current + rotated)
        assert!(log_files.len() >= 2, "Expected at least 2 log files, found {}", log_files.len());
    }

    #[tokio::test]
    async fn test_timestamped_entries() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();

        let mut writer = LogWriter::new(log_dir, "test-process", 1).await.unwrap();
        
        writer.write_stdout(b"Line 1").await.unwrap();
        writer.write_stdout(b"Line 2").await.unwrap();
        writer.flush().await.unwrap();

        let content = tokio::fs::read_to_string(writer.stdout_path()).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();
        
        assert_eq!(lines.len(), 2);
        // Each line should start with a timestamp in brackets
        for line in lines {
            assert!(line.starts_with("["));
            assert!(line.contains("]"));
        }
    }

    #[tokio::test]
    async fn test_size_tracking() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();

        let mut writer = LogWriter::new(log_dir, "test-process", 1).await.unwrap();
        
        let initial_size = writer.stdout_size();
        writer.write_stdout(b"Test data").await.unwrap();
        writer.flush().await.unwrap();
        
        assert!(writer.stdout_size() > initial_size);
    }
}
