// Integration test for IPC protocol serialization/deserialization

use adasa::ipc::{Command, ProcessId, ProcessState, Request, Response, ResponseData, StartOptions};
use std::collections::HashMap;
use std::path::PathBuf;

#[test]
fn test_request_serialization() {
    let req = Request::new(1, Command::List);

    // Serialize to JSON
    let json = serde_json::to_string(&req).expect("Failed to serialize request");

    // Deserialize back
    let deserialized: Request = serde_json::from_str(&json).expect("Failed to deserialize request");

    assert_eq!(req.id, deserialized.id);
}

#[test]
fn test_start_command_serialization() {
    let start_opts = StartOptions {
        script: PathBuf::from("/usr/bin/node"),
        name: Some("my-app".to_string()),
        instances: 1,
        env: HashMap::new(),
        cwd: Some(PathBuf::from("/app")),
        args: vec!["server.js".to_string()],
    };

    let req = Request::new(1, Command::Start(start_opts));

    // Serialize and deserialize
    let json = serde_json::to_string(&req).expect("Failed to serialize");
    let _deserialized: Request = serde_json::from_str(&json).expect("Failed to deserialize");
}

#[test]
fn test_response_serialization() {
    let response = Response::success(
        1,
        ResponseData::Started {
            id: ProcessId::new(42),
            name: "test-process".to_string(),
        },
    );

    // Serialize and deserialize
    let json = serde_json::to_string(&response).expect("Failed to serialize");
    let deserialized: Response = serde_json::from_str(&json).expect("Failed to deserialize");

    assert_eq!(response.id, deserialized.id);
    assert!(deserialized.result.is_ok());
}

#[test]
fn test_process_state_display() {
    assert_eq!(ProcessState::Starting.to_string(), "starting");
    assert_eq!(ProcessState::Running.to_string(), "running");
    assert_eq!(ProcessState::Stopping.to_string(), "stopping");
    assert_eq!(ProcessState::Stopped.to_string(), "stopped");
    assert_eq!(ProcessState::Errored.to_string(), "errored");
    assert_eq!(ProcessState::Restarting.to_string(), "restarting");
}

#[test]
fn test_process_id_display() {
    let id = ProcessId::new(123);
    assert_eq!(id.to_string(), "123");
    assert_eq!(id.as_u64(), 123);
}
