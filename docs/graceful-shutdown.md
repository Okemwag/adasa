# Graceful Shutdown

This document describes the graceful shutdown functionality in Adasa, which allows processes to terminate cleanly and save their state before exiting.

## Overview

Graceful shutdown is a critical feature for production applications that need to:
- Save state before terminating
- Complete in-flight requests
- Close database connections properly
- Release resources cleanly
- Avoid data corruption

Adasa provides comprehensive graceful shutdown support with:
- Configurable stop signals
- Configurable timeout periods
- Automatic fallback to force kill
- Daemon-level shutdown coordination

## Configuration

### Stop Signal

You can configure which signal is sent to a process when stopping it. The default is `SIGTERM`, but you can use any of the following:

- `SIGTERM` - Standard termination signal (default)
- `SIGINT` - Interrupt signal (Ctrl+C)
- `SIGQUIT` - Quit signal
- `SIGHUP` - Hangup signal
- `SIGUSR1` - User-defined signal 1
- `SIGUSR2` - User-defined signal 2
- `SIGKILL` - Force kill (not recommended as stop signal)

**Example configuration:**

```toml
name = "my-app"
script = "/usr/bin/node"
args = ["server.js"]
stop_signal = "SIGINT"
stop_timeout_secs = 10
```

```json
{
  "name": "my-app",
  "script": "/usr/bin/node",
  "args": ["server.js"],
  "stop_signal": "SIGINT",
  "stop_timeout_secs": 10
}
```

### Stop Timeout

The `stop_timeout_secs` configuration specifies how long to wait for a process to exit gracefully before sending `SIGKILL`. The default is 10 seconds.

**Choosing a timeout:**
- Short-lived processes: 2-5 seconds
- Web servers: 10-30 seconds
- Database connections: 30-60 seconds
- Long-running tasks: 60+ seconds

## How It Works

### Normal Stop Flow

1. **Send Stop Signal**: Adasa sends the configured stop signal (e.g., `SIGTERM`) to the process
2. **Wait for Exit**: Adasa waits for the process to exit gracefully
3. **Monitor Timeout**: If the process doesn't exit within `stop_timeout_secs`, proceed to step 4
4. **Force Kill**: Send `SIGKILL` to forcefully terminate the process
5. **Cleanup**: Mark the process as stopped and clean up resources

### Force Stop Flow

When using the `--force` flag or `force: true` option:

1. **Send SIGKILL**: Immediately send `SIGKILL` to the process
2. **Wait for Exit**: Wait for the process to terminate
3. **Cleanup**: Mark the process as stopped and clean up resources

## CLI Usage

### Stop a Single Process

```bash
# Graceful stop (uses configured signal and timeout)
adasa stop <process-id>

# Force stop (immediate SIGKILL)
adasa stop <process-id> --force
```

### Stop All Processes

```bash
# Gracefully stop all managed processes
adasa stop-all

# Force stop all processes
adasa stop-all --force
```

### Restart with Graceful Shutdown

```bash
# Restart uses graceful stop by default
adasa restart <process-id>

# Rolling restart for multi-instance processes
adasa restart <process-name> --rolling
```

## Daemon Shutdown

When the daemon receives `SIGTERM` or `SIGINT` (Ctrl+C), it performs a graceful shutdown:

1. **Stop Accepting Commands**: The IPC server stops accepting new connections
2. **Stop All Processes**: All managed processes are stopped gracefully using their configured signals and timeouts
3. **Save State**: The daemon state is persisted to disk
4. **Flush Logs**: All log buffers are flushed to disk
5. **Exit**: The daemon exits cleanly

### Daemon Signals

- `SIGTERM`: Graceful shutdown
- `SIGINT` (Ctrl+C): Graceful shutdown
- `SIGKILL`: Immediate termination (not recommended)

## Best Practices

### 1. Handle Signals in Your Application

Ensure your application handles the stop signal properly:

**Node.js Example:**
```javascript
process.on('SIGTERM', async () => {
  console.log('Received SIGTERM, shutting down gracefully...');
  
  // Close server
  await server.close();
  
  // Close database connections
  await db.close();
  
  // Exit
  process.exit(0);
});
```

**Python Example:**
```python
import signal
import sys

def signal_handler(sig, frame):
    print('Received SIGTERM, shutting down gracefully...')
    # Cleanup code here
    sys.exit(0)

signal.signal(signal.SIGTERM, signal_handler)
```

**Rust Example:**
```rust
use tokio::signal;

async fn shutdown_signal() {
    signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C signal handler");
    
    println!("Received shutdown signal, cleaning up...");
    // Cleanup code here
}
```

### 2. Set Appropriate Timeouts

- **Development**: Use shorter timeouts (2-5 seconds) for faster iteration
- **Production**: Use longer timeouts (10-30 seconds) to ensure clean shutdown
- **Critical Systems**: Use very long timeouts (60+ seconds) for systems that must save state

### 3. Test Graceful Shutdown

Always test your application's graceful shutdown behavior:

```bash
# Start your application
adasa start my-app

# Test graceful shutdown
adasa stop my-app

# Check logs to verify clean shutdown
adasa logs my-app
```

### 4. Monitor Shutdown Time

If processes consistently hit the timeout, consider:
- Increasing the timeout value
- Optimizing your shutdown code
- Investigating what's preventing clean shutdown

### 5. Use Different Signals for Different Purposes

- `SIGTERM`: Standard graceful shutdown
- `SIGINT`: Interactive shutdown (Ctrl+C)
- `SIGHUP`: Reload configuration without restart
- `SIGUSR1/SIGUSR2`: Custom application-specific signals

## Troubleshooting

### Process Won't Stop Gracefully

**Symptoms**: Process always hits timeout and requires SIGKILL

**Solutions**:
1. Verify your application handles the stop signal
2. Check application logs for shutdown errors
3. Increase the timeout value
4. Use a different stop signal (try `SIGINT` instead of `SIGTERM`)

### Timeout Too Short

**Symptoms**: Process is killed before completing shutdown

**Solutions**:
1. Increase `stop_timeout_secs` in configuration
2. Optimize shutdown code to complete faster
3. Consider using `--force` for non-critical processes

### Timeout Too Long

**Symptoms**: Shutdown takes too long, blocking other operations

**Solutions**:
1. Decrease `stop_timeout_secs` in configuration
2. Fix hanging shutdown code in your application
3. Use `--force` flag for immediate termination when needed

## Examples

### Example 1: Web Server with 30-Second Timeout

```toml
name = "web-server"
script = "/usr/bin/node"
args = ["server.js"]
stop_signal = "SIGTERM"
stop_timeout_secs = 30
```

This gives the web server 30 seconds to:
- Stop accepting new connections
- Complete in-flight requests
- Close database connections
- Save any pending data

### Example 2: Worker Process with Custom Signal

```toml
name = "worker"
script = "/usr/bin/python"
args = ["worker.py"]
stop_signal = "SIGUSR1"
stop_timeout_secs = 60
```

This uses `SIGUSR1` to trigger a custom shutdown handler in the worker process, with 60 seconds to complete current tasks.

### Example 3: Quick Development Process

```toml
name = "dev-server"
script = "/usr/bin/npm"
args = ["run", "dev"]
stop_signal = "SIGINT"
stop_timeout_secs = 2
```

For development, use a short timeout since clean shutdown is less critical.

## API Reference

### ProcessConfig

```rust
pub struct ProcessConfig {
    // ... other fields ...
    
    /// Signal to send on stop (default: "SIGTERM")
    pub stop_signal: String,
    
    /// Timeout before force kill (in seconds, default: 10)
    pub stop_timeout_secs: u64,
}
```

### ProcessManager

```rust
impl ProcessManager {
    /// Stop a process gracefully or forcefully
    pub async fn stop(&mut self, id: ProcessId, force: bool) -> Result<()>;
    
    /// Stop all managed processes gracefully
    pub async fn stop_all(&mut self) -> Result<()>;
}
```

## See Also

- [Process Management](./process-management.md)
- [Configuration](./configuration.md)
- [Daemon Management](./daemon-management.md)
- [Rolling Restart](./rolling-restart.md)
