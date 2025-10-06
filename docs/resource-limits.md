# Resource Limits

Adasa supports setting resource limits on managed processes to prevent any single process from consuming all system resources.

## Supported Limits

### Memory Limits

Memory limits are enforced using OS-level mechanisms (`setrlimit` on Unix systems). When a process exceeds its memory limit, the operating system will typically terminate it with a SIGKILL signal.

**Configuration:**
```toml
[[processes]]
name = "my-app"
script = "/usr/bin/node"
args = ["server.js"]

# Set memory limit to 512MB (in bytes)
max_memory = 536870912  # 512 * 1024 * 1024
```

### CPU Limits

CPU limits are enforced using cgroups v2 on Linux systems. The limit is specified as a percentage of a single CPU core (1-100%).

**Configuration:**
```toml
[[processes]]
name = "my-worker"
script = "/usr/bin/python3"
args = ["worker.py"]

# Limit to 50% of one CPU core
max_cpu = 50
```

**Note:** CPU throttling requires:
- Linux operating system
- cgroups v2 enabled
- Sufficient permissions to create cgroups (typically requires root or appropriate capabilities)

On non-Linux systems or when cgroups are not available, CPU limits will be logged as warnings but not enforced.

## Limit Actions

When a process exceeds its resource limits, you can configure what action Adasa should take:

### Log (Default)

Just log the violation but continue running the process:

```toml
limit_action = "log"
```

### Restart

Automatically restart the process when it exceeds limits:

```toml
limit_action = "restart"
```

This is useful for processes that may have memory leaks or gradually increase resource usage over time.

### Stop

Stop the process when it exceeds limits:

```toml
limit_action = "stop"
```

This is useful for strict resource enforcement where you want to prevent runaway processes.

## Complete Example

```toml
[[processes]]
name = "web-server"
script = "/usr/bin/node"
args = ["server.js"]
instances = 4

# Resource limits
max_memory = 1073741824  # 1GB
max_cpu = 75             # 75% of one core

# Restart on limit violations
limit_action = "restart"

# Other settings
autorestart = true
max_restarts = 10
```

## Monitoring Resource Usage

Resource usage is tracked and displayed in the process status:

```bash
adasa list
```

Output includes:
- Current CPU usage percentage
- Current memory usage in bytes
- Number of memory limit violations
- Number of CPU limit violations

## Implementation Details

### Memory Limits

Memory limits are applied using `setrlimit(RLIMIT_AS)` which limits the virtual memory address space. This is applied when the process is spawned and enforced by the operating system.

### CPU Limits

CPU limits are implemented using cgroups v2 on Linux:

1. A cgroup is created at `/sys/fs/cgroup/adasa/<process-name>`
2. The process is added to this cgroup
3. CPU quota is set using the `cpu.max` controller
4. The quota is calculated as: `(period * cpu_percent) / 100`

For example, a 50% CPU limit means the process can use 50ms of CPU time out of every 100ms period.

### Violation Detection

The supervisor checks resource usage periodically (default: every 5 seconds) and:

1. Compares current usage against configured limits
2. Records violations in process statistics
3. Executes the configured limit action (log/restart/stop)
4. Logs all violations with timestamps

## Limitations

- **Memory limits:** Applied at spawn time, cannot be changed for running processes
- **CPU limits:** Only supported on Linux with cgroups v2
- **Permissions:** CPU throttling may require elevated privileges
- **Granularity:** CPU limits are per-core percentages (not total system CPU)

## Best Practices

1. **Set realistic limits:** Monitor your processes first to understand their normal resource usage
2. **Use restart action carefully:** Frequent restarts may indicate underlying issues
3. **Test in development:** Verify limits work as expected before deploying to production
4. **Monitor violations:** Check violation counts regularly to identify problematic processes
5. **Combine with autorestart:** Use resource limits with autorestart policies for resilient services

## Troubleshooting

### CPU limits not working

Check if cgroups v2 is available:
```bash
ls /sys/fs/cgroup/cgroup.controllers
```

If the file doesn't exist, your system may be using cgroups v1 or cgroups may not be enabled.

### Permission denied errors

CPU throttling requires permissions to create cgroups. Run the daemon with appropriate privileges or configure your system to allow cgroup management for your user.

### Memory limits causing crashes

If processes are being killed frequently due to memory limits:
1. Increase the limit
2. Investigate memory leaks in your application
3. Consider using the "restart" action to automatically recover
