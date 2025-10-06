#!/bin/bash
# Example: Complete systemd setup for Adasa in production

set -e

echo "=== Adasa Production Systemd Setup Example ==="
echo ""

# This is an example script showing how to set up Adasa with systemd
# for a production environment. Adjust paths and users as needed.

# Configuration
APP_USER="appuser"
ADASA_BINARY="/usr/local/bin/adasa"
CONFIG_FILE="/etc/adasa/production.toml"

echo "Step 1: Install Adasa systemd service"
echo "--------------------------------------"
echo "sudo ./systemd/install.sh install-system"
echo ""

echo "Step 2: Enable auto-start on boot"
echo "----------------------------------"
echo "sudo systemctl enable adasa@${APP_USER}"
echo ""

echo "Step 3: Start the service"
echo "-------------------------"
echo "sudo systemctl start adasa@${APP_USER}"
echo ""

echo "Step 4: Verify service is running"
echo "----------------------------------"
echo "sudo systemctl status adasa@${APP_USER}"
echo ""

echo "Step 5: Deploy your applications"
echo "---------------------------------"
echo "sudo -u ${APP_USER} adasa start --config ${CONFIG_FILE}"
echo ""

echo "Step 6: Monitor the service"
echo "---------------------------"
echo "# View service status"
echo "sudo systemctl status adasa@${APP_USER}"
echo ""
echo "# View logs"
echo "sudo journalctl -u adasa@${APP_USER} -f"
echo ""
echo "# List managed processes"
echo "sudo -u ${APP_USER} adasa list"
echo ""

echo "Step 7: Set up log rotation (optional)"
echo "---------------------------------------"
cat << 'EOF'
# Create /etc/logrotate.d/adasa
/home/appuser/.adasa/logs/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
    create 0640 appuser appuser
}
EOF
echo ""

echo "Step 8: Set up monitoring (optional)"
echo "-------------------------------------"
echo "# Add to your monitoring system (e.g., Prometheus, Nagios)"
echo "# Monitor systemd service status:"
echo "systemctl is-active adasa@${APP_USER}"
echo ""
echo "# Monitor Adasa processes:"
echo "sudo -u ${APP_USER} adasa list --json | jq '.'"
echo ""

echo "=== Setup Complete ==="
echo ""
echo "Your Adasa daemon will now:"
echo "  ✓ Start automatically on system boot"
echo "  ✓ Restart automatically if it crashes"
echo "  ✓ Manage all configured processes"
echo "  ✓ Log to systemd journal"
echo ""
echo "For more information, see:"
echo "  - systemd/README.md - Full systemd documentation"
echo "  - systemd/QUICKSTART.md - Quick start guide"
echo "  - README.md - Main Adasa documentation"
