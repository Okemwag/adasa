// Integration test for configuration file support

use adasa::config::ProcessConfig;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_load_toml_config_single_process() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let toml_content = r#"
        name = "test-app"
        script = "/bin/echo"
        args = ["hello", "world"]
        instances = 2
        autorestart = true
        max_restarts = 5
        restart_delay_secs = 2
        stop_signal = "SIGTERM"
        stop_timeout_secs = 15
    "#;

    fs::write(&config_path, toml_content).unwrap();

    let configs = ProcessConfig::from_file(&config_path).unwrap();
    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].name, "test-app");
    assert_eq!(configs[0].script, PathBuf::from("/bin/echo"));
    assert_eq!(configs[0].args, vec!["hello", "world"]);
    assert_eq!(configs[0].instances, 2);
    assert_eq!(configs[0].autorestart, true);
    assert_eq!(configs[0].max_restarts, 5);
    assert_eq!(configs[0].restart_delay_secs, 2);
    assert_eq!(configs[0].stop_signal, "SIGTERM");
    assert_eq!(configs[0].stop_timeout_secs, 15);
}

#[test]
fn test_load_toml_config_multiple_processes() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let toml_content = r#"
        [[processes]]
        name = "web-server"
        script = "/usr/bin/node"
        args = ["server.js"]
        instances = 4
        autorestart = true

        [[processes]]
        name = "worker"
        script = "/usr/bin/python3"
        args = ["worker.py"]
        instances = 2
        autorestart = true
        max_memory = 536870912
    "#;

    fs::write(&config_path, toml_content).unwrap();

    let configs = ProcessConfig::from_file(&config_path).unwrap();
    assert_eq!(configs.len(), 2);
    
    assert_eq!(configs[0].name, "web-server");
    assert_eq!(configs[0].instances, 4);
    
    assert_eq!(configs[1].name, "worker");
    assert_eq!(configs[1].instances, 2);
    assert_eq!(configs[1].max_memory, Some(536870912));
}

#[test]
fn test_load_json_config_single_process() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");

    let json_content = r#"
        {
            "name": "api-server",
            "script": "/usr/bin/node",
            "args": ["api.js"],
            "instances": 3,
            "autorestart": true,
            "max_restarts": 10,
            "env": {
                "NODE_ENV": "production",
                "PORT": "8080"
            }
        }
    "#;

    fs::write(&config_path, json_content).unwrap();

    let configs = ProcessConfig::from_file(&config_path).unwrap();
    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].name, "api-server");
    assert_eq!(configs[0].instances, 3);
    assert_eq!(configs[0].env.get("NODE_ENV"), Some(&"production".to_string()));
    assert_eq!(configs[0].env.get("PORT"), Some(&"8080".to_string()));
}

#[test]
fn test_load_json_config_multiple_processes() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");

    let json_content = r#"
        {
            "processes": [
                {
                    "name": "app1",
                    "script": "/usr/bin/node",
                    "args": ["app1.js"],
                    "instances": 2
                },
                {
                    "name": "app2",
                    "script": "/usr/bin/python3",
                    "args": ["app2.py"],
                    "instances": 1,
                    "max_cpu": 50
                }
            ]
        }
    "#;

    fs::write(&config_path, json_content).unwrap();

    let configs = ProcessConfig::from_file(&config_path).unwrap();
    assert_eq!(configs.len(), 2);
    
    assert_eq!(configs[0].name, "app1");
    assert_eq!(configs[0].instances, 2);
    
    assert_eq!(configs[1].name, "app2");
    assert_eq!(configs[1].instances, 1);
    assert_eq!(configs[1].max_cpu, Some(50));
}

#[test]
fn test_config_validation_empty_name() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");

    let json_content = r#"
        {
            "name": "",
            "script": "/bin/echo"
        }
    "#;

    fs::write(&config_path, json_content).unwrap();

    let result = ProcessConfig::from_file(&config_path);
    assert!(result.is_err());
}

#[test]
fn test_config_validation_zero_instances() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");

    let json_content = r#"
        {
            "name": "test",
            "script": "/bin/echo",
            "instances": 0
        }
    "#;

    fs::write(&config_path, json_content).unwrap();

    let result = ProcessConfig::from_file(&config_path);
    assert!(result.is_err());
}

#[test]
fn test_config_validation_invalid_signal() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");

    let json_content = r#"
        {
            "name": "test",
            "script": "/bin/echo",
            "stop_signal": "INVALID_SIGNAL"
        }
    "#;

    fs::write(&config_path, json_content).unwrap();

    let result = ProcessConfig::from_file(&config_path);
    assert!(result.is_err());
}

#[test]
fn test_config_with_working_directory() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");
    let work_dir = temp_dir.path().join("workdir");
    fs::create_dir(&work_dir).unwrap();

    let json_content = format!(
        r#"
        {{
            "name": "test",
            "script": "/bin/echo",
            "cwd": "{}"
        }}
        "#,
        work_dir.display()
    );

    fs::write(&config_path, json_content).unwrap();

    let configs = ProcessConfig::from_file(&config_path).unwrap();
    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].cwd, Some(work_dir));
}

#[test]
fn test_config_with_environment_variables() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let toml_content = r#"
        name = "test"
        script = "/bin/echo"
        
        [env]
        VAR1 = "value1"
        VAR2 = "value2"
        VAR3 = "value3"
    "#;

    fs::write(&config_path, toml_content).unwrap();

    let configs = ProcessConfig::from_file(&config_path).unwrap();
    assert_eq!(configs.len(), 1);
    
    let env = &configs[0].env;
    assert_eq!(env.get("VAR1"), Some(&"value1".to_string()));
    assert_eq!(env.get("VAR2"), Some(&"value2".to_string()));
    assert_eq!(env.get("VAR3"), Some(&"value3".to_string()));
}

#[test]
fn test_config_with_resource_limits() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");

    let json_content = r#"
        {
            "name": "limited-app",
            "script": "/bin/echo",
            "max_memory": 268435456,
            "max_cpu": 75,
            "limit_action": "restart"
        }
    "#;

    fs::write(&config_path, json_content).unwrap();

    let configs = ProcessConfig::from_file(&config_path).unwrap();
    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].max_memory, Some(268435456));
    assert_eq!(configs[0].max_cpu, Some(75));
    assert_eq!(configs[0].limit_action, adasa::config::LimitAction::Restart);
}

#[test]
fn test_unsupported_file_format() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.yaml");

    fs::write(&config_path, "name: test").unwrap();

    let result = ProcessConfig::from_file(&config_path);
    assert!(result.is_err());
}

#[test]
fn test_config_defaults() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");

    // Minimal config with only required fields
    let json_content = r#"
        {
            "name": "minimal",
            "script": "/bin/echo"
        }
    "#;

    fs::write(&config_path, json_content).unwrap();

    let configs = ProcessConfig::from_file(&config_path).unwrap();
    assert_eq!(configs.len(), 1);
    
    let config = &configs[0];
    assert_eq!(config.instances, 1);
    assert_eq!(config.autorestart, true);
    assert_eq!(config.max_restarts, 10);
    assert_eq!(config.restart_delay_secs, 1);
    assert_eq!(config.stop_signal, "SIGTERM");
    assert_eq!(config.stop_timeout_secs, 10);
    assert_eq!(config.limit_action, adasa::config::LimitAction::Log);
}

#[test]
fn test_env_var_expansion() {
    std::env::set_var("TEST_SCRIPT_PATH", "/usr/bin/node");
    std::env::set_var("TEST_PORT", "3000");

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");

    let json_content = r#"
        {
            "name": "test",
            "script": "$TEST_SCRIPT_PATH",
            "args": ["--port=${TEST_PORT}"],
            "env": {
                "PORT": "$TEST_PORT"
            }
        }
    "#;

    fs::write(&config_path, json_content).unwrap();

    let configs = ProcessConfig::from_file(&config_path).unwrap();
    assert_eq!(configs.len(), 1);
    
    let config = &configs[0];
    assert_eq!(config.script, PathBuf::from("/usr/bin/node"));
    assert_eq!(config.args[0], "--port=3000");
    assert_eq!(config.env.get("PORT"), Some(&"3000".to_string()));
}
