# Daemon Management

This document describes how to manage the Adasa daemon process.

## Overview

The Adasa daemon is a background process that manages all your application processes. It must be running for process management commands to work.

## Commands

### Start the Daemon

Start the daemon in the background:

```bash
adasa daemon start
```

This command will:
- Check if the daemon is already running
- Spawn the daemon process in the background (daemonized)
- Write the daemon PID to `/tmp/adasa.pid`
- Create a Unix socket at `/tmp/adasa.sock` for IPC

### Stop the Daemon

Stop the running daemon:

```bash
adasa daemon stop
```

This command will:
- Send SIGTERM to the daemon process
- Wait up to 10 seconds for graceful shutdown
- Send SIGKILL if the daemon doesn't stop gracefully
- Remove the PID file

### Check Daemon Status

Check if the daemon is running:

```bash
adasa daemon status
```

This command will display:
- Whether the daemon is running
- The daemon's PID (if running)
- The location of the PID file

## PID File Management

The daemon uses a PID file to track its process ID. By default, the PID file is located at `/tmp/adasa.pid`.

### PID File Features

- **Automatic Creation**: Created when the daemon starts
- **Automatic Cleanup**: Removed when the daemon stops gracefully
- **Stale Detection**: The daemon checks if the PID in the file is actually running
- **Permissions**: The PID file is only readable/writable by the owner

### Manual PID File Cleanup

If the daemon crashes or is killed forcefully, the PID file may become stale. You can manually remove it:

```bash
rm /tmp/adasa.pid
```

## Daemonization

When the daemon starts, it performs the following daemonization steps (Unix only):

1. **First Fork**: Creates a child process and exits the parent
2. **Session Leader**: The child becomes a session leader (setsid)
3. **Second Fork**: Creates another child to prevent acquiring a controlling terminal
4. **Change Directory**: Changes working directory to `/` to avoid keeping directories in use
5. **Redirect I/O**: Redirects stdin, stdout, and stderr to `/dev/null`

This ensures the daemon runs completely in the background without any terminal attachment.

## Examples

### Basic Workflow

```bash
# Start the daemon
adasa daemon start

# Check status
adasa daemon status

# Use the daemon to manage processes
adasa start /path/to/app --name myapp

# Stop the daemon (this will also stop all managed processes)
adasa daemon stop
```

### Troubleshooting

If you encounter issues:

1. **Check if daemon is running**:
   ```bash
   adasa daemon status
   ```

2. **Check for stale PID file**:
   ```bash
   cat /tmp/adasa.pid
   ps -p $(cat /tmp/adasa.pid)  # Check if process exists
   ```

3. **Clean up and restart**:
   ```bash
   rm /tmp/adasa.pid /tmp/adasa.sock
   adasa daemon start
   ```

## Platform Support

- **Linux**: Full support with daemonization
- **macOS**: Full support with daemonization
- **Windows**: Not yet supported (daemonization is Unix-specific)

## Security Considerations

- The PID file and Unix socket are created with restrictive permissions (owner only)
- The daemon runs with the same user permissions as the user who started it
- Managed processes inherit the daemon's user permissions
