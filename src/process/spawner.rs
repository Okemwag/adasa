use crate::config::ProcessConfig;
use crate::error::{AdasaError, Result};
use std::process::Stdio;
use tokio::process::{Child, Command};

/// Metadata returned when spawning a process
#[derive(Debug)]
pub struct SpawnedProcess {
    /// The child process handle
    pub child: Child,

    /// Process ID assigned by the OS
    pub pid: u32,

    /// Process name from configuration
    pub name: String,
}

/// Spawn a process based on the provided configuration
///
/// This function creates a new process using tokio::process::Command,
/// applying all configuration settings including:
/// - Working directory
/// - Environment variables
/// - Command-line arguments
/// - Stdout/stderr pipe capture
///
/// # Arguments
/// * `config` - Process configuration containing all spawn settings
///
/// # Returns
/// * `Ok(SpawnedProcess)` - Successfully spawned process with metadata
/// * `Err(AdasaError)` - Failed to spawn process
pub async fn spawn_process(config: &ProcessConfig) -> Result<SpawnedProcess> {
    // Validate that the script exists and is executable
    if !config.script.exists() {
        return Err(AdasaError::SpawnError(format!(
            "Script does not exist: {}",
            config.script.display()
        )));
    }

    // Build the command
    let mut command = Command::new(&config.script);

    // Apply command-line arguments
    if !config.args.is_empty() {
        command.args(&config.args);
    }

    // Apply working directory if specified
    if let Some(ref cwd) = config.cwd {
        command.current_dir(cwd);
    }

    // Apply environment variables
    if !config.env.is_empty() {
        for (key, value) in &config.env {
            command.env(key, value);
        }
    }

    // Capture stdout and stderr as pipes for log management
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    // Spawn the process
    let child = command.spawn().map_err(|e| {
        AdasaError::SpawnError(format!("Failed to spawn process '{}': {}", config.name, e))
    })?;

    // Get the process ID
    let pid = child.id().ok_or_else(|| {
        AdasaError::SpawnError(format!("Failed to get PID for process '{}'", config.name))
    })?;

    Ok(SpawnedProcess {
        child,
        pid,
        name: config.name.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_config(name: &str, script: PathBuf) -> ProcessConfig {
        use crate::config::LimitAction;
        ProcessConfig {
            name: name.to_string(),
            script,
            args: vec![],
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
            stop_timeout_secs: 10,
        }
    }

    #[tokio::test]
    async fn test_spawn_simple_process() {
        let config = create_test_config("test-echo", PathBuf::from("/bin/echo"));

        let result = spawn_process(&config).await;
        assert!(result.is_ok());

        let spawned = result.unwrap();
        assert_eq!(spawned.name, "test-echo");
        assert!(spawned.pid > 0);
    }

    #[tokio::test]
    async fn test_spawn_with_args() {
        let mut config = create_test_config("test-echo-args", PathBuf::from("/bin/echo"));
        config.args = vec!["hello".to_string(), "world".to_string()];

        let result = spawn_process(&config).await;
        assert!(result.is_ok());

        let spawned = result.unwrap();
        assert_eq!(spawned.name, "test-echo-args");
    }

    #[tokio::test]
    async fn test_spawn_with_working_directory() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = create_test_config("test-pwd", PathBuf::from("/bin/pwd"));
        config.cwd = Some(temp_dir.path().to_path_buf());

        let result = spawn_process(&config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_spawn_with_env_vars() {
        let mut config = create_test_config("test-env", PathBuf::from("/bin/sh"));
        config.args = vec!["-c".to_string(), "echo $TEST_VAR".to_string()];
        config
            .env
            .insert("TEST_VAR".to_string(), "test_value".to_string());

        let result = spawn_process(&config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_spawn_nonexistent_script() {
        let config = create_test_config("test-nonexistent", PathBuf::from("/nonexistent/script"));

        let result = spawn_process(&config).await;
        assert!(result.is_err());

        match result {
            Err(AdasaError::SpawnError(msg)) => {
                assert!(msg.contains("does not exist"));
            }
            _ => panic!("Expected SpawnError"),
        }
    }

    #[tokio::test]
    async fn test_spawn_captures_stdout_stderr() {
        let config = create_test_config("test-output", PathBuf::from("/bin/echo"));

        let result = spawn_process(&config).await;
        assert!(result.is_ok());

        let spawned = result.unwrap();

        // Verify stdout is captured
        assert!(spawned.child.stdout.is_some());

        // Verify stderr is captured
        assert!(spawned.child.stderr.is_some());
    }

    #[tokio::test]
    async fn test_spawn_invalid_working_directory() {
        let mut config = create_test_config("test-invalid-cwd", PathBuf::from("/bin/echo"));
        config.cwd = Some(PathBuf::from("/nonexistent/directory"));

        let result = spawn_process(&config).await;
        assert!(result.is_err());

        match result {
            Err(AdasaError::SpawnError(_)) => {}
            _ => panic!("Expected SpawnError"),
        }
    }
}
