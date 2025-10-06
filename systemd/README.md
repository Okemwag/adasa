# Adasa Systemd Integration

This directory contains systemd service files and installation scripts for running Adasa as a system service.

## Overview

Running Adasa as a systemd service provides several benefits:

- **Auto-start on boot** - Adasa starts automatically when the system boots
- **Automatic restart** - If Adasa crashes, systemd will restart it automatically
- **Resource management** - Systemd can enforce resource limits
- **Logging integration** - Logs are available via `journalctl`
- **Service management** - Standard systemd commands for start/stop/status

## Installation Options

### Option 1: User Service (Recommended for Development)

User services run under your user account and don't require root privileges.

**Advantages:**
- No root access required
- Isolated to your user account
- Easy to install and manage
- Suitable for development environments

**Installation:**

```bash
# Using the installation script
./systemd/install.sh install-user

# Or manually
mkdir -p ~/.config/systemd/user
cp systemd/adasa-user.service ~/.config/systemd/user/adasa.service
systemctl --user daemon-reload
systemctl --user enable adasa
systemctl --user start adasa
```

**Management:**

```bash
# Start the service
systemctl --user start adasa

# Stop the service
systemctl --user stop adasa

# Restart the service
systemctl --user restart adasa

# Check status
systemctl --user status adasa

# View logs
journalctl --user -u adasa -f

# Enable auto-start on login
systemctl --user enable adasa

# Disable auto-start
systemctl --user disable adasa
```

### Option 2: System Service (Recommended for Production)

System services run at the system level and can be configured for any user.

**Advantages:**
- Runs at system level
- Can be configured for specific users
- Better for production deployments
- Survives user logouts

**Installation:**

```bash
# Using the installation script (recommended)
sudo ./systemd/install.sh install-system

# Or manually
sudo cp systemd/adasa.service /etc/systemd/system/adasa@.service
sudo systemctl daemon-reload
sudo systemctl enable adasa@yourusername
sudo systemctl start adasa@yourusername
```

**Management:**

```bash
# Replace 'yourusername' with the actual username

# Start the service
sudo systemctl start adasa@yourusername

# Stop the service
sudo systemctl stop adasa@yourusername

# Restart the service
sudo systemctl restart adasa@yourusername

# Check status
sudo systemctl status adasa@yourusername

# View logs
sudo journalctl -u adasa@yourusername -f

# Enable auto-start on boot
sudo systemctl enable adasa@yourusername

# Disable auto-start
sudo systemctl disable adasa@yourusername
```

## Service Files

### adasa.service (System Service)

Template service file for system-wide installation. Uses systemd's instance feature (`@`) to support multiple users.

**Features:**
- Runs as specified user
- Automatic restart on failure
- Security hardening (NoNewPrivileges, PrivateTmp, ProtectSystem)
- Resource limits (file descriptors, processes)
- Graceful shutdown with 30s timeout

**Customization:**

Edit `/etc/systemd/system/adasa@.service` to customize:

```ini
# Change restart behavior
Restart=always
RestartSec=10s

# Adjust resource limits
LimitNOFILE=131072
LimitNPROC=8192

# Add environment variables
Environment="RUST_LOG=info"
Environment="ADASA_HOME=/var/lib/adasa"

# Change working directory
WorkingDirectory=/var/lib/adasa
```

### adasa-user.service (User Service)

Service file for user-level installation.

**Features:**
- Runs under user account
- No root privileges required
- Automatic restart on failure
- Resource limits

**Customization:**

Edit `~/.config/systemd/user/adasa.service` to customize:

```ini
# Change restart behavior
Restart=always
RestartSec=10s

# Add environment variables
Environment="RUST_LOG=debug"

# Change working directory
WorkingDirectory=%h/adasa
```

## Installation Script

The `install.sh` script provides an interactive way to install and manage Adasa systemd services.

### Usage

```bash
# Install as user service
./systemd/install.sh install-user

# Install as system service (requires root)
sudo ./systemd/install.sh install-system

# Uninstall user service
./systemd/install.sh uninstall-user

# Uninstall system service (requires root)
sudo ./systemd/install.sh uninstall-system

# Show help
./systemd/install.sh help
```

### Features

- Automatic detection of adasa binary location
- Interactive prompts for configuration
- Validation of user accounts
- Automatic systemd daemon reload
- Service enablement
- User lingering configuration (for user services)
- Colored output for better readability

## Auto-Start on Boot

### System Service

```bash
# Enable auto-start
sudo systemctl enable adasa@yourusername

# Verify it's enabled
sudo systemctl is-enabled adasa@yourusername
```

### User Service

For user services to start on boot (before login), you need to enable lingering:

```bash
# Enable lingering (allows services to run without login)
sudo loginctl enable-linger $USER

# Enable the service
systemctl --user enable adasa

# Verify
systemctl --user is-enabled adasa
```

## Logging

Systemd captures all output from Adasa and makes it available via `journalctl`.

### View Logs

```bash
# System service logs
sudo journalctl -u adasa@yourusername

# User service logs
journalctl --user -u adasa

# Follow logs in real-time
sudo journalctl -u adasa@yourusername -f

# Show only recent logs
sudo journalctl -u adasa@yourusername -n 100

# Show logs since boot
sudo journalctl -u adasa@yourusername -b

# Show logs for specific time range
sudo journalctl -u adasa@yourusername --since "2024-01-01" --until "2024-01-02"

# Show logs with priority (error and above)
sudo journalctl -u adasa@yourusername -p err
```

## Troubleshooting

### Service Won't Start

1. Check service status:
   ```bash
   sudo systemctl status adasa@yourusername
   ```

2. Check logs:
   ```bash
   sudo journalctl -u adasa@yourusername -n 50
   ```

3. Verify adasa binary exists:
   ```bash
   which adasa
   ```

4. Test adasa manually:
   ```bash
   adasa daemon start
   adasa list
   adasa daemon stop
   ```

### Permission Issues

If you see permission errors:

1. Check file permissions:
   ```bash
   ls -la ~/.adasa
   ```

2. Ensure the service user has access:
   ```bash
   # For system service
   sudo chown -R yourusername:yourusername ~/.adasa
   ```

3. Check SELinux/AppArmor if enabled:
   ```bash
   # Check SELinux status
   sestatus
   
   # Check AppArmor status
   sudo aa-status
   ```

### Service Keeps Restarting

1. Check if daemon is already running:
   ```bash
   adasa daemon status
   ```

2. Stop any manually started daemon:
   ```bash
   adasa daemon stop
   ```

3. Check for port conflicts or socket issues:
   ```bash
   # Check if socket file exists
   ls -la ~/.adasa/adasa.sock
   
   # Remove stale socket
   rm ~/.adasa/adasa.sock
   ```

### User Service Not Starting on Boot

1. Enable lingering:
   ```bash
   sudo loginctl enable-linger $USER
   ```

2. Verify lingering is enabled:
   ```bash
   loginctl show-user $USER | grep Linger
   ```

3. Check if service is enabled:
   ```bash
   systemctl --user is-enabled adasa
   ```

## Security Considerations

### System Service Security

The system service includes several security hardening options:

- **NoNewPrivileges**: Prevents privilege escalation
- **PrivateTmp**: Provides isolated /tmp directory
- **ProtectSystem=strict**: Makes most of the filesystem read-only
- **ProtectHome=read-only**: Makes home directories read-only
- **ReadWritePaths**: Explicitly allows writing to specific paths

### Customizing Security

You can adjust security settings based on your needs:

```ini
# Less restrictive (if processes need more access)
ProtectSystem=full
ProtectHome=no

# More restrictive
ProtectSystem=strict
ProtectHome=true
PrivateNetwork=true  # Isolate network
```

### Resource Limits

The service files include resource limits to prevent runaway processes:

```ini
LimitNOFILE=65536    # Max open files
LimitNPROC=4096      # Max processes
```

Adjust these based on your workload:

```ini
# For high-load scenarios
LimitNOFILE=131072
LimitNPROC=8192
LimitMEMLOCK=infinity
```

## Advanced Configuration

### Multiple Adasa Instances

You can run multiple Adasa instances for different users:

```bash
# Start for user1
sudo systemctl start adasa@user1

# Start for user2
sudo systemctl start adasa@user2

# Check all instances
sudo systemctl list-units 'adasa@*'
```

### Custom Socket Path

If you need to use a custom socket path:

1. Edit the service file:
   ```ini
   [Service]
   Environment="ADASA_SOCKET=/custom/path/adasa.sock"
   ```

2. Reload and restart:
   ```bash
   sudo systemctl daemon-reload
   sudo systemctl restart adasa@yourusername
   ```

### Integration with Other Services

Make Adasa start after other services:

```ini
[Unit]
After=network.target postgresql.service redis.service
Wants=postgresql.service redis.service
```

### Monitoring with systemd

Use systemd's built-in monitoring:

```bash
# Watch service status
watch -n 1 'systemctl status adasa@yourusername'

# Get service uptime
systemctl show adasa@yourusername -p ActiveEnterTimestamp

# Get restart count
systemctl show adasa@yourusername -p NRestarts
```

## Uninstallation

### User Service

```bash
# Using the script
./systemd/install.sh uninstall-user

# Or manually
systemctl --user stop adasa
systemctl --user disable adasa
rm ~/.config/systemd/user/adasa.service
systemctl --user daemon-reload
```

### System Service

```bash
# Using the script
sudo ./systemd/install.sh uninstall-system

# Or manually
sudo systemctl stop adasa@yourusername
sudo systemctl disable adasa@yourusername
sudo rm /etc/systemd/system/adasa@.service
sudo systemctl daemon-reload
```

## Best Practices

1. **Use system service for production** - More reliable and survives user logouts
2. **Enable lingering for user services** - Allows services to run without login
3. **Monitor logs regularly** - Use `journalctl` to check for issues
4. **Set appropriate resource limits** - Prevent resource exhaustion
5. **Use security hardening** - Enable security options in production
6. **Test before enabling** - Start service manually first to verify it works
7. **Keep backups** - Backup Adasa state directory (~/.adasa)

## Examples

### Development Setup

```bash
# Install as user service
./systemd/install.sh install-user

# Start the service
systemctl --user start adasa

# Deploy your app
adasa start ./my-app --name dev-app

# Check status
systemctl --user status adasa
adasa list
```

### Production Setup

```bash
# Install as system service
sudo ./systemd/install.sh install-system

# Enable auto-start
sudo systemctl enable adasa@appuser

# Start the service
sudo systemctl start adasa@appuser

# Deploy your apps from config
sudo -u appuser adasa start --config /etc/adasa/production.toml

# Monitor
sudo systemctl status adasa@appuser
sudo journalctl -u adasa@appuser -f
```

## Support

For issues or questions:

- Check the [main documentation](../README.md)
- Open an issue on [GitHub](https://github.com/Okemwa/adasa/issues)
- Check systemd logs: `journalctl -u adasa@yourusername`

