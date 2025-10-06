# Adasa Systemd Quick Start

This guide will get you up and running with Adasa as a systemd service in under 5 minutes.

## Prerequisites

- Adasa installed (`cargo install adasa` or from binary)
- Linux system with systemd
- Basic terminal knowledge

## Choose Your Installation Type

### For Development: User Service

**Pros:** No root required, easy to manage, isolated to your user  
**Cons:** Only runs when you're logged in (unless lingering is enabled)

```bash
# 1. Install the service
./systemd/install.sh install-user

# 2. Start Adasa
systemctl --user start adasa

# 3. Verify it's running
systemctl --user status adasa

# 4. (Optional) Enable auto-start on login
systemctl --user enable adasa

# 5. (Optional) Allow running without login
sudo loginctl enable-linger $USER
```

### For Production: System Service

**Pros:** Runs at system level, survives logouts, auto-starts on boot  
**Cons:** Requires root access

```bash
# 1. Install the service (will prompt for username)
sudo ./systemd/install.sh install-system

# 2. Start Adasa (replace 'yourusername' with your username)
sudo systemctl start adasa@yourusername

# 3. Verify it's running
sudo systemctl status adasa@yourusername

# 4. Enable auto-start on boot
sudo systemctl enable adasa@yourusername
```

## Common Commands

### User Service

```bash
# Start
systemctl --user start adasa

# Stop
systemctl --user stop adasa

# Restart
systemctl --user restart adasa

# Status
systemctl --user status adasa

# Logs
journalctl --user -u adasa -f

# Enable auto-start
systemctl --user enable adasa

# Disable auto-start
systemctl --user disable adasa
```

### System Service

Replace `yourusername` with the actual username:

```bash
# Start
sudo systemctl start adasa@yourusername

# Stop
sudo systemctl stop adasa@yourusername

# Restart
sudo systemctl restart adasa@yourusername

# Status
sudo systemctl status adasa@yourusername

# Logs
sudo journalctl -u adasa@yourusername -f

# Enable auto-start
sudo systemctl enable adasa@yourusername

# Disable auto-start
sudo systemctl disable adasa@yourusername
```

## Using Adasa After Installation

Once the service is running, use Adasa normally:

```bash
# Start a process
adasa start ./my-app --name my-service

# List processes
adasa list

# View logs
adasa logs my-service

# Stop a process
adasa stop my-service
```

## Troubleshooting

### Service won't start?

```bash
# Check status
systemctl --user status adasa  # or sudo systemctl status adasa@yourusername

# View logs
journalctl --user -u adasa -n 50  # or sudo journalctl -u adasa@yourusername -n 50

# Test manually
adasa daemon start
adasa list
adasa daemon stop
```

### Permission errors?

```bash
# Check Adasa directory permissions
ls -la ~/.adasa

# Fix permissions if needed
chmod 755 ~/.adasa
```

### Already running error?

```bash
# Stop any manually started daemon
adasa daemon stop

# Remove stale socket
rm ~/.adasa/adasa.sock

# Restart the service
systemctl --user restart adasa
```

## Next Steps

- Read the [full systemd documentation](README.md) for advanced configuration
- Check the [main README](../README.md) for Adasa usage
- Configure your processes with [configuration files](../docs/configuration-files.md)

## Uninstallation

### User ServiceOkemwa

```bash
./systemd/install.sh uninstall-user
```

### System Service

```bash
sudo ./systemd/install.sh uninstall-system
```

## Need Help?

- Full documentation: [systemd/README.md](README.md)
- Main documentation: [README.md](../README.md)
- Issues: [GitHub Issues](https://github.com/Okemwag/adasa/issues)
