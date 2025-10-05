use adasa::logs::LogManager;
use std::process::Stdio;
use tempfile::TempDir;
use tokio::process::Command;

#[tokio::test]
async fn test_log_manager_captures_process_output() {
    let temp_dir = TempDir::new().unwrap();
    let log_dir = temp_dir.path();

    // Create log manager
    let mut manager = LogManager::new(log_dir).await.unwrap();

    // Create logger for process
    manager.create_logger(1, "test-process").await.unwrap();

    // Spawn a process that outputs to both stdout and stderr
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg("echo 'stdout message' && echo 'stderr message' >&2")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    // Capture logs
    manager
        .capture_logs(1, "test-process", &mut child)
        .await
        .unwrap();

    // Wait for process to complete
    let _ = child.wait().await;

    // Give time for logs to be written
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Flush logs
    manager.flush_all().await.unwrap();

    // Verify log files exist
    let stdout_log = log_dir.join("test-process-1-out.log");
    let stderr_log = log_dir.join("test-process-1-err.log");

    assert!(stdout_log.exists(), "stdout log file should exist");
    assert!(stderr_log.exists(), "stderr log file should exist");

    // Read and verify log contents
    let stdout_content = tokio::fs::read_to_string(&stdout_log).await.unwrap();
    let stderr_content = tokio::fs::read_to_string(&stderr_log).await.unwrap();

    assert!(
        stdout_content.contains("stdout message"),
        "stdout log should contain the message"
    );
    assert!(
        stderr_content.contains("stderr message"),
        "stderr log should contain the message"
    );

    // Verify timestamps are present
    assert!(
        stdout_content.contains("["),
        "stdout log should have timestamps"
    );
    assert!(
        stderr_content.contains("["),
        "stderr log should have timestamps"
    );
}

#[tokio::test]
async fn test_log_manager_multiple_processes() {
    let temp_dir = TempDir::new().unwrap();
    let log_dir = temp_dir.path();

    let mut manager = LogManager::new(log_dir).await.unwrap();

    // Create loggers for multiple processes
    manager.create_logger(1, "process-1").await.unwrap();
    manager.create_logger(2, "process-2").await.unwrap();

    // Spawn first process
    let mut child1 = Command::new("/bin/echo")
        .arg("Process 1 output")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    manager
        .capture_logs(1, "process-1", &mut child1)
        .await
        .unwrap();

    // Spawn second process
    let mut child2 = Command::new("/bin/echo")
        .arg("Process 2 output")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    manager
        .capture_logs(2, "process-2", &mut child2)
        .await
        .unwrap();

    // Wait for both processes
    let _ = child1.wait().await;
    let _ = child2.wait().await;

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    manager.flush_all().await.unwrap();

    // Verify both log files exist
    assert!(log_dir.join("process-1-1-out.log").exists());
    assert!(log_dir.join("process-2-2-out.log").exists());

    // Verify contents
    let log1 = tokio::fs::read_to_string(log_dir.join("process-1-1-out.log"))
        .await
        .unwrap();
    let log2 = tokio::fs::read_to_string(log_dir.join("process-2-2-out.log"))
        .await
        .unwrap();

    assert!(log1.contains("Process 1 output"));
    assert!(log2.contains("Process 2 output"));
}

#[tokio::test]
async fn test_log_manager_directory_management() {
    let temp_dir = TempDir::new().unwrap();
    let log_dir = temp_dir.path().join("logs");

    // Log directory doesn't exist yet
    assert!(!log_dir.exists());

    // Creating LogManager should create the directory
    let manager = LogManager::new(&log_dir).await.unwrap();

    assert!(log_dir.exists(), "Log directory should be created");
    assert_eq!(manager.log_dir(), log_dir);
}
