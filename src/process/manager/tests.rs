use super::*;
use crate::config::LimitAction;
use std::collections::HashMap;
use std::path::PathBuf;

fn create_test_config(name: &str) -> ProcessConfig {
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
async fn test_process_manager_new() {
    let manager = ProcessManager::new();
    assert_eq!(manager.list().len(), 0);
    assert_eq!(manager.next_id, 1);
}

#[tokio::test]
async fn test_spawn_process() {
    let mut manager = ProcessManager::new();
    let config = create_test_config("test-process");

    let result = manager.spawn(config).await;
    assert!(result.is_ok());

    let id = result.unwrap();
    assert_eq!(id.as_u64(), 1);
    assert_eq!(manager.list().len(), 1);

    let process = manager.get_status(id).unwrap();
    assert_eq!(process.name, "test-process");
    assert_eq!(process.state, ProcessState::Running);

    let _ = manager.stop(id, true).await;
}

#[tokio::test]
async fn test_spawn_duplicate_name() {
    let mut manager = ProcessManager::new();
    let config1 = create_test_config("duplicate");
    let config2 = create_test_config("duplicate");

    let result1 = manager.spawn(config1).await;
    assert!(result1.is_ok());

    let result2 = manager.spawn(config2).await;
    assert!(result2.is_err());
    assert!(matches!(result2, Err(AdasaError::ProcessAlreadyExists(_))));

    let _ = manager.stop(result1.unwrap(), true).await;
}

#[tokio::test]
async fn test_stop_process_graceful() {
    let mut manager = ProcessManager::new();
    let config = create_test_config("test-stop");

    let id = manager.spawn(config).await.unwrap();

    let process = manager.get_status(id).unwrap();
    assert_eq!(process.state, ProcessState::Running);

    let result = manager.stop(id, false).await;
    assert!(result.is_ok());

    let process = manager.get_status(id).unwrap();
    assert_eq!(process.state, ProcessState::Stopped);
}

#[tokio::test]
async fn test_stop_process_force() {
    let mut manager = ProcessManager::new();
    let config = create_test_config("test-force-stop");

    let id = manager.spawn(config).await.unwrap();

    let result = manager.stop(id, true).await;
    assert!(result.is_ok());

    let process = manager.get_status(id).unwrap();
    assert_eq!(process.state, ProcessState::Stopped);
}

#[tokio::test]
async fn test_stop_nonexistent_process() {
    let mut manager = ProcessManager::new();
    let id = ProcessId::new(999);

    let result = manager.stop(id, false).await;
    assert!(result.is_err());
    assert!(matches!(result, Err(AdasaError::ProcessNotFound(_))));
}

#[tokio::test]
async fn test_get_status() {
    let mut manager = ProcessManager::new();
    let config = create_test_config("test-status");

    let id = manager.spawn(config).await.unwrap();

    let status = manager.get_status(id);
    assert!(status.is_some());

    let process = status.unwrap();
    assert_eq!(process.name, "test-status");
    assert_eq!(process.state, ProcessState::Running);
    assert!(process.stats.pid > 0);

    let _ = manager.stop(id, true).await;
}

#[tokio::test]
async fn test_get_status_nonexistent() {
    let manager = ProcessManager::new();
    let id = ProcessId::new(999);

    let status = manager.get_status(id);
    assert!(status.is_none());
}

#[tokio::test]
async fn test_list_processes() {
    let mut manager = ProcessManager::new();

    assert_eq!(manager.list().len(), 0);

    let config1 = create_test_config("process-1");
    let config2 = create_test_config("process-2");
    let config3 = create_test_config("process-3");

    let id1 = manager.spawn(config1).await.unwrap();
    let id2 = manager.spawn(config2).await.unwrap();
    let id3 = manager.spawn(config3).await.unwrap();

    let list = manager.list();
    assert_eq!(list.len(), 3);

    let names: Vec<&str> = list.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"process-1"));
    assert!(names.contains(&"process-2"));
    assert!(names.contains(&"process-3"));

    let _ = manager.stop(id1, true).await;
    let _ = manager.stop(id2, true).await;
    let _ = manager.stop(id3, true).await;
}

#[tokio::test]
async fn test_process_state_transitions() {
    let mut manager = ProcessManager::new();
    let config = create_test_config("test-states");

    let id = manager.spawn(config).await.unwrap();

    let process = manager.get_status(id).unwrap();
    assert_eq!(process.state, ProcessState::Running);

    manager.stop(id, true).await.unwrap();
    let process = manager.get_status(id).unwrap();
    assert_eq!(process.state, ProcessState::Stopped);
}

#[tokio::test]
async fn test_process_stats() {
    let mut manager = ProcessManager::new();
    let config = create_test_config("test-stats");

    let id = manager.spawn(config).await.unwrap();

    let process = manager.get_status(id).unwrap();
    let stats = &process.stats;

    assert!(stats.pid > 0);
    assert_eq!(stats.restarts, 0);
    assert!(stats.last_restart.is_none());

    let uptime = stats.uptime();
    assert!(uptime.as_secs() < 5);

    let _ = manager.stop(id, true).await;
}

#[tokio::test]
async fn test_find_by_name() {
    let mut manager = ProcessManager::new();
    let config = create_test_config("findable");

    let id = manager.spawn(config).await.unwrap();

    let found = manager.find_by_name("findable");
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, id);

    let not_found = manager.find_by_name("nonexistent");
    assert!(not_found.is_none());

    let _ = manager.stop(id, true).await;
}

#[tokio::test]
async fn test_remove_process() {
    let mut manager = ProcessManager::new();
    let config = create_test_config("removable");

    let id = manager.spawn(config).await.unwrap();
    assert_eq!(manager.list().len(), 1);

    manager.stop(id, true).await.unwrap();

    let result = manager.remove(id);
    assert!(result.is_ok());
    assert_eq!(manager.list().len(), 0);
}

#[tokio::test]
async fn test_custom_stop_signal() {
    let mut manager = ProcessManager::new();

    let mut config = create_test_config("custom-signal");
    config.stop_signal = "SIGINT".to_string();
    config.stop_timeout_secs = 2;

    let id = manager.spawn(config).await.unwrap();

    let process = manager.get_status(id).unwrap();
    assert_eq!(process.state, ProcessState::Running);
    assert_eq!(process.config.stop_signal, "SIGINT");

    let result = manager.stop(id, false).await;
    assert!(result.is_ok());

    let process = manager.get_status(id).unwrap();
    assert_eq!(process.state, ProcessState::Stopped);
}

#[tokio::test]
async fn test_stop_with_timeout() {
    let mut manager = ProcessManager::new();

    let mut config = create_test_config("timeout-test");
    config.stop_timeout_secs = 2;

    let id = manager.spawn(config).await.unwrap();

    let result = manager.stop(id, false).await;

    assert!(result.is_ok());

    let process = manager.get_status(id).unwrap();
    assert_eq!(process.state, ProcessState::Stopped);
}

#[tokio::test]
async fn test_force_stop_immediate() {
    let mut manager = ProcessManager::new();
    let config = create_test_config("force-stop");

    let id = manager.spawn(config).await.unwrap();

    let start = std::time::Instant::now();
    let result = manager.stop(id, true).await;
    let elapsed = start.elapsed();

    assert!(result.is_ok());

    assert!(elapsed < Duration::from_millis(500));

    let process = manager.get_status(id).unwrap();
    assert_eq!(process.state, ProcessState::Stopped);
}

#[tokio::test]
async fn test_stop_all_processes() {
    let mut manager = ProcessManager::new();

    let mut ids = Vec::new();
    for i in 0..3 {
        let config = create_test_config(&format!("stop-all-{}", i));
        let id = manager.spawn(config).await.unwrap();
        ids.push(id);
    }

    assert_eq!(manager.list().len(), 3);
    for id in &ids {
        let process = manager.get_status(*id).unwrap();
        assert_eq!(process.state, ProcessState::Running);
    }

    let result = manager.stop_all().await;
    assert!(result.is_ok());

    for id in &ids {
        let process = manager.get_status(*id).unwrap();
        assert_eq!(process.state, ProcessState::Stopped);
    }
}

#[tokio::test]
async fn test_parse_signal_valid() {
    let valid_signals = vec![
        "SIGTERM", "SIGINT", "SIGQUIT", "SIGKILL", "SIGHUP", "SIGUSR1", "SIGUSR2",
    ];

    for signal_name in valid_signals {
        let result = ProcessManager::parse_signal(signal_name);
        assert!(result.is_ok(), "Signal {} should be valid", signal_name);
    }
}

#[tokio::test]
async fn test_parse_signal_invalid() {
    let result = ProcessManager::parse_signal("INVALID");
    assert!(result.is_err());
    assert!(matches!(result, Err(AdasaError::SignalError(_))));
}

#[tokio::test]
async fn test_graceful_shutdown_with_different_signals() {
    let mut manager = ProcessManager::new();

    let signals = vec!["SIGTERM", "SIGINT", "SIGHUP"];

    for signal in signals {
        let mut config = create_test_config(&format!("signal-{}", signal));
        config.stop_signal = signal.to_string();

        let id = manager.spawn(config).await.unwrap();

        let result = manager.stop(id, false).await;
        assert!(result.is_ok(), "Failed to stop with signal {}", signal);

        let process = manager.get_status(id).unwrap();
        assert_eq!(process.state, ProcessState::Stopped);
    }
}
