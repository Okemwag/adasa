use adasa::config::ProcessConfig;
use adasa::process::{ProcessManager, ProcessState};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

fn create_test_config(name: &str) -> ProcessConfig {
    ProcessConfig {
        name: name.to_string(),
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
    }
}

#[tokio::test]
async fn test_graceful_shutdown_with_sigterm() {
    let mut manager = ProcessManager::new();

    // Spawn a process with SIGTERM as stop signal
    let config = create_test_config("sigterm-test");
    let id = manager.spawn(config).await.unwrap();

    // Verify process is running
    let process = manager.get_status(id).unwrap();
    assert_eq!(process.state, ProcessState::Running);
    assert_eq!(process.config.stop_signal, "SIGTERM");

    // Stop gracefully
    let result = manager.stop(id, false).await;
    assert!(result.is_ok());

    // Verify process stopped
    let process = manager.get_status(id).unwrap();
    assert_eq!(process.state, ProcessState::Stopped);
}

#[tokio::test]
async fn test_graceful_shutdown_with_sigint() {
    let mut manager = ProcessManager::new();

    // Spawn a process with SIGINT as stop signal
    let mut config = create_test_config("sigint-test");
    config.stop_signal = "SIGINT".to_string();
    
    let id = manager.spawn(config).await.unwrap();

    // Verify process is running
    let process = manager.get_status(id).unwrap();
    assert_eq!(process.state, ProcessState::Running);
    assert_eq!(process.config.stop_signal, "SIGINT");

    // Stop gracefully
    let result = manager.stop(id, false).await;
    assert!(result.is_ok());

    // Verify process stopped
    let process = manager.get_status(id).unwrap();
    assert_eq!(process.state, ProcessState::Stopped);
}

#[tokio::test]
async fn test_graceful_shutdown_with_custom_timeout() {
    let mut manager = ProcessManager::new();

    // Spawn a process with custom timeout
    let mut config = create_test_config("timeout-test");
    config.stop_timeout_secs = 3;
    
    let id = manager.spawn(config).await.unwrap();

    // Verify process is running
    let process = manager.get_status(id).unwrap();
    assert_eq!(process.state, ProcessState::Running);
    assert_eq!(process.config.stop_timeout_secs, 3);

    // Stop gracefully
    let start = std::time::Instant::now();
    let result = manager.stop(id, false).await;
    let elapsed = start.elapsed();

    assert!(result.is_ok());
    
    // Should complete quickly since sleep responds to SIGTERM
    assert!(elapsed < Duration::from_secs(3));

    // Verify process stopped
    let process = manager.get_status(id).unwrap();
    assert_eq!(process.state, ProcessState::Stopped);
}

#[tokio::test]
async fn test_force_kill_bypasses_graceful_shutdown() {
    let mut manager = ProcessManager::new();

    // Spawn a process with a long timeout
    let mut config = create_test_config("force-kill-test");
    config.stop_timeout_secs = 10; // Long timeout
    
    let id = manager.spawn(config).await.unwrap();

    // Force kill should be immediate
    let start = std::time::Instant::now();
    let result = manager.stop(id, true).await;
    let elapsed = start.elapsed();

    assert!(result.is_ok());
    
    // Should be much faster than the timeout
    assert!(elapsed < Duration::from_secs(1));

    // Verify process stopped
    let process = manager.get_status(id).unwrap();
    assert_eq!(process.state, ProcessState::Stopped);
}

#[tokio::test]
async fn test_stop_all_graceful_shutdown() {
    let mut manager = ProcessManager::new();

    // Spawn multiple processes with different stop signals
    let mut config1 = create_test_config("multi-1");
    config1.stop_signal = "SIGTERM".to_string();
    
    let mut config2 = create_test_config("multi-2");
    config2.stop_signal = "SIGINT".to_string();
    
    let mut config3 = create_test_config("multi-3");
    config3.stop_signal = "SIGHUP".to_string();

    let id1 = manager.spawn(config1).await.unwrap();
    let id2 = manager.spawn(config2).await.unwrap();
    let id3 = manager.spawn(config3).await.unwrap();

    // Verify all are running
    assert_eq!(manager.list().len(), 3);

    // Stop all gracefully
    let result = manager.stop_all().await;
    assert!(result.is_ok());

    // Verify all stopped
    let process1 = manager.get_status(id1).unwrap();
    let process2 = manager.get_status(id2).unwrap();
    let process3 = manager.get_status(id3).unwrap();

    assert_eq!(process1.state, ProcessState::Stopped);
    assert_eq!(process2.state, ProcessState::Stopped);
    assert_eq!(process3.state, ProcessState::Stopped);
}

#[tokio::test]
async fn test_graceful_shutdown_respects_signal_config() {
    let mut manager = ProcessManager::new();

    // Test each supported signal
    let signals = vec![
        "SIGTERM", "SIGINT", "SIGQUIT", "SIGHUP", "SIGUSR1", "SIGUSR2"
    ];

    for signal in signals {
        let mut config = create_test_config(&format!("signal-{}", signal));
        config.stop_signal = signal.to_string();
        
        let id = manager.spawn(config).await.unwrap();
        
        // Verify the signal is configured correctly
        let process = manager.get_status(id).unwrap();
        assert_eq!(process.config.stop_signal, signal);
        
        // Stop gracefully
        let result = manager.stop(id, false).await;
        assert!(result.is_ok(), "Failed to stop with signal {}", signal);
        
        // Verify stopped
        let process = manager.get_status(id).unwrap();
        assert_eq!(process.state, ProcessState::Stopped);
    }
}
