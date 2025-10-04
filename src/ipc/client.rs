// IPC Client - Communicates with the daemon via Unix socket

use crate::error::{AdasaError, Result};
use crate::ipc::{Command, Request, Response};
use serde_json;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Default socket path for daemon communication
const DEFAULT_SOCKET_PATH: &str = "/tmp/adasa.sock";

/// Maximum number of connection retry attempts
const MAX_RETRY_ATTEMPTS: u32 = 3;

/// Delay between retry attempts
const RETRY_DELAY: Duration = Duration::from_millis(100);

/// IPC client for communicating with the daemon
pub struct IpcClient {
    socket_path: PathBuf,
    request_id: AtomicU64,
}

impl IpcClient {
    /// Create a new IPC client with the default socket path
    pub fn new() -> Self {
        Self::with_socket_path(DEFAULT_SOCKET_PATH)
    }

    /// Create a new IPC client with a custom socket path
    pub fn with_socket_path<P: AsRef<Path>>(path: P) -> Self {
        Self {
            socket_path: path.as_ref().to_path_buf(),
            request_id: AtomicU64::new(1),
        }
    }

    /// Send a command to the daemon and wait for a response
    pub fn send_command(&self, command: Command) -> Result<Response> {
        let request_id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = Request::new(request_id, command);

        // Try to send the command with retry logic
        let mut last_error = None;
        for attempt in 1..=MAX_RETRY_ATTEMPTS {
            match self.try_send_request(&request) {
                Ok(response) => {
                    // Verify response ID matches request ID
                    if response.id != request_id {
                        return Err(AdasaError::ProtocolError(format!(
                            "Response ID mismatch: expected {}, got {}",
                            request_id, response.id
                        )));
                    }
                    return Ok(response);
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < MAX_RETRY_ATTEMPTS {
                        std::thread::sleep(RETRY_DELAY);
                    }
                }
            }
        }

        // All retry attempts failed
        Err(last_error.unwrap_or_else(|| {
            AdasaError::ConnectionError("Failed to connect after retries".to_string())
        }))
    }

    /// Attempt to send a request to the daemon (single attempt)
    fn try_send_request(&self, request: &Request) -> Result<Response> {
        // Connect to the Unix socket
        let mut stream = self.connect()?;

        // Serialize and send the request
        let request_json = serde_json::to_string(request).map_err(|e| {
            AdasaError::SerializationError(format!("Failed to serialize request: {}", e))
        })?;

        // Write request with newline delimiter
        writeln!(stream, "{}", request_json).map_err(|e| {
            AdasaError::IpcError(format!("Failed to write request: {}", e))
        })?;

        // Flush to ensure data is sent
        stream.flush().map_err(|e| {
            AdasaError::IpcError(format!("Failed to flush stream: {}", e))
        })?;

        // Read the response
        let mut reader = BufReader::new(stream);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).map_err(|e| {
            AdasaError::IpcError(format!("Failed to read response: {}", e))
        })?;

        // Deserialize the response
        let response: Response = serde_json::from_str(&response_line).map_err(|e| {
            AdasaError::DeserializationError(format!("Failed to deserialize response: {}", e))
        })?;

        Ok(response)
    }

    /// Establish a connection to the daemon's Unix socket
    fn connect(&self) -> Result<UnixStream> {
        // Check if socket file exists
        if !self.socket_path.exists() {
            return Err(AdasaError::DaemonNotRunning);
        }

        // Attempt to connect
        UnixStream::connect(&self.socket_path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::ConnectionRefused
                || e.kind() == std::io::ErrorKind::NotFound
            {
                AdasaError::DaemonNotRunning
            } else {
                AdasaError::ConnectionError(format!("Failed to connect to daemon: {}", e))
            }
        })
    }

    /// Get the socket path being used
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}

impl Default for IpcClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = IpcClient::new();
        assert_eq!(client.socket_path(), Path::new(DEFAULT_SOCKET_PATH));
    }

    #[test]
    fn test_client_with_custom_path() {
        let custom_path = "/tmp/custom.sock";
        let client = IpcClient::with_socket_path(custom_path);
        assert_eq!(client.socket_path(), Path::new(custom_path));
    }

    #[test]
    fn test_request_id_increment() {
        let client = IpcClient::new();
        let id1 = client.request_id.load(Ordering::SeqCst);
        client.request_id.fetch_add(1, Ordering::SeqCst);
        let id2 = client.request_id.load(Ordering::SeqCst);
        assert_eq!(id2, id1 + 1);
    }

    #[test]
    fn test_daemon_not_running_error() {
        let client = IpcClient::with_socket_path("/tmp/nonexistent.sock");
        let command = Command::List;
        let result = client.send_command(command);
        assert!(result.is_err());
        match result.unwrap_err() {
            AdasaError::DaemonNotRunning => {}
            e => panic!("Expected DaemonNotRunning error, got: {:?}", e),
        }
    }
}
