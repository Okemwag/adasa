use adasa::logs::{LogManager, LogReadOptions};
use std::process::Stdio;
use tempfile::TempDir;
use tokio::process::Command;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_read_logs_integration() {
    let temp_dir = TempDir::new().unwrap();
    let log_dir = temp_dir.path();

    // Create log manager
    let mut log_manager = LogManager::new(log_dir).await.unwrap();

    // Create logger for a test process
    log_manager.create_logger(1, "test-echo").await.unwrap();

    // Spawn a process that outputs to both stdout and stderr
    let mut child = Command::new("sh")
        .arg("-c")
        .arg("echo 'stdout message' && echo 'stderr message' >&2")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    // Capture logs
    log_manager
        .capture_logs(1, "test-echo", &mut child)
        .await
        .unwrap();

    // Wait for process to complete
    let _ = child.wait().await;

    // Give some time for logs to be written
    sleep(Duration::from_millis(200)).await;

    // Flush logs
    log_manager.flush_all().await.unwrap();

    // Read logs
    let options = LogReadOptions {
        lines: 100,
        include_stdout: true,
        include_stderr: true,
        filter: None,
    };

    let entries = log_manager
        .read_logs(1, "test-echo", &options)
        .await
        .unwrap();

    // Verify we got both stdout and stderr entries
    assert!(entries.len() >= 2, "Expected at least 2 log entries");

    let has_stdout = entries.iter().any(|e| e.message.contains("stdout message"));
    let has_stderr = entries.iter().any(|e| e.message.contains("stderr message"));

    assert!(has_stdout, "Should have stdout message");
    assert!(has_stderr, "Should have stderr message");
}

#[tokio::test]
async fn test_read_logs_with_filter() {
    let temp_dir = TempDir::new().unwrap();
    let log_dir = temp_dir.path();

    let mut log_manager = LogManager::new(log_dir).await.unwrap();
    log_manager.create_logger(2, "test-filter").await.unwrap();

    // Spawn a process with multiple log lines
    let mut child = Command::new("sh")
        .arg("-c")
        .arg("echo 'INFO: Starting' && echo 'ERROR: Failed' && echo 'INFO: Continuing'")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    log_manager
        .capture_logs(2, "test-filter", &mut child)
        .await
        .unwrap();

    let _ = child.wait().await;
    sleep(Duration::from_millis(200)).await;
    log_manager.flush_all().await.unwrap();

    // Read logs with filter
    let options = LogReadOptions {
        lines: 100,
        include_stdout: true,
        include_stderr: true,
        filter: Some("ERROR".to_string()),
    };

    let entries = log_manager
        .read_logs(2, "test-filter", &options)
        .await
        .unwrap();

    // Should only get the ERROR line
    assert!(entries.len() >= 1, "Expected at least 1 filtered entry");
    assert!(
        entries.iter().all(|e| e.message.contains("ERROR")),
        "All entries should contain ERROR"
    );
}

#[tokio::test]
async fn test_log_stream_integration() {
    let temp_dir = TempDir::new().unwrap();
    let log_dir = temp_dir.path();

    let mut log_manager = LogManager::new(log_dir).await.unwrap();
    log_manager.create_logger(3, "test-stream").await.unwrap();

    // Spawn a long-running process
    let mut child = Command::new("sh")
        .arg("-c")
        .arg("echo 'Line 1' && sleep 0.1 && echo 'Line 2' && sleep 0.1 && echo 'Line 3'")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    log_manager
        .capture_logs(3, "test-stream", &mut child)
        .await
        .unwrap();

    // Create log stream
    let mut stream = log_manager
        .stream_logs(3, "test-stream", true, false, None)
        .await
        .unwrap();

    // Read entries from stream with timeout
    let mut entries = Vec::new();
    for _ in 0..3 {
        if let Ok(Some(entry)) = tokio::time::timeout(Duration::from_secs(2), stream.next()).await {
            entries.push(entry);
        }
    }

    // Wait for process to complete
    let _ = child.wait().await;

    // Should have received at least some entries
    assert!(
        !entries.is_empty(),
        "Should have received at least one log entry from stream"
    );
}

#[tokio::test]
async fn test_read_logs_stdout_only() {
    let temp_dir = TempDir::new().unwrap();
    let log_dir = temp_dir.path();

    let mut log_manager = LogManager::new(log_dir).await.unwrap();
    log_manager.create_logger(4, "test-stdout").await.unwrap();

    let mut child = Command::new("sh")
        .arg("-c")
        .arg("echo 'stdout only' && echo 'stderr message' >&2")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    log_manager
        .capture_logs(4, "test-stdout", &mut child)
        .await
        .unwrap();

    let _ = child.wait().await;
    sleep(Duration::from_millis(200)).await;
    log_manager.flush_all().await.unwrap();

    // Read only stdout logs
    let options = LogReadOptions {
        lines: 100,
        include_stdout: true,
        include_stderr: false,
        filter: None,
    };

    let entries = log_manager
        .read_logs(4, "test-stdout", &options)
        .await
        .unwrap();

    // Should only have stdout entries
    assert!(
        entries.iter().all(|e| e.message.contains("stdout")),
        "Should only contain stdout messages"
    );
}

#[tokio::test]
async fn test_read_logs_last_n_lines() {
    let temp_dir = TempDir::new().unwrap();
    let log_dir = temp_dir.path();

    let mut log_manager = LogManager::new(log_dir).await.unwrap();
    log_manager.create_logger(5, "test-lines").await.unwrap();

    // Generate multiple log lines
    let mut child = Command::new("sh")
        .arg("-c")
        .arg("for i in 1 2 3 4 5 6 7 8 9 10; do echo \"Line $i\"; done")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    log_manager
        .capture_logs(5, "test-lines", &mut child)
        .await
        .unwrap();

    let _ = child.wait().await;
    sleep(Duration::from_millis(200)).await;
    log_manager.flush_all().await.unwrap();

    // Read only last 3 lines
    let options = LogReadOptions {
        lines: 3,
        include_stdout: true,
        include_stderr: false,
        filter: None,
    };

    let entries = log_manager
        .read_logs(5, "test-lines", &options)
        .await
        .unwrap();

    // Should have at most 3 entries
    assert!(
        entries.len() <= 3,
        "Should have at most 3 entries, got {}",
        entries.len()
    );

    // Should contain the last lines
    if !entries.is_empty() {
        let last_entry = &entries[entries.len() - 1];
        assert!(
            last_entry.message.contains("Line 10"),
            "Last entry should be Line 10"
        );
    }
}
