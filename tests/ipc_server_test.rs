// Integration tests for IPC server

use adasa::ipc::{
    Command, IpcClient, IpcServer, ProcessId, ProcessInfo, ProcessState, ProcessStats, Response,
    ResponseData,
};
use std::thread;
use std::time::Duration;

#[test]
fn test_server_client_communication() {
    let socket_path = "/tmp/test_adasa_integration.sock";

    // Start server in a separate thread
    let server_socket_path = socket_path.to_string();
    let server_thread = thread::spawn(move || {
        let mut server = IpcServer::with_socket_path(&server_socket_path);
        server.start().expect("Failed to start server");

        // Accept one connection and handle it
        let stream = server.accept().expect("Failed to accept connection");
        server
            .handle_connection(stream, |command| {
                // Simple handler that returns a success response
                match command {
                    Command::List => Ok(Response::success(
                        1,
                        ResponseData::ProcessList(vec![ProcessInfo {
                            id: ProcessId::new(1),
                            name: "test-process".to_string(),
                            state: ProcessState::Running,
                            stats: ProcessStats::default(),
                        }]),
                    )),
                    _ => Ok(Response::success(
                        1,
                        ResponseData::Success("OK".to_string()),
                    )),
                }
            })
            .expect("Failed to handle connection");

        server.stop().expect("Failed to stop server");
    });

    // Give server time to start
    thread::sleep(Duration::from_millis(100));

    // Create client and send command
    let client = IpcClient::with_socket_path(socket_path);
    let response = client
        .send_command(Command::List)
        .expect("Failed to send command");

    // Verify response
    assert!(response.result.is_ok());
    match response.result.unwrap() {
        ResponseData::ProcessList(processes) => {
            assert_eq!(processes.len(), 1);
            assert_eq!(processes[0].name, "test-process");
            assert_eq!(processes[0].state, ProcessState::Running);
        }
        _ => panic!("Expected ProcessList response"),
    }

    // Wait for server thread to finish
    server_thread.join().expect("Server thread panicked");
}

#[test]
fn test_server_error_handling() {
    let socket_path = "/tmp/test_adasa_error.sock";

    // Start server in a separate thread
    let server_socket_path = socket_path.to_string();
    let server_thread = thread::spawn(move || {
        let mut server = IpcServer::with_socket_path(&server_socket_path);
        server.start().expect("Failed to start server");

        // Accept one connection and handle it with an error
        let stream = server.accept().expect("Failed to accept connection");
        server
            .handle_connection(stream, |_command| {
                Err(adasa::error::AdasaError::ProcessNotFound(
                    "test-process".to_string(),
                ))
            })
            .expect("Failed to handle connection");

        server.stop().expect("Failed to stop server");
    });

    // Give server time to start
    thread::sleep(Duration::from_millis(100));

    // Create client and send command
    let client = IpcClient::with_socket_path(socket_path);
    let response = client
        .send_command(Command::List)
        .expect("Failed to send command");

    // Verify error response
    assert!(response.result.is_err());
    let error_msg = response.result.unwrap_err();
    assert!(error_msg.contains("Process not found"));

    // Wait for server thread to finish
    server_thread.join().expect("Server thread panicked");
}

#[test]
fn test_server_multiple_connections() {
    let socket_path = "/tmp/test_adasa_multiple.sock";

    // Start server in a separate thread
    let server_socket_path = socket_path.to_string();
    let server_thread = thread::spawn(move || {
        let mut server = IpcServer::with_socket_path(&server_socket_path);
        server.start().expect("Failed to start server");

        // Handle two connections
        for _ in 0..2 {
            let stream = server.accept().expect("Failed to accept connection");
            server
                .handle_connection(stream, |_command| {
                    Ok(Response::success(
                        1,
                        ResponseData::Success("OK".to_string()),
                    ))
                })
                .expect("Failed to handle connection");
        }

        server.stop().expect("Failed to stop server");
    });

    // Give server time to start
    thread::sleep(Duration::from_millis(100));

    // Create client and send two commands
    let client = IpcClient::with_socket_path(socket_path);

    let response1 = client
        .send_command(Command::List)
        .expect("Failed to send first command");
    assert!(response1.result.is_ok());

    let response2 = client
        .send_command(Command::List)
        .expect("Failed to send second command");
    assert!(response2.result.is_ok());

    // Wait for server thread to finish
    server_thread.join().expect("Server thread panicked");
}
