use crate::error::{AdasaError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Process configuration with all settings for managing a process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessConfig {
    /// Process name (unique identifier)
    pub name: String,

    /// Path to the script or executable to run
    pub script: PathBuf,

    /// Command-line arguments
    #[serde(default)]
    pub args: Vec<String>,

    /// Working directory for the process
    #[serde(default)]
    pub cwd: Option<PathBuf>,

    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Number of instances to run
    #[serde(default = "default_instances")]
    pub instances: usize,

    /// Whether to automatically restart on crash
    #[serde(default = "default_autorestart")]
    pub autorestart: bool,

    /// Maximum number of restarts within time window
    #[serde(default = "default_max_restarts")]
    pub max_restarts: usize,

    /// Delay before restart (in seconds)
    #[serde(default = "default_restart_delay")]
    pub restart_delay_secs: u64,

    /// Maximum memory limit in bytes (optional)
    #[serde(default)]
    pub max_memory: Option<u64>,

    /// Signal to send on stop (default: SIGTERM)
    #[serde(default = "default_stop_signal")]
    pub stop_signal: String,

    /// Timeout before force kill (in seconds)
    #[serde(default = "default_stop_timeout")]
    pub stop_timeout_secs: u64,
}

// Default value functions for serde
fn default_instances() -> usize {
    1
}

fn default_autorestart() -> bool {
    true
}

fn default_max_restarts() -> usize {
    10
}

fn default_restart_delay() -> u64 {
    1
}

fn default_stop_signal() -> String {
    "SIGTERM".to_string()
}

fn default_stop_timeout() -> u64 {
    10
}

impl ProcessConfig {
    /// Load process configurations from a file (supports TOML and JSON)
    pub fn from_file(path: &Path) -> Result<Vec<ProcessConfig>> {
        // Read file contents
        let contents = std::fs::read_to_string(path)
            .map_err(|e| AdasaError::ConfigError(format!("Failed to read config file: {}", e)))?;

        // Determine format based on file extension
        let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");

        let configs = match extension {
            "toml" => Self::parse_toml(&contents)?,
            "json" => Self::parse_json(&contents)?,
            _ => {
                return Err(AdasaError::InvalidConfig(format!(
                    "Unsupported file format: {}. Use .toml or .json",
                    extension
                )))
            }
        };

        // Expand environment variables in all configs
        let expanded_configs: Vec<ProcessConfig> = configs
            .into_iter()
            .map(|mut config| {
                config.expand_env_vars();
                config
            })
            .collect();

        // Validate all configs
        for config in &expanded_configs {
            config.validate()?;
        }

        Ok(expanded_configs)
    }

    /// Parse TOML configuration file
    fn parse_toml(contents: &str) -> Result<Vec<ProcessConfig>> {
        #[derive(Deserialize)]
        struct ConfigFile {
            #[serde(default)]
            processes: Vec<ProcessConfig>,
            #[serde(flatten)]
            single: Option<ProcessConfig>,
        }

        let config_file: ConfigFile = toml::from_str(contents)
            .map_err(|e| AdasaError::InvalidConfig(format!("Failed to parse TOML: {}", e)))?;

        // Support both single process and array of processes
        if let Some(single) = config_file.single {
            Ok(vec![single])
        } else if !config_file.processes.is_empty() {
            Ok(config_file.processes)
        } else {
            Err(AdasaError::InvalidConfig(
                "No process configuration found in file".to_string(),
            ))
        }
    }

    /// Parse JSON configuration file
    fn parse_json(contents: &str) -> Result<Vec<ProcessConfig>> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum ConfigFile {
            Single(ProcessConfig),
            Multiple { processes: Vec<ProcessConfig> },
        }

        let config_file: ConfigFile = serde_json::from_str(contents)
            .map_err(|e| AdasaError::InvalidConfig(format!("Failed to parse JSON: {}", e)))?;

        match config_file {
            ConfigFile::Single(config) => Ok(vec![config]),
            ConfigFile::Multiple { processes } => {
                if processes.is_empty() {
                    Err(AdasaError::InvalidConfig(
                        "No process configuration found in file".to_string(),
                    ))
                } else {
                    Ok(processes)
                }
            }
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Validate name
        if self.name.is_empty() {
            return Err(AdasaError::MissingConfigField("name".to_string()));
        }

        // Validate script path
        if self.script.as_os_str().is_empty() {
            return Err(AdasaError::MissingConfigField("script".to_string()));
        }

        // Validate instances
        if self.instances == 0 {
            return Err(AdasaError::ConfigValidationError(
                "instances must be at least 1".to_string(),
            ));
        }

        if self.instances > 100 {
            return Err(AdasaError::ConfigValidationError(
                "instances cannot exceed 100".to_string(),
            ));
        }

        // Validate max_restarts
        if self.max_restarts == 0 {
            return Err(AdasaError::ConfigValidationError(
                "max_restarts must be at least 1".to_string(),
            ));
        }

        // Validate stop_signal
        let valid_signals = [
            "SIGTERM", "SIGINT", "SIGQUIT", "SIGKILL", "SIGHUP", "SIGUSR1", "SIGUSR2",
        ];
        if !valid_signals.contains(&self.stop_signal.as_str()) {
            return Err(AdasaError::ConfigValidationError(format!(
                "Invalid stop_signal: {}. Must be one of: {}",
                self.stop_signal,
                valid_signals.join(", ")
            )));
        }

        // Validate working directory exists if specified
        if let Some(ref cwd) = self.cwd {
            if !cwd.exists() {
                return Err(AdasaError::ConfigValidationError(format!(
                    "Working directory does not exist: {}",
                    cwd.display()
                )));
            }
            if !cwd.is_dir() {
                return Err(AdasaError::ConfigValidationError(format!(
                    "Working directory is not a directory: {}",
                    cwd.display()
                )));
            }
        }

        Ok(())
    }

    /// Expand environment variables in configuration fields
    fn expand_env_vars(&mut self) {
        // Expand in script path
        self.script = Self::expand_env_in_path(&self.script);

        // Expand in working directory
        if let Some(ref cwd) = self.cwd {
            self.cwd = Some(Self::expand_env_in_path(cwd));
        }

        // Expand in arguments
        self.args = self
            .args
            .iter()
            .map(|arg| Self::expand_env_in_string(arg))
            .collect();

        // Expand in environment variables (values only)
        self.env = self
            .env
            .iter()
            .map(|(k, v)| (k.clone(), Self::expand_env_in_string(v)))
            .collect();
    }

    /// Expand environment variables in a string
    fn expand_env_in_string(s: &str) -> String {
        let mut result = s.to_string();

        // Handle $VAR and ${VAR} syntax
        for (key, value) in std::env::vars() {
            result = result.replace(&format!("${{{}}}", key), &value);
            result = result.replace(&format!("${}", key), &value);
        }

        result
    }

    /// Expand environment variables in a path
    fn expand_env_in_path(path: &Path) -> PathBuf {
        let path_str = path.to_string_lossy();
        let expanded = Self::expand_env_in_string(&path_str);
        PathBuf::from(expanded)
    }

    /// Get restart delay as Duration
    pub fn restart_delay(&self) -> Duration {
        Duration::from_secs(self.restart_delay_secs)
    }

    /// Get stop timeout as Duration
    pub fn stop_timeout(&self) -> Duration {
        Duration::from_secs(self.stop_timeout_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_process_config_defaults() {
        let config = ProcessConfig {
            name: "test".to_string(),
            script: PathBuf::from("/bin/echo"),
            args: vec![],
            cwd: None,
            env: HashMap::new(),
            instances: default_instances(),
            autorestart: default_autorestart(),
            max_restarts: default_max_restarts(),
            restart_delay_secs: default_restart_delay(),
            max_memory: None,
            stop_signal: default_stop_signal(),
            stop_timeout_secs: default_stop_timeout(),
        };

        assert_eq!(config.instances, 1);
        assert_eq!(config.autorestart, true);
        assert_eq!(config.max_restarts, 10);
        assert_eq!(config.restart_delay_secs, 1);
        assert_eq!(config.stop_signal, "SIGTERM");
        assert_eq!(config.stop_timeout_secs, 10);
    }

    #[test]
    fn test_validate_valid_config() {
        let config = ProcessConfig {
            name: "test".to_string(),
            script: PathBuf::from("/bin/echo"),
            args: vec![],
            cwd: None,
            env: HashMap::new(),
            instances: 1,
            autorestart: true,
            max_restarts: 10,
            restart_delay_secs: 1,
            max_memory: None,
            stop_signal: "SIGTERM".to_string(),
            stop_timeout_secs: 10,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_empty_name() {
        let config = ProcessConfig {
            name: "".to_string(),
            script: PathBuf::from("/bin/echo"),
            args: vec![],
            cwd: None,
            env: HashMap::new(),
            instances: 1,
            autorestart: true,
            max_restarts: 10,
            restart_delay_secs: 1,
            max_memory: None,
            stop_signal: "SIGTERM".to_string(),
            stop_timeout_secs: 10,
        };

        assert!(matches!(
            config.validate(),
            Err(AdasaError::MissingConfigField(_))
        ));
    }

    #[test]
    fn test_validate_zero_instances() {
        let config = ProcessConfig {
            name: "test".to_string(),
            script: PathBuf::from("/bin/echo"),
            args: vec![],
            cwd: None,
            env: HashMap::new(),
            instances: 0,
            autorestart: true,
            max_restarts: 10,
            restart_delay_secs: 1,
            max_memory: None,
            stop_signal: "SIGTERM".to_string(),
            stop_timeout_secs: 10,
        };

        assert!(matches!(
            config.validate(),
            Err(AdasaError::ConfigValidationError(_))
        ));
    }

    #[test]
    fn test_validate_invalid_signal() {
        let config = ProcessConfig {
            name: "test".to_string(),
            script: PathBuf::from("/bin/echo"),
            args: vec![],
            cwd: None,
            env: HashMap::new(),
            instances: 1,
            autorestart: true,
            max_restarts: 10,
            restart_delay_secs: 1,
            max_memory: None,
            stop_signal: "INVALID".to_string(),
            stop_timeout_secs: 10,
        };

        assert!(matches!(
            config.validate(),
            Err(AdasaError::ConfigValidationError(_))
        ));
    }

    #[test]
    fn test_expand_env_vars() {
        std::env::set_var("TEST_VAR", "test_value");
        std::env::set_var("TEST_PATH", "/tmp");

        let mut config = ProcessConfig {
            name: "test".to_string(),
            script: PathBuf::from("$TEST_PATH/script.sh"),
            args: vec!["--arg=${TEST_VAR}".to_string()],
            cwd: Some(PathBuf::from("${TEST_PATH}")),
            env: {
                let mut map = HashMap::new();
                map.insert("KEY".to_string(), "$TEST_VAR".to_string());
                map
            },
            instances: 1,
            autorestart: true,
            max_restarts: 10,
            restart_delay_secs: 1,
            max_memory: None,
            stop_signal: "SIGTERM".to_string(),
            stop_timeout_secs: 10,
        };

        config.expand_env_vars();

        assert_eq!(config.script, PathBuf::from("/tmp/script.sh"));
        assert_eq!(config.args[0], "--arg=test_value");
        assert_eq!(config.cwd, Some(PathBuf::from("/tmp")));
        assert_eq!(config.env.get("KEY"), Some(&"test_value".to_string()));
    }

    #[test]
    fn test_parse_toml_single() {
        let toml_content = r#"
            name = "my-app"
            script = "/usr/bin/node"
            args = ["server.js"]
            instances = 2
            autorestart = true
        "#;

        let configs = ProcessConfig::parse_toml(toml_content).unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].name, "my-app");
        assert_eq!(configs[0].instances, 2);
    }

    #[test]
    fn test_parse_toml_multiple() {
        let toml_content = r#"
            [[processes]]
            name = "app1"
            script = "/usr/bin/node"
            args = ["server.js"]
            
            [[processes]]
            name = "app2"
            script = "/usr/bin/python"
            args = ["worker.py"]
        "#;

        let configs = ProcessConfig::parse_toml(toml_content).unwrap();
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].name, "app1");
        assert_eq!(configs[1].name, "app2");
    }

    #[test]
    fn test_parse_json_single() {
        let json_content = r#"
            {
                "name": "my-app",
                "script": "/usr/bin/node",
                "args": ["server.js"],
                "instances": 2
            }
        "#;

        let configs = ProcessConfig::parse_json(json_content).unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].name, "my-app");
        assert_eq!(configs[0].instances, 2);
    }

    #[test]
    fn test_parse_json_multiple() {
        let json_content = r#"
            {
                "processes": [
                    {
                        "name": "app1",
                        "script": "/usr/bin/node",
                        "args": ["server.js"]
                    },
                    {
                        "name": "app2",
                        "script": "/usr/bin/python",
                        "args": ["worker.py"]
                    }
                ]
            }
        "#;

        let configs = ProcessConfig::parse_json(json_content).unwrap();
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].name, "app1");
        assert_eq!(configs[1].name, "app2");
    }

    #[test]
    fn test_from_file_toml() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let toml_content = r#"
            name = "test-app"
            script = "/bin/echo"
            args = ["hello"]
        "#;

        fs::write(&config_path, toml_content).unwrap();

        let configs = ProcessConfig::from_file(&config_path).unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].name, "test-app");
    }

    #[test]
    fn test_from_file_json() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        let json_content = r#"
            {
                "name": "test-app",
                "script": "/bin/echo",
                "args": ["hello"]
            }
        "#;

        fs::write(&config_path, json_content).unwrap();

        let configs = ProcessConfig::from_file(&config_path).unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].name, "test-app");
    }

    #[test]
    fn test_from_file_unsupported_format() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yaml");

        fs::write(&config_path, "name: test").unwrap();

        let result = ProcessConfig::from_file(&config_path);
        assert!(matches!(result, Err(AdasaError::InvalidConfig(_))));
    }
}
