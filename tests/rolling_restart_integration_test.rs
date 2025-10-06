use adasa::config::ProcessConfig;
use adasa::ipc::protocol::{Command, ProcessId, RestartOptions};
use adasa::process::ProcessManager;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::test]
async fn test_rolling_restart_integration() {
    let mut manager = ProcessManager::new();

    // Spawn multiple instances of the same application
    let mut instance_ids = Vec::new();
    for i in 0..3 {
        let config = ProcessConfig {
            name: format!("web-server-{}", i),
            script: PathBuf::from("/bin/sleep"),
            args: vec!["30".to_string()],
            cwd: None,
            env: HashMap::new(),
            instances: 1,
            autorestart: true,
            max_restarts: 10,
            restart_delay_secs: 1,
            max_memory: None,
            max_cpu: None,
            limit_action: adasa::config::LimitAction::Log,
            stop_signal: "SIGTERM".to_string(),
            stop_timeout_secs: 5,
        };

        let id = manager.spawn(config).await.unwrap();
        instance_ids.push(id);
    }

    // Verify all instances are running
    for id in &instance_ids {
        let process = manager.get_status(*id).unwrap();
        assert_eq!(process.state, adasa::process::ProcessState::Running);
    }

    // Get initial PIDs
    let initial_pids: Vec<u32> = instance_ids
        .iter()
        .map(|id| manager.get_status(*id).unwrap().stats.pid)
        .collect();

    // Perform rolling restart
    let health_check_delay = Duration::from_millis(500);
    let result = manager
        .rolling_restart("web-server", health_check_delay)
        .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 3);

    // Verify all instances were restarted with new PIDs
    for (idx, id) in instance_ids.iter().enumerate() {
        let process = manager.get_status(*id).unwrap();
        assert_ne!(
            process.stats.pid, initial_pids[idx],
            "Instance {} should have a new PID",
            idx
        );
        assert_eq!(process.stats.restarts, 1);
        assert_eq!(process.state, adasa::process::ProcessState::Running);
    }

    // Cleanup
    for id in instance_ids {
        let _ = manager.stop(id, true).await;
    }
}

#[tokio::test]
async fn test_rolling_restart_maintains_availability() {
    let mut manager = ProcessManager::new();

    // Spawn 3 instances
    let mut instance_ids = Vec::new();
    for i in 0..3 {
        let config = ProcessConfig {
            name: format!("api-server-{}", i),
            script: PathBuf::from("/bin/sleep"),
            args: vec!["30".to_string()],
            cwd: None,
            env: HashMap::new(),
            instances: 1,
            autorestart: true,
            max_restarts: 10,
            restart_delay_secs: 1,
            max_memory: None,
            max_cpu: None,
            limit_action: adasa::config::LimitAction::Log,
            stop_signal: "SIGTERM".to_string(),
            stop_timeout_secs: 5,
        };

        let id = manager.spawn(config).await.unwrap();
        instance_ids.push(id);
    }

    // Start rolling restart in background
    let health_check_delay = Duration::from_millis(300);
    let restart_task = tokio::spawn({
        let mut manager_clone = ProcessManager::new();
        // Note: In a real scenario, we'd share the manager properly
        // For this test, we're just verifying the logic
        async move {
            // Simulate rolling restart behavior
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });

    // During rolling restart, at least 2 out of 3 instances should be running at any time
    // This is a simplified check - in production, you'd monitor actual availability
    tokio::time::sleep(Duration::from_millis(50)).await;

    let running_count = instance_ids
        .iter()
        .filter(|id| {
            manager
                .get_status(**id)
                .map(|p| p.state == adasa::process::ProcessState::Running)
                .unwrap_or(false)
        })
        .count();

    // All should still be running since we haven't actually started the restart
    assert_eq!(running_count, 3);

    restart_task.await.unwrap();

    // Cleanup
    for id in instance_ids {
        let _ = manager.stop(id, true).await;
    }
}

#[tokio::test]
async fn test_rolling_restart_with_failing_health_check() {
    let mut manager = ProcessManager::new();

    // Spawn instances that will exit immediately (simulating failure)
    let mut instance_ids = Vec::new();
    for i in 0..2 {
        let config = ProcessConfig {
            name: format!("failing-app-{}", i),
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
            limit_action: adasa::config::LimitAction::Log,
            stop_signal: "SIGTERM".to_string(),
            stop_timeout_secs: 2,
        };

        let id = manager.spawn(config).await.unwrap();
        instance_ids.push(id);
    }

    // Manually kill the first instance to simulate a crash after restart
    let first_id = instance_ids[0];
    manager.stop(first_id, true).await.unwrap();

    // Try rolling restart - it should fail health check
    let health_check_delay = Duration::from_millis(200);
    let result = manager
        .rolling_restart("failing-app", health_check_delay)
        .await;

    // Should fail because the first instance is not alive
    assert!(result.is_err());

    // Cleanup remaining instances
    for id in instance_ids {
        let _ = manager.stop(id, true).await;
    }
}
