// IPC Server - Listens for client connections and handles requests

use crate::error::{AdasaError, Result};
use crate::ipc::{Command, Request, Response};
use serde_json;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as AsyncBufReader};
use tokio::net::UnixStream as TokioUnixStream;

/// Default socket path for daemon communication
const DEFAULT_SOCKET_PATH: &str = "/tmp/adasa.sock";

/// IPC server for handling client connections
pub struct IpcServer {
    socket_path: PathBuf,
    listener: Option<UnixListener>,
}

impl IpcServer {
    /// Create a new IPC server with the default socket path
    pub fn new() -> Self {
        Self::with_socket_path(DEFAULT_SOCKET_PATH)
    }

    /// Create a new IPC server with a custom socket path
    pub fn with_socket_path<P: AsRef<Path>>(path: P) -> Self {
        Self {
            socket_path: path.as_ref().to_path_buf(),
            listener: None,
        }
    }

    /// Start the IPC server and bind to the Unix socket
    pub fn start(&mut self) -> Result<()> {
        // Remove existing socket file if it exists
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path).map_err(|e| {
                AdasaError::IpcError(format!("Failed to remove existing socket: {}", e))
            })?;
        }

        // Bind to the Unix socket
        let listener = UnixListener::bind(&self.socket_path)
            .map_err(|e| AdasaError::IpcError(format!("Failed to bind to socket: {}", e)))?;

        // Set socket permissions to be accessible only by owner (0600)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&self.socket_path, permissions).map_err(|e| {
                AdasaError::IpcError(format!("Failed to set socket permissions: {}", e))
            })?;
        }

        self.listener = Some(listener);
        Ok(())
    }

    /// Accept a single incoming connection and return the stream
    pub fn accept(&self) -> Result<UnixStream> {
        let listener = self
            .listener
            .as_ref()
            .ok_or_else(|| AdasaError::IpcError("Server not started".to_string()))?;

        let (stream, _addr) = listener
            .accept()
            .map_err(|e| AdasaError::IpcError(format!("Failed to accept connection: {}", e)))?;

        Ok(stream)
    }

    /// Handle a single client connection
    pub fn handle_connection<F>(&self, mut stream: UnixStream, handler: F) -> Result<()>
    where
        F: FnOnce(Command) -> Result<Response>,
    {
        // Read the request from the stream
        let mut reader = BufReader::new(&stream);
        let mut request_line = String::new();
        reader
            .read_line(&mut request_line)
            .map_err(|e| AdasaError::IpcError(format!("Failed to read request: {}", e)))?;

        // Parse the request
        let request: Request = serde_json::from_str(&request_line).map_err(|e| {
            AdasaError::DeserializationError(format!("Failed to deserialize request: {}", e))
        })?;

        // Handle the command
        let response = match handler(request.command) {
            Ok(resp) => resp,
            Err(e) => Response::error(request.id, e.to_string()),
        };

        // Ensure response ID matches request ID
        let response = Response {
            id: request.id,
            result: response.result,
        };

        // Serialize and send the response
        let response_json = serde_json::to_string(&response).map_err(|e| {
            AdasaError::SerializationError(format!("Failed to serialize response: {}", e))
        })?;

        writeln!(stream, "{}", response_json)
            .map_err(|e| AdasaError::IpcError(format!("Failed to write response: {}", e)))?;

        stream
            .flush()
            .map_err(|e| AdasaError::IpcError(format!("Failed to flush stream: {}", e)))?;

        Ok(())
    }

    /// Run the server accept loop with an async handler (optimized)
    pub async fn run<F, Fut>(&self, handler: F) -> Result<()>
    where
        F: Fn(Command) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<Response>> + Send,
    {
        let handler = Arc::new(handler);
        
        // Set listener to non-blocking mode for async operations
        let listener = self
            .listener
            .as_ref()
            .ok_or_else(|| AdasaError::IpcError("Server not started".to_string()))?;
        
        listener.set_nonblocking(true)
            .map_err(|e| AdasaError::IpcError(format!("Failed to set non-blocking: {}", e)))?;

        // Convert to tokio listener
        let tokio_listener = tokio::net::UnixListener::from_std(listener.try_clone()
            .map_err(|e| AdasaError::IpcError(format!("Failed to clone listener: {}", e)))?)
            .map_err(|e| AdasaError::IpcError(format!("Failed to convert listener: {}", e)))?;

        loop {
            // Accept a connection asynchronously
            let (stream, _) = match tokio_listener.accept().await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Failed to accept connection: {}", e);
                    continue;
                }
            };

            // Clone the handler for this connection
            let handler = Arc::clone(&handler);

            // Spawn a task to handle this connection
            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection_async(stream, handler).await {
                    eprintln!("Connection handler error: {}", e);
                }
            });
        }
    }

    /// Handle a single connection asynchronously (optimized for performance)
    async fn handle_connection_async<F, Fut>(
        stream: TokioUnixStream,
        handler: Arc<F>,
    ) -> Result<()>
    where
        F: Fn(Command) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<Response>> + Send,
    {
        // Use async buffered reader for efficient I/O
        let (reader, mut writer) = stream.into_split();
        let mut reader = AsyncBufReader::new(reader);
        
        // Pre-allocate buffer to reduce allocations
        let mut request_line = String::with_capacity(1024);
        
        // Read request line
        reader
            .read_line(&mut request_line)
            .await
            .map_err(|e| AdasaError::IpcError(format!("Failed to read request: {}", e)))?;

        // Parse request (reuse buffer)
        let request: Request = serde_json::from_str(&request_line).map_err(|e| {
            AdasaError::DeserializationError(format!("Failed to deserialize request: {}", e))
        })?;

        // Handle the command
        let response = match handler(request.command).await {
            Ok(resp) => Response {
                id: request.id,
                result: resp.result,
            },
            Err(e) => Response::error(request.id, e.to_string()),
        };

        // Serialize response (use to_vec for better performance)
        let mut response_bytes = serde_json::to_vec(&response).map_err(|e| {
            AdasaError::SerializationError(format!("Failed to serialize response: {}", e))
        })?;
        
        // Add newline
        response_bytes.push(b'\n');

        // Write response asynchronously
        writer
            .write_all(&response_bytes)
            .await
            .map_err(|e| AdasaError::IpcError(format!("Failed to write response: {}", e)))?;

        writer
            .flush()
            .await
            .map_err(|e| AdasaError::IpcError(format!("Failed to flush stream: {}", e)))?;

        Ok(())
    }



    /// Stop the server and clean up the socket file
    pub fn stop(&mut self) -> Result<()> {
        // Drop the listener
        self.listener = None;

        // Remove the socket file
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path).map_err(|e| {
                AdasaError::IpcError(format!("Failed to remove socket file: {}", e))
            })?;
        }

        Ok(())
    }

    /// Get the socket path being used
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}

impl Default for IpcServer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        // Clean up socket file on drop
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_creation() {
        let server = IpcServer::new();
        assert_eq!(server.socket_path(), Path::new(DEFAULT_SOCKET_PATH));
    }

    #[test]
    fn test_server_with_custom_path() {
        let custom_path = "/tmp/test_adasa.sock";
        let server = IpcServer::with_socket_path(custom_path);
        assert_eq!(server.socket_path(), Path::new(custom_path));
    }

    #[test]
    fn test_server_start_stop() {
        let socket_path = "/tmp/test_adasa_start_stop.sock";
        let mut server = IpcServer::with_socket_path(socket_path);

        // Start the server
        assert!(server.start().is_ok());
        assert!(Path::new(socket_path).exists());

        // Stop the server
        assert!(server.stop().is_ok());
        assert!(!Path::new(socket_path).exists());
    }

    #[test]
    fn test_server_cleanup_on_drop() {
        let socket_path = "/tmp/test_adasa_drop.sock";
        {
            let mut server = IpcServer::with_socket_path(socket_path);
            server.start().unwrap();
            assert!(Path::new(socket_path).exists());
        }
        // Server should clean up socket on drop
        assert!(!Path::new(socket_path).exists());
    }
}
