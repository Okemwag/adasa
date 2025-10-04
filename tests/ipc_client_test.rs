// Integration test for IPC client

use adasa::error::AdasaError;
use adasa::ipc::{Command, IpcClient};

#[test]
fn test_client_daemon_not_running() {
    // Test that client properly handles daemon not running
    let client = IpcClient::with_socket_path("/tmp/test_adasa_nonexistent.sock");
    let result = client.send_command(Command::List);
    
    assert!(result.is_err());
    match result.unwrap_err() {
        AdasaError::DaemonNotRunning => {
            // Expected error
        }
        e => panic!("Expected DaemonNotRunning error, got: {:?}", e),
    }
}

#[test]
fn test_client_socket_path() {
    let custom_path = "/tmp/custom_test.sock";
    let client = IpcClient::with_socket_path(custom_path);
    assert_eq!(client.socket_path().to_str().unwrap(), custom_path);
}
