# Configuration File Quick Reference

## Basic Commands

```bash
# Start processes from config file
adasa start --config config.toml
adasa start -f config.json

# Reload config (add new processes without stopping existing)
adasa reload config.toml
```

## Minimal Config

### TOML
```toml
name = "my-app"
script = "/usr/bin/node"
args = ["server.js"]
```

### JSON
```json
{
  "name": "my-app",
  "script": "/usr/bin/node",
  "args": ["server.js"]
}
```

## Multi-Process Config

### TOML
```toml
[[processes]]
name = "web"
script = "/usr/bin/node"
args = ["server.js"]
instances = 4

[[processes]]
name = "worker"
script = "/usr/bin/python3"
args = ["worker.py"]
instances = 2
```

### JSON
```json
{
  "processes": [
    {
      "name": "web",
      "script": "/usr/bin/node",
      "args": ["server.js"],
      "instances": 4
    },
    {
      "name": "worker",
      "script": "/usr/bin/python3",
      "args": ["worker.py"],
      "instances": 2
    }
  ]
}
```

## Common Options

```toml
name = "my-app"
script = "/usr/bin/node"
args = ["server.js"]
cwd = "/var/www/app"
instances = 4
autorestart = true
max_restarts = 10
restart_delay_secs = 2
max_memory = 536870912  # 512MB
max_cpu = 80
limit_action = "restart"
stop_signal = "SIGTERM"
stop_timeout_secs = 10

[env]
NODE_ENV = "production"
PORT = "3000"
```

## Environment Variables

```toml
name = "app"
script = "$HOME/bin/myapp"
args = ["--config=${CONFIG_PATH}"]

[env]
DATABASE_URL = "$DATABASE_URL"
API_KEY = "${API_KEY}"
```

## Resource Limits

```toml
name = "limited-app"
script = "/usr/bin/node"
args = ["app.js"]
max_memory = 268435456  # 256MB in bytes
max_cpu = 50            # 50% CPU
limit_action = "restart" # or "log" or "stop"
```

## Field Reference

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | Yes | - | Process name |
| `script` | string | Yes | - | Executable path |
| `args` | array | No | `[]` | Arguments |
| `cwd` | string | No | current | Working directory |
| `env` | object | No | `{}` | Environment vars |
| `instances` | number | No | `1` | Instance count (1-100) |
| `autorestart` | boolean | No | `true` | Auto-restart on crash |
| `max_restarts` | number | No | `10` | Max restart attempts |
| `restart_delay_secs` | number | No | `1` | Restart delay |
| `max_memory` | number | No | none | Memory limit (bytes) |
| `max_cpu` | number | No | none | CPU limit (1-100%) |
| `limit_action` | string | No | `"log"` | "log", "restart", "stop" |
| `stop_signal` | string | No | `"SIGTERM"` | Stop signal |
| `stop_timeout_secs` | number | No | `10` | Stop timeout |

## Valid Signals

- `SIGTERM` (default)
- `SIGINT`
- `SIGQUIT`
- `SIGKILL`
- `SIGHUP`
- `SIGUSR1`
- `SIGUSR2`

## Tips

1. **Start small** - Begin with minimal config and add options as needed
2. **Use env vars** - Keep secrets in environment variables, not config files
3. **Test first** - Validate configs in a test environment
4. **Version control** - Keep config files in git
5. **Document** - Add comments (TOML only) to explain process purposes
6. **Resource limits** - Set appropriate limits to prevent resource exhaustion
7. **Instances** - Use multiple instances for load balancing

## See Also

- [Full Documentation](../docs/configuration-files.md)
- [Example Configs](../examples/)
