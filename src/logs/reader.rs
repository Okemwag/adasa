use crate::error::{AdasaError, Result};
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

/// Options for reading logs
#[derive(Debug, Clone)]
pub struct LogReadOptions {
    /// Number of lines to read from the end of the file
    pub lines: usize,
    /// Whether to include stderr logs
    pub include_stderr: bool,
    /// Whether to include stdout logs
    pub include_stdout: bool,
    /// Optional filter pattern (simple substring match)
    pub filter: Option<String>,
}

impl Default for LogReadOptions {
    fn default() -> Self {
        Self {
            lines: 100,
            include_stderr: true,
            include_stdout: true,
            filter: None,
        }
    }
}

/// A formatted log entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// The source of the log (stdout or stderr)
    pub source: LogSource,
    /// The timestamp from the log entry
    pub timestamp: Option<String>,
    /// The log message content
    pub message: String,
}

/// Source of a log entry
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSource {
    Stdout,
    Stderr,
}

impl LogEntry {
    /// Format the log entry for display
    pub fn format(&self) -> String {
        let source_prefix = match self.source {
            LogSource::Stdout => "[OUT]",
            LogSource::Stderr => "[ERR]",
        };

        if let Some(ref timestamp) = self.timestamp {
            format!("{} {} {}", timestamp, source_prefix, self.message)
        } else {
            format!("{} {}", source_prefix, self.message)
        }
    }

    /// Parse a log line into a LogEntry
    fn parse(line: &str, source: LogSource) -> Self {
        // Try to extract timestamp from format: [YYYY-MM-DD HH:MM:SS.mmm] message
        if line.starts_with('[') {
            if let Some(end_bracket) = line.find(']') {
                let timestamp = line[1..end_bracket].to_string();
                let message = line[end_bracket + 1..].trim_start().to_string();
                return Self {
                    source,
                    timestamp: Some(timestamp),
                    message,
                };
            }
        }

        // No timestamp found, return the whole line as message
        Self {
            source,
            timestamp: None,
            message: line.to_string(),
        }
    }
}

/// Read the last N lines from a log file
///
/// # Arguments
/// * `file_path` - Path to the log file
/// * `lines` - Number of lines to read from the end
/// * `source` - Source type (stdout or stderr)
/// * `filter` - Optional filter pattern
///
/// # Returns
/// * `Ok(Vec<LogEntry>)` - Successfully read log entries
/// * `Err(AdasaError)` - Failed to read log file
pub async fn read_last_lines(
    file_path: &Path,
    lines: usize,
    source: LogSource,
    filter: Option<&str>,
) -> Result<Vec<LogEntry>> {
    // Check if file exists
    if !file_path.exists() {
        return Ok(Vec::new());
    }

    // Open the file
    let file = File::open(file_path)
        .await
        .map_err(|e| AdasaError::LogFileError(format!("Failed to open log file: {}", e)))?;

    let reader = BufReader::new(file);
    let mut all_lines = Vec::new();

    // Read all lines
    let mut lines_stream = reader.lines();
    while let Some(line) = lines_stream
        .next_line()
        .await
        .map_err(|e| AdasaError::LogError(format!("Failed to read log line: {}", e)))?
    {
        // Apply filter if specified
        if let Some(filter_pattern) = filter {
            if !line.contains(filter_pattern) {
                continue;
            }
        }

        all_lines.push(line);
    }

    // Take the last N lines
    let start_index = if all_lines.len() > lines {
        all_lines.len() - lines
    } else {
        0
    };

    // Parse lines into LogEntry objects
    let entries: Vec<LogEntry> = all_lines[start_index..]
        .iter()
        .map(|line| LogEntry::parse(line, source))
        .collect();

    Ok(entries)
}

/// Read logs from both stdout and stderr files
///
/// # Arguments
/// * `log_dir` - Directory containing log files
/// * `process_name` - Name of the process
/// * `process_id` - ID of the process
/// * `options` - Options for reading logs
///
/// # Returns
/// * `Ok(Vec<LogEntry>)` - Successfully read log entries
/// * `Err(AdasaError)` - Failed to read log files
pub async fn read_logs(
    log_dir: &Path,
    process_name: &str,
    process_id: u64,
    options: &LogReadOptions,
) -> Result<Vec<LogEntry>> {
    let mut all_entries = Vec::new();

    // Read stdout logs if requested
    if options.include_stdout {
        let stdout_path = log_dir.join(format!("{}-{}-out.log", process_name, process_id));
        let stdout_entries = read_last_lines(
            &stdout_path,
            options.lines,
            LogSource::Stdout,
            options.filter.as_deref(),
        )
        .await?;
        all_entries.extend(stdout_entries);
    }

    // Read stderr logs if requested
    if options.include_stderr {
        let stderr_path = log_dir.join(format!("{}-{}-err.log", process_name, process_id));
        let stderr_entries = read_last_lines(
            &stderr_path,
            options.lines,
            LogSource::Stderr,
            options.filter.as_deref(),
        )
        .await?;
        all_entries.extend(stderr_entries);
    }

    // Sort by timestamp if available (simple lexicographic sort works for ISO format)
    all_entries.sort_by(|a, b| {
        match (&a.timestamp, &b.timestamp) {
            (Some(ts_a), Some(ts_b)) => ts_a.cmp(ts_b),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    // Limit to requested number of lines
    if all_entries.len() > options.lines {
        let start = all_entries.len() - options.lines;
        all_entries = all_entries[start..].to_vec();
    }

    Ok(all_entries)
}

/// A stream for tailing log files in real-time
pub struct LogStream {
    /// Receiver for log entries
    receiver: mpsc::Receiver<LogEntry>,
    /// Handle to the background task
    _task_handle: tokio::task::JoinHandle<()>,
}

impl LogStream {
    /// Create a new LogStream for tailing log files
    ///
    /// # Arguments
    /// * `log_dir` - Directory containing log files
    /// * `process_name` - Name of the process
    /// * `process_id` - ID of the process
    /// * `include_stdout` - Whether to tail stdout
    /// * `include_stderr` - Whether to tail stderr
    /// * `filter` - Optional filter pattern
    ///
    /// # Returns
    /// * `Ok(LogStream)` - Successfully created log stream
    /// * `Err(AdasaError)` - Failed to create log stream
    pub async fn new(
        log_dir: PathBuf,
        process_name: String,
        process_id: u64,
        include_stdout: bool,
        include_stderr: bool,
        filter: Option<String>,
    ) -> Result<Self> {
        let (tx, rx) = mpsc::channel(100);

        // Spawn background task to tail log files
        let task_handle = tokio::spawn(async move {
            let mut stdout_reader = if include_stdout {
                let stdout_path = log_dir.join(format!("{}-{}-out.log", process_name, process_id));
                Some(LogTailer::new(stdout_path, LogSource::Stdout).await)
            } else {
                None
            };

            let mut stderr_reader = if include_stderr {
                let stderr_path = log_dir.join(format!("{}-{}-err.log", process_name, process_id));
                Some(LogTailer::new(stderr_path, LogSource::Stderr).await)
            } else {
                None
            };

            loop {
                let mut has_data = false;

                // Read from stdout
                if let Some(ref mut tailer) = stdout_reader {
                    if let Ok(Some(entry)) = tailer.read_next().await {
                        // Apply filter
                        let should_send = if let Some(ref filter_pattern) = filter {
                            entry.message.contains(filter_pattern)
                        } else {
                            true
                        };

                        if should_send {
                            if tx.send(entry).await.is_err() {
                                // Receiver dropped, exit
                                break;
                            }
                            has_data = true;
                        }
                    }
                }

                // Read from stderr
                if let Some(ref mut tailer) = stderr_reader {
                    if let Ok(Some(entry)) = tailer.read_next().await {
                        // Apply filter
                        let should_send = if let Some(ref filter_pattern) = filter {
                            entry.message.contains(filter_pattern)
                        } else {
                            true
                        };

                        if should_send {
                            if tx.send(entry).await.is_err() {
                                // Receiver dropped, exit
                                break;
                            }
                            has_data = true;
                        }
                    }
                }

                // If no data was read, sleep briefly to avoid busy-waiting
                if !has_data {
                    sleep(Duration::from_millis(100)).await;
                }
            }
        });

        Ok(Self {
            receiver: rx,
            _task_handle: task_handle,
        })
    }

    /// Receive the next log entry from the stream
    ///
    /// # Returns
    /// * `Some(LogEntry)` - Next log entry
    /// * `None` - Stream has ended
    pub async fn next(&mut self) -> Option<LogEntry> {
        self.receiver.recv().await
    }
}

/// Internal helper for tailing a single log file
struct LogTailer {
    path: PathBuf,
    source: LogSource,
    reader: Option<BufReader<File>>,
    position: u64,
}

impl LogTailer {
    /// Create a new LogTailer
    async fn new(path: PathBuf, source: LogSource) -> Self {
        Self {
            path,
            source,
            reader: None,
            position: 0,
        }
    }

    /// Read the next log entry
    async fn read_next(&mut self) -> Result<Option<LogEntry>> {
        // Open file if not already open
        if self.reader.is_none() {
            if !self.path.exists() {
                // File doesn't exist yet, wait for it
                return Ok(None);
            }

            let file = File::open(&self.path).await.map_err(|e| {
                AdasaError::LogFileError(format!("Failed to open log file: {}", e))
            })?;

            let mut reader = BufReader::new(file);

            // Seek to the last known position
            reader
                .seek(std::io::SeekFrom::Start(self.position))
                .await
                .map_err(|e| AdasaError::LogError(format!("Failed to seek in log file: {}", e)))?;

            self.reader = Some(reader);
        }

        // Try to read a line
        if let Some(ref mut reader) = self.reader {
            let mut line = String::new();
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    // EOF - no new data
                    Ok(None)
                }
                Ok(n) => {
                    // Update position
                    self.position += n as u64;

                    // Parse and return entry
                    let entry = LogEntry::parse(line.trim_end(), self.source);
                    Ok(Some(entry))
                }
                Err(e) => {
                    // Error reading - file may have been rotated
                    // Reset reader to try reopening
                    self.reader = None;
                    Err(AdasaError::LogError(format!("Failed to read log line: {}", e)))
                }
            }
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn test_read_last_lines_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let log_file = temp_dir.path().join("test.log");

        // Create empty file
        File::create(&log_file).await.unwrap();

        let entries = read_last_lines(&log_file, 10, LogSource::Stdout, None)
            .await
            .unwrap();
        assert_eq!(entries.len(), 0);
    }

    #[tokio::test]
    async fn test_read_last_lines_with_content() {
        let temp_dir = TempDir::new().unwrap();
        let log_file = temp_dir.path().join("test.log");

        // Write test data
        let mut file = File::create(&log_file).await.unwrap();
        file.write_all(b"[2024-01-01 10:00:00.000] Line 1\n")
            .await
            .unwrap();
        file.write_all(b"[2024-01-01 10:00:01.000] Line 2\n")
            .await
            .unwrap();
        file.write_all(b"[2024-01-01 10:00:02.000] Line 3\n")
            .await
            .unwrap();
        file.flush().await.unwrap();
        drop(file);

        let entries = read_last_lines(&log_file, 2, LogSource::Stdout, None)
            .await
            .unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].message.contains("Line 2"));
        assert!(entries[1].message.contains("Line 3"));
    }

    #[tokio::test]
    async fn test_read_last_lines_with_filter() {
        let temp_dir = TempDir::new().unwrap();
        let log_file = temp_dir.path().join("test.log");

        // Write test data
        let mut file = File::create(&log_file).await.unwrap();
        file.write_all(b"[2024-01-01 10:00:00.000] INFO: Starting\n")
            .await
            .unwrap();
        file.write_all(b"[2024-01-01 10:00:01.000] ERROR: Failed\n")
            .await
            .unwrap();
        file.write_all(b"[2024-01-01 10:00:02.000] INFO: Continuing\n")
            .await
            .unwrap();
        file.flush().await.unwrap();
        drop(file);

        let entries = read_last_lines(&log_file, 10, LogSource::Stdout, Some("ERROR"))
            .await
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].message.contains("ERROR"));
    }

    #[tokio::test]
    async fn test_log_entry_parse() {
        let entry = LogEntry::parse(
            "[2024-01-01 10:00:00.000] Test message",
            LogSource::Stdout,
        );
        assert_eq!(entry.timestamp, Some("2024-01-01 10:00:00.000".to_string()));
        assert_eq!(entry.message, "Test message");
        assert_eq!(entry.source, LogSource::Stdout);
    }

    #[tokio::test]
    async fn test_log_entry_parse_no_timestamp() {
        let entry = LogEntry::parse("Plain message", LogSource::Stderr);
        assert_eq!(entry.timestamp, None);
        assert_eq!(entry.message, "Plain message");
        assert_eq!(entry.source, LogSource::Stderr);
    }

    #[tokio::test]
    async fn test_log_entry_format() {
        let entry = LogEntry {
            source: LogSource::Stdout,
            timestamp: Some("2024-01-01 10:00:00.000".to_string()),
            message: "Test".to_string(),
        };

        let formatted = entry.format();
        assert!(formatted.contains("[OUT]"));
        assert!(formatted.contains("2024-01-01 10:00:00.000"));
        assert!(formatted.contains("Test"));
    }

    #[tokio::test]
    async fn test_read_logs_both_sources() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();

        // Create stdout log
        let stdout_file = log_dir.join("test-1-out.log");
        let mut file = File::create(&stdout_file).await.unwrap();
        file.write_all(b"[2024-01-01 10:00:00.000] Stdout line\n")
            .await
            .unwrap();
        file.flush().await.unwrap();
        drop(file);

        // Create stderr log
        let stderr_file = log_dir.join("test-1-err.log");
        let mut file = File::create(&stderr_file).await.unwrap();
        file.write_all(b"[2024-01-01 10:00:01.000] Stderr line\n")
            .await
            .unwrap();
        file.flush().await.unwrap();
        drop(file);

        let options = LogReadOptions::default();
        let entries = read_logs(log_dir, "test", 1, &options).await.unwrap();

        assert_eq!(entries.len(), 2);
        // Should be sorted by timestamp
        assert!(entries[0].message.contains("Stdout"));
        assert!(entries[1].message.contains("Stderr"));
    }

    #[tokio::test]
    async fn test_read_logs_stdout_only() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path();

        // Create stdout log
        let stdout_file = log_dir.join("test-1-out.log");
        let mut file = File::create(&stdout_file).await.unwrap();
        file.write_all(b"[2024-01-01 10:00:00.000] Stdout line\n")
            .await
            .unwrap();
        file.flush().await.unwrap();
        drop(file);

        let options = LogReadOptions {
            lines: 100,
            include_stdout: true,
            include_stderr: false,
            filter: None,
        };
        let entries = read_logs(log_dir, "test", 1, &options).await.unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source, LogSource::Stdout);
    }

    #[tokio::test]
    async fn test_log_stream() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path().to_path_buf();

        // Create log file with initial content
        let log_file = log_dir.join("test-1-out.log");
        let mut file = File::create(&log_file).await.unwrap();
        file.write_all(b"[2024-01-01 10:00:00.000] Initial line\n")
            .await
            .unwrap();
        file.flush().await.unwrap();
        drop(file);

        // Create stream
        let mut stream = LogStream::new(
            log_dir.clone(),
            "test".to_string(),
            1,
            true,
            false,
            None,
        )
        .await
        .unwrap();

        // Append new line to log file
        let mut file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(&log_file)
            .await
            .unwrap();
        file.write_all(b"[2024-01-01 10:00:01.000] New line\n")
            .await
            .unwrap();
        file.flush().await.unwrap();
        drop(file);

        // Read from stream with timeout
        let entry = tokio::time::timeout(Duration::from_secs(2), stream.next())
            .await
            .ok()
            .flatten();

        assert!(entry.is_some());
    }
}
