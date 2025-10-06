# Rolling Restart Feature

## Overview

The rolling restart feature allows you to restart multiple instances of an application sequentially while maintaining service availability. This is particularly useful for zero-downtime deployments and updates.

## How It Works

When you perform a rolling restart:

1. **Sequential Restart**: Each instance is restarted one at a time, not all at once
2. **Health Checks**: After restarting each instance, the system waits for a configurable health check delay
3. **Availability**: Other instances continue running while one is being restarted
4. **Fail-Safe**: If an instance fails its health check, the rolling restart stops immediately

## Usage

### CLI Command

```bash
# Restart all instances of a process with rolling restart
adasa restart <process-name-or-id> --rolling

# Examples:
adasa restart web-server --rolling
adasa restart 1 --rolling
```

### Configuration

The health check delay is currently set to 3 seconds by default. This gives each restarted instance time to initialize before the next instance is restarted.

## Example Scenario

Suppose you have 3 instances of a web server running:
- web-server-0 (PID: 1001)
- web-server-1 (PID: 1002)
- web-server-2 (PID: 1003)

When you run `adasa restart web-server --rolling`:

1. **Instance 0** is restarted (new PID: 2001)
2. System waits 3 seconds and verifies instance 0 is healthy
3. **Instance 1** is restarted (new PID: 2002)
4. System waits 3 seconds and verifies instance 1 is healthy
5. **Instance 2** is restarted (new PID: 2003)
6. All instances are now running with new PIDs

During this process, at least 2 out of 3 instances are always running, maintaining service availability.

## Benefits

- **Zero Downtime**: Service remains available during restarts
- **Safe Updates**: Failed health checks prevent cascading failures
- **Controlled Rollout**: Updates are applied gradually
- **Easy Rollback**: If an instance fails, you can stop the rollout immediately

## Implementation Details

### Health Check

The health check verifies that a restarted process is still alive after the configured delay. If the process has crashed or stopped, the rolling restart fails immediately.

### Process Selection

Rolling restart works with:
- **Process ID**: Finds all instances with the same base name
- **Process Name**: Finds all instances matching the name pattern

For example, if you have processes named `api-0`, `api-1`, `api-2`, you can restart them all with:
```bash
adasa restart api --rolling
```

### Error Handling

If a health check fails during rolling restart:
- The restart process stops immediately
- Already restarted instances remain running
- An error is returned indicating which instance failed
- No further instances are restarted

## Code Example

```rust
use adasa::process::ProcessManager;
use std::time::Duration;

// Perform rolling restart with 3-second health check delay
let health_check_delay = Duration::from_secs(3);
let count = manager.rolling_restart("web-server", health_check_delay).await?;
println!("Restarted {} instances", count);
```

## Testing

The rolling restart feature includes comprehensive tests:
- Unit tests for single and multiple instances
- Integration tests for end-to-end functionality
- Health check verification tests
- Failure scenario tests

Run tests with:
```bash
cargo test rolling_restart
```

## Future Enhancements

Potential improvements for future versions:
- Configurable health check delay via CLI
- Custom health check commands
- Parallel restart with configurable batch size
- Progress reporting during long rolling restarts
- Automatic rollback on failure
