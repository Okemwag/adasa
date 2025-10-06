# CLI Output Formatting

This document describes the enhanced CLI output formatting features in Adasa.

## Features

### 1. Formatted Table Output for Process List

The process list is displayed in a clean, rounded table format with the following columns:

- **ID**: Process identifier
- **Name**: Process name (truncated to 20 characters if longer)
- **State**: Current process state with color coding
- **PID**: Operating system process ID
- **CPU**: CPU usage percentage
- **Memory**: Memory usage in human-readable format (B, KB, MB, GB)
- **Uptime**: Process uptime in human-readable format
- **Restarts**: Number of times the process has been restarted

Example:
```
╭────┬──────────────────────┬──────────┬───────┬───────┬─────────┬────────┬──────────╮
│ ID │         Name         │  State   │  PID  │  CPU  │ Memory  │ Uptime │ Restarts │
├────┼──────────────────────┼──────────┼───────┼───────┼─────────┼────────┼──────────┤
│ 1  │ web-server           │ running  │ 12345 │ 2.5%  │ 128.0MB │ 1h 1m  │ 0        │
│ 2  │ background-worker... │ running  │ 12346 │ 15.8% │ 512.0MB │ 2h     │ 3        │
╰────┴──────────────────────┴──────────┴───────┴───────┴─────────┴────────┴──────────╯

Total: 2 process(es)
```

### 2. Color Coding for Process States

Process states are color-coded for quick visual identification:

- **Running**: Green - Process is running normally
- **Starting**: Yellow - Process is starting up
- **Restarting**: Yellow - Process is being restarted
- **Stopping**: Yellow - Process is shutting down
- **Stopped**: Gray - Process has stopped
- **Errored**: Red (bold) - Process encountered an error

### 3. Detailed Status View

Use the `--detailed` flag with the `list` command to see comprehensive information for each process:

```bash
adasa list --detailed
```

Output includes:
- Process ID and name
- Current state (color-coded)
- Operating system PID
- CPU usage percentage
- Memory usage
- Total uptime
- Number of restarts
- Last restart timestamp (if applicable)

Example:
```
Process Details

  ID:             2
  Name:           background-worker
  State:          running
  PID:            12346
  CPU Usage:      15.8%
  Memory:         512.0MB
  Uptime:         2h
  Restarts:       3
  Last Restart:   2025-10-06 12:08:07
```

### 4. Formatted Log Output with Timestamps

Logs are displayed with timestamps for easy tracking:

- Lines with existing timestamps (format: `[YYYY-MM-DD HH:MM:SS]`) are preserved
- Lines without timestamps get automatic timestamps added (format: `[HH:MM:SS]`)

Example:
```
Logs

[2024-01-15 10:30:45] Server started on port 3000
[2024-01-15 10:30:46] Connected to database
[13:08:07] Application log without timestamp
```

### 5. Progress Indicators for Long Operations

Long-running operations (like starting or restarting processes) display an animated spinner:

```
⠋ Processing...
```

When complete, the spinner is replaced with a success or error indicator:

```
✓ Process started successfully
```

or

```
✗ Failed to start process
```

## Human-Readable Formatting

### Duration Formatting

Durations are displayed in the most appropriate unit:

- Less than 60 seconds: `30s`
- Less than 1 hour: `1m 30s`
- Less than 1 day: `1h 1m`
- 1 day or more: `1d 1h`

### Memory Formatting

Memory usage is displayed with appropriate units and precision:

- Bytes: `512B`
- Kilobytes: `2.0KB`
- Megabytes: `128.5MB`
- Gigabytes: `3.25GB`

## Success and Error Messages

All success messages are prefixed with a green checkmark (✓) and formatted in bold:

```
✓ Process started successfully
  ID: 6
  Name: new-service
```

Error messages are prefixed with a red X (✗) and displayed to stderr:

```
✗ Error: Process not found: 999
```

## Usage Examples

### List all processes
```bash
adasa list
```

### List processes with detailed information
```bash
adasa list --detailed
```

### View process logs
```bash
adasa logs 1 --lines 50
```

### Follow logs in real-time
```bash
adasa logs 1 --follow
```

### Start a process (with progress indicator)
```bash
adasa start ./my-app.js --name my-service
```

## Implementation Details

The output formatting uses the following libraries:

- **tabled**: For creating formatted tables with borders
- **colored**: For ANSI color codes and text styling
- **indicatif**: For progress bars and spinners
- **chrono**: For timestamp formatting

All formatting respects terminal capabilities and gracefully degrades when colors are not supported.
