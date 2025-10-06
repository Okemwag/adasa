# Configuration File Support

Adasa supports loading process configurations from TOML or JSON files, making it easy to manage multiple processes declaratively.

## Overview

Configuration files allow you to:
- Define multiple processes in a single file
- Specify all process settings (instances, environment variables, resource limits, etc.)
- Start all processes with a single command
- Reload configuration to add new processes without stopping existing ones
- Use environment variable expansion for dynamic configuration

## File Formats

Adasa supports two configuration file formats:
- **TOML** (`.toml` extension)
- **JSON** (`.json` extension)

## Basic Usage

### Starting Processes from Config

```bash
# Start all processes defined in a config file
adasa start --config config.toml

# Or using short form
adasa start -f config.json
```

### Reloading Configuration

```bash
# Reload config file and start any new processes
# Existing processes are not affected
adasa reload config.toml
```

## Configuration Schema

### Required Fields

- `name` - Unique process name (string)
- `script` - Path to executable or script (string)

### Optional Fields

- `args` - Command-line arguments (array of strings, default: `[]`)
- `cwd` - Working directory (string, default: current directory)
- `env` - Environment variables (object/map, default: `{}`)
- `instances` - Number of instances to run (integer, default: `1`)
- `autorestart` - Auto-restart on crash (boolean, default: `true`)
- `max_restarts` - Maximum restart attempts (integer, default: `10`)
- `restart_delay_secs` - Delay before restart in seconds (integer, default: `1`)
- `max_memory` - Memory limit in bytes (integer, optional)
- `max_cpu` - CPU limit percentage 1-100 (integer, optional)
- `limit_action` - Action on limit violation: `"log"`, `"restart"`, or `"stop"` (string, default: `"log"`)
- `stop_signal` - Signal to send on stop (string, default: `"SIGTERM"`)
- `stop_timeout_secs` - Timeout before force kill in seconds (integer, default: `10`)

## TOML Configuration Examples

### Single Process

```toml
name = "web-server"
script = "/usr/bin/node"
args = ["server.js", "--port=3000"]
cwd = "/var/www/app"
instances = 4
autorestart = true
max_restarts = 10
restart_delay_secs = 2
stop_signal = "SIGTERM"
stop_timeout_secs = 10

[env]
NODE_ENV = "production"
PORT = "3000"
```

### Multiple Processes

```toml
[[processes]]
name = "web-server"
script = "/usr/bin/node"
args = ["server.js"]
cwd = "/var/www/app"
instances = 4
autorestart = true

[processes.env]
NODE_ENV = "production"
PORT = "3000"

[[processes]]
name = "worker"
script = "/usr/bin/python3"
args = ["worker.py"]
cwd = "/var/www/app"
instances = 2
autorestart = true
max_memory = 536870912  # 512MB
max_cpu = 75
limit_action = "restart"

[processes.env]
PYTHON_ENV = "production"
WORKER_THREADS = "4"

[[processes]]
name = "background-job"
script = "/usr/bin/python3"
args = ["job.py", "--interval=60"]
cwd = "/var/www/jobs"
instances = 1
autorestart = true
max_restarts = 5
restart_delay_secs = 10
stop_signal = "SIGINT"
stop_timeout_secs = 30
```

## JSON Configuration Examples

### Single Process

```json
{
  "name": "api-server",
  "script": "/usr/bin/node",
  "args": ["api.js"],
  "cwd": "/var/www/api",
  "instances": 2,
  "autorestart": true,
  "max_restarts": 10,
  "restart_delay_secs": 1,
  "stop_signal": "SIGTERM",
  "stop_timeout_secs": 10,
  "env": {
    "NODE_ENV": "production",
    "API_PORT": "8080"
  }
}
```

### Multiple Processes

```json
{
  "processes": [
    {
      "name": "api-server",
      "script": "/usr/bin/node",
      "args": ["api.js"],
      "cwd": "/var/www/api",
      "instances": 2,
      "autorestart": true,
      "max_restarts": 10,
      "restart_delay_secs": 1,
      "env": {
        "NODE_ENV": "production",
        "API_PORT": "8080"
      }
    },
    {
      "name": "background-job",
      "script": "/usr/bin/python3",
      "args": ["job.py", "--interval=60"],
      "cwd": "/var/www/jobs",
      "instances": 1,
      "autorestart": true,
      "max_restarts": 5,
      "restart_delay_secs": 10,
      "max_memory": 268435456,
      "stop_signal": "SIGINT",
      "stop_timeout_secs": 30,
      "env": {
        "PYTHON_ENV": "production",
        "LOG_LEVEL": "info"
      }
    }
  ]
}
```

## Environment Variable Expansion

Configuration files support environment variable expansion using `$VAR` or `${VAR}` syntax:

```toml
name = "app"
script = "$HOME/bin/myapp"
args = ["--config=${CONFIG_PATH}"]
cwd = "${APP_DIR}"

[env]
DATABASE_URL = "$DATABASE_URL"
API_KEY = "${API_KEY}"
```

```json
{
  "name": "app",
  "script": "$HOME/bin/myapp",
  "args": ["--config=${CONFIG_PATH}"],
  "cwd": "${APP_DIR}",
  "env": {
    "DATABASE_URL": "$DATABASE_URL",
    "API_KEY": "${API_KEY}"
  }
}
```

## Resource Limits

You can set memory and CPU limits for processes:

```toml
name = "limited-app"
script = "/usr/bin/node"
args = ["app.js"]
max_memory = 268435456  # 256MB in bytes
max_cpu = 50            # 50% CPU limit
limit_action = "restart" # restart, stop, or log
```

```json
{
  "name": "limited-app",
  "script": "/usr/bin/node",
  "args": ["app.js"],
  "max_memory": 268435456,
  "max_cpu": 50,
  "limit_action": "restart"
}
```

### Limit Actions

- `"log"` - Log the violation but continue running (default)
- `"restart"` - Restart the process when limit is exceeded
- `"stop"` - Stop the process when limit is exceeded

## Multi-Instance Support

Run multiple instances of the same process for load balancing:

```toml
name = "web-server"
script = "/usr/bin/node"
args = ["server.js"]
instances = 4  # Runs 4 instances: web-server-0, web-server-1, web-server-2, web-server-3
```

Each instance gets a unique name with a numeric suffix.

## Validation

Configuration files are validated before processes are started. Common validation errors:

- **Empty name**: Process name cannot be empty
- **Zero instances**: Must have at least 1 instance
- **Invalid signal**: Stop signal must be one of: SIGTERM, SIGINT, SIGQUIT, SIGKILL, SIGHUP, SIGUSR1, SIGUSR2
- **Invalid working directory**: Directory must exist
- **Invalid CPU limit**: Must be between 1 and 100
- **Too many instances**: Cannot exceed 100 instances per process

## Configuration Reload Behavior

When you reload a configuration file:

1. **Existing processes** with matching names are left running (not restarted)
2. **New processes** defined in the config are started
3. **Removed processes** are NOT stopped (use `adasa stop` or `adasa delete` to remove them)

This allows you to add new processes without disrupting existing ones.

## Best Practices

1. **Use version control** - Keep your config files in git
2. **Environment-specific configs** - Use different config files for dev/staging/production
3. **Environment variables** - Use env var expansion for secrets and environment-specific values
4. **Start with defaults** - Only specify settings that differ from defaults
5. **Test configs** - Validate configs by starting processes in a test environment first
6. **Document processes** - Add comments (in TOML) to explain what each process does
7. **Resource limits** - Set appropriate memory and CPU limits to prevent resource exhaustion

## Example: Complete Production Setup

```toml
# Production configuration for MyApp

[[processes]]
name = "nginx"
script = "/usr/sbin/nginx"
args = ["-g", "daemon off;"]
instances = 1
autorestart = true
max_restarts = 5
restart_delay_secs = 5
stop_signal = "SIGQUIT"
stop_timeout_secs = 30

[[processes]]
name = "api"
script = "/usr/bin/node"
args = ["dist/server.js"]
cwd = "/var/www/api"
instances = 4
autorestart = true
max_restarts = 10
restart_delay_secs = 2
max_memory = 536870912  # 512MB
max_cpu = 80
limit_action = "restart"

[processes.env]
NODE_ENV = "production"
PORT = "3000"
DATABASE_URL = "${DATABASE_URL}"
REDIS_URL = "${REDIS_URL}"

[[processes]]
name = "worker"
script = "/usr/bin/node"
args = ["dist/worker.js"]
cwd = "/var/www/api"
instances = 2
autorestart = true
max_restarts = 5
restart_delay_secs = 10
max_memory = 268435456  # 256MB

[processes.env]
NODE_ENV = "production"
QUEUE_URL = "${QUEUE_URL}"

[[processes]]
name = "scheduler"
script = "/usr/bin/node"
args = ["dist/scheduler.js"]
cwd = "/var/www/api"
instances = 1
autorestart = true
max_restarts = 3
restart_delay_secs = 30

[processes.env]
NODE_ENV = "production"
DATABASE_URL = "${DATABASE_URL}"
```

## Troubleshooting

### Config file not found
```bash
Error: Failed to read config file: No such file or directory
```
**Solution**: Check that the file path is correct and the file exists.

### Invalid format
```bash
Error: Unsupported file format: yaml. Use .toml or .json
```
**Solution**: Use `.toml` or `.json` file extension.

### Validation errors
```bash
Error: Configuration validation failed: instances must be at least 1
```
**Solution**: Fix the validation error in your config file.

### Working directory doesn't exist
```bash
Error: Working directory does not exist: /path/to/dir
```
**Solution**: Create the directory or fix the path in your config.

## See Also

- [CLI Output Formatting](cli-output-formatting.md)
- [Resource Limits](resource-limits.md)
- [Graceful Shutdown](graceful-shutdown.md)
- [Rolling Restart](rolling-restart.md)
