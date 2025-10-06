use adasa::config::{LimitAction, ProcessConfig};
use adasa::process::{ProcessManager, ResourceLimits};
use std::collections::HashMap;
use std::path::PathBuf;

#[tokio::test]
async fn test_resource_limits_creation() {
    let limits = ResourceLimits::new(Some(1024 * 1024 * 100), Some(50));
    
    assert_eq!(limits.max_memory, Some(1024 * 1024 * 100));
    assert_eq!(limits.max_cpu, Some(50));
    assert!(limits.has_cpu_limit());
    assert_eq!(limits.cpu_limit(), Some(50));
}

#[tokio::test]
async fn test_process_config_with_resource_limits() {
    let config = ProcessConfig {
        name: "test-app".to_string(),
        script: PathBuf::from("/bin/echo"),
        args: vec!["hello".to_string()],
        cwd: None,
        env: HashMap::new(),
        instances: 1,
        autorestart: true,
        max_restarts: 10,
        restart_delay_secs: 1,
        max_memory: Some(1024 * 1024 * 512), // 512MB
        max_cpu: Some(75),                    // 75%
        limit_action: LimitAction::Restart,
        stop_signal: "SIGTERM".to_string(),
        stop_timeout_secs: 10,
    };

    // Validate configuration
    assert!(config.validate().is_ok());
    assert_eq!(config.max_memory, Some(1024 * 1024 * 512));
    assert_eq!(config.max_cpu, Some(75));
    assert_eq!(config.limit_action, LimitAction::Restart);
}

#[tokio::test]
async fn test_invalid_cpu_limit() {
    let config = ProcessConfig {
        name: "test-app".to_string(),
        script: PathBuf::from("/bin/echo"),
        args: vec![],
        cwd: None,
        env: HashMap::new(),
        instances: 1,
        autorestart: true,
        max_restarts: 10,
        restart_delay_secs: 1,
        max_memory: None,
        max_cpu: Some(150), // Invalid: > 100
        limit_action: LimitAction::Log,
        stop_signal: "SIGTERM".to_string(),
        stop_timeout_secs: 10,
    };

    // Should fail validation
    assert!(config.validate().is_err());
}

#[tokio::test]
async fn test_zero_cpu_limit() {
    let config = ProcessConfig {
        name: "test-app".to_string(),
        script: PathBuf::from("/bin/echo"),
        args: vec![],
        cwd: None,
        env: HashMap::new(),
        instances: 1,
        autorestart: true,
        max_restarts: 10,
        restart_delay_secs: 1,
        max_memory: None,
        max_cpu: Some(0), // Invalid: must be at least 1
        limit_action: LimitAction::Log,
        stop_signal: "SIGTERM".to_string(),
        stop_timeout_secs: 10,
    };

    // Should fail validation
    assert!(config.validate().is_err());
}

#[tokio::test]
async fn test_limit_action_variants() {
    // Test all limit action variants
    let log_action = LimitAction::Log;
    let restart_action = LimitAction::Restart;
    let stop_action = LimitAction::Stop;

    assert_eq!(log_action, LimitAction::Log);
    assert_eq!(restart_action, LimitAction::Restart);
    assert_eq!(stop_action, LimitAction::Stop);
}

#[tokio::test]
async fn test_process_stats_violation_tracking() {
    use adasa::process::ProcessStats;

    let mut stats = ProcessStats::new(1234);
    
    assert_eq!(stats.memory_violations, 0);
    assert_eq!(stats.cpu_violations, 0);

    stats.record_memory_violation();
    assert_eq!(stats.memory_violations, 1);

    stats.record_cpu_violation();
    assert_eq!(stats.cpu_violations, 1);

    stats.record_memory_violation();
    stats.record_cpu_violation();
    assert_eq!(stats.memory_violations, 2);
    assert_eq!(stats.cpu_violations, 2);
}

#[tokio::test]
async fn test_spawn_process_with_cpu_limit() {
    let mut manager = ProcessManager::new();

    let config = ProcessConfig {
        name: "cpu-limited-test".to_string(),
        script: PathBuf::from("/bin/sleep"),
        args: vec!["5".to_string()],
        cwd: None,
        env: HashMap::new(),
        instances: 1,
        autorestart: false,
        max_restarts: 10,
        restart_delay_secs: 1,
        max_memory: None,
        max_cpu: Some(50), // 50% CPU limit
        limit_action: LimitAction::Log,
        stop_signal: "SIGTERM".to_string(),
        stop_timeout_secs: 2,
    };

    let result = manager.spawn(config).await;
    assert!(result.is_ok());

    let process_id = result.unwrap();
    let process = manager.get_status(process_id).unwrap();

    // Verify cgroup manager was created for CPU limit
    assert!(process.cgroup_manager.is_some());

    // Cleanup
    let _ = manager.stop(process_id, true).await;
}

#[tokio::test]
async fn test_spawn_process_without_cpu_limit() {
    let mut manager = ProcessManager::new();

    let config = ProcessConfig {
        name: "no-cpu-limit-test".to_string(),
        script: PathBuf::from("/bin/sleep"),
        args: vec!["5".to_string()],
        cwd: None,
        env: HashMap::new(),
        instances: 1,
        autorestart: false,
        max_restarts: 10,
        restart_delay_secs: 1,
        max_memory: None,
        max_cpu: None, // No CPU limit
        limit_action: LimitAction::Log,
        stop_signal: "SIGTERM".to_string(),
        stop_timeout_secs: 2,
    };

    let result = manager.spawn(config).await;
    assert!(result.is_ok());

    let process_id = result.unwrap();
    let process = manager.get_status(process_id).unwrap();

    // Verify cgroup manager was NOT created
    assert!(process.cgroup_manager.is_none());

    // Cleanup
    let _ = manager.stop(process_id, true).await;
}
