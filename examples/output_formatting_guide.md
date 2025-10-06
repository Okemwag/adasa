# CLI Output Formatting Guide

This guide demonstrates the enhanced CLI output formatting features implemented in Task 25.

## Features Implemented

### ✅ 1. Formatted Table Output for Process List

The `adasa list` command now displays processes in a beautiful rounded table:

```
╭────┬──────────────────────┬──────────┬───────┬───────┬─────────┬────────┬──────────╮
│ ID │         Name         │  State   │  PID  │  CPU  │ Memory  │ Uptime │ Restarts │
├────┼──────────────────────┼──────────┼───────┼───────┼─────────┼────────┼──────────┤
│ 1  │ web-server           │ running  │ 12345 │ 2.5%  │ 128.0MB │ 1h 1m  │ 0        │
│ 2  │ background-worker... │ running  │ 12346 │ 15.8% │ 512.0MB │ 2h     │ 3        │
╰────┴──────────────────────┴──────────┴───────┴───────┴─────────┴────────┴──────────╯
```

**Implementation:**
- Uses `tabled` crate for professional table rendering
- Rounded borders for modern appearance
- Centered column headers
- Automatic column width adjustment
- Long names are truncated with ellipsis (...)

### ✅ 2. Color Coding for Process States

Process states are color-coded for instant visual feedback:

| State | Color | Meaning |
|-------|-------|---------|
| **running** | 🟢 Green | Process is healthy and running |
| **starting** | 🟡 Yellow | Process is starting up |
| **restarting** | 🟡 Yellow | Process is being restarted |
| **stopping** | 🟡 Yellow | Process is shutting down |
| **stopped** | ⚪ Gray | Process has stopped |
| **errored** | 🔴 Red (bold) | Process encountered an error |

**Implementation:**
- Uses `colored` crate for ANSI color codes
- Bold text for error states to draw attention
- Graceful degradation when colors aren't supported

### ✅ 3. Detailed Status View

Use `adasa list --detailed` to see comprehensive information:

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

**Features:**
- Clean, aligned layout
- Formatted timestamps for last restart
- All metrics in human-readable format
- Color-coded state indicator

### ✅ 4. Formatted Log Output with Timestamps

Logs are displayed with automatic timestamp formatting:

```
Logs

[2024-01-15 10:30:45] Server started on port 3000
[2024-01-15 10:30:46] Connected to database
[13:08:07] Application log without timestamp
```

**Implementation:**
- Preserves existing timestamps in logs
- Adds timestamps to lines without them
- Uses `chrono` for timestamp formatting
- Dimmed timestamp text for better readability

### ✅ 5. Progress Indicators for Long Operations

Long-running operations show an animated spinner:

```
⠋ Processing...
```

On completion:
```
✓ Process started successfully
```

**Implementation:**
- Uses `indicatif` crate for smooth animations
- 10 different spinner frames for fluid animation
- Green checkmark (✓) for success
- Red X (✗) for errors
- Automatic cleanup when operation completes

## Human-Readable Formatting

### Duration Formatting

Smart duration display that uses the most appropriate unit:

| Duration | Display |
|----------|---------|
| 30 seconds | `30s` |
| 90 seconds | `1m 30s` |
| 3700 seconds | `1h 1m` |
| 90000 seconds | `1d 1h` |

### Memory Formatting

Memory usage with appropriate units and precision:

| Bytes | Display |
|-------|---------|
| 512 | `512B` |
| 2048 | `2.0KB` |
| 2097152 | `2.0MB` |
| 3221225472 | `3.00GB` |

## Success and Error Messages

### Success Messages
All success messages use consistent formatting:
- Green checkmark (✓) prefix
- Bold text for emphasis
- Structured information display

Example:
```
✓ Process started successfully
  ID: 6
  Name: new-service
```

### Error Messages
Error messages are clear and actionable:
- Red X (✗) prefix
- Sent to stderr (not stdout)
- Clear error description

Example:
```
✗ Error: Process not found: 999
```

## Testing

Run the demo to see all features in action:

```bash
cargo run --example output_demo
```

This demonstrates:
1. Process list table
2. Detailed status view
3. Success messages
4. Error messages
5. Daemon status
6. Log output
7. Progress indicators

## Requirements Satisfied

This implementation satisfies the following requirements:

- ✅ **Requirement 3.1**: Display process ID, name, status, uptime, and restart count
- ✅ **Requirement 3.2**: Include CPU usage, memory usage, and process state in detailed view
- ✅ **Requirement 3.3**: Show all managed processes in a formatted table
- ✅ **Requirement 11.4**: Use clear formatting and colors

## Dependencies Added

```toml
tabled = "0.16"      # Professional table formatting
colored = "2.1"      # ANSI color codes and text styling
indicatif = "0.17"   # Progress bars and spinners
```

## Code Organization

The output formatting is centralized in `src/cli/output.rs`:

- `print_success()` - Main success output handler
- `print_error()` - Error message handler
- `print_process_table()` - Formatted table for process list
- `print_detailed_status()` - Detailed single process view
- `print_logs()` - Formatted log output
- `create_progress_bar()` - Progress indicator creation
- `finish_progress_success()` - Success completion
- `finish_progress_error()` - Error completion
- Helper functions for formatting durations, memory, etc.

## Future Enhancements

Potential improvements for future tasks:
- JSON output format for scripting (`--json` flag)
- Custom color themes
- Configurable table styles
- Export to CSV/HTML
- Real-time updates in list view
- Interactive TUI mode
