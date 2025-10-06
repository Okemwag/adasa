#!/bin/bash
# Adasa systemd service installation script

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Print colored message
print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if running as root for system-wide installation
check_root() {
    if [ "$EUID" -ne 0 ]; then
        return 1
    fi
    return 0
}

# Install system-wide service
install_system() {
    print_info "Installing Adasa as a system-wide service..."
    
    # Check if adasa binary exists
    if ! command -v adasa &> /dev/null; then
        print_error "adasa binary not found in PATH"
        print_info "Please install adasa first: cargo install adasa"
        exit 1
    fi
    
    # Get adasa binary path
    ADASA_PATH=$(which adasa)
    print_info "Found adasa at: $ADASA_PATH"
    
    # Prompt for user
    read -p "Enter the user to run Adasa as (default: $SUDO_USER): " SERVICE_USER
    SERVICE_USER=${SERVICE_USER:-$SUDO_USER}
    
    # Validate user exists
    if ! id "$SERVICE_USER" &>/dev/null; then
        print_error "User $SERVICE_USER does not exist"
        exit 1
    fi
    
    # Create service file
    SERVICE_FILE="/etc/systemd/system/adasa@.service"
    print_info "Creating service file: $SERVICE_FILE"
    
    cat > "$SERVICE_FILE" << EOF
[Unit]
Description=Adasa Process Manager
Documentation=https://github.com/Okemwag/adasa
After=network.target

[Service]
Type=forking
User=%i
Group=%i
ExecStart=$ADASA_PATH daemon start
ExecStop=$ADASA_PATH daemon stop
ExecReload=$ADASA_PATH daemon restart
Restart=on-failure
RestartSec=5s
TimeoutStopSec=30s

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=read-only
ReadWritePaths=/home/%i/.adasa /var/log/adasa

# Resource limits
LimitNOFILE=65536
LimitNPROC=4096

[Install]
WantedBy=multi-user.target
EOF
    
    # Reload systemd
    print_info "Reloading systemd daemon..."
    systemctl daemon-reload
    
    # Enable service
    print_info "Enabling Adasa service for user: $SERVICE_USER"
    systemctl enable "adasa@$SERVICE_USER.service"
    
    print_info "✓ System-wide installation complete!"
    echo ""
    print_info "To start Adasa, run:"
    echo "  sudo systemctl start adasa@$SERVICE_USER"
    echo ""
    print_info "To check status:"
    echo "  sudo systemctl status adasa@$SERVICE_USER"
    echo ""
    print_info "To enable auto-start on boot:"
    echo "  sudo systemctl enable adasa@$SERVICE_USER"
}

# Install user service
install_user() {
    print_info "Installing Adasa as a user service..."
    
    # Check if adasa binary exists
    if ! command -v adasa &> /dev/null; then
        print_error "adasa binary not found in PATH"
        print_info "Please install adasa first: cargo install adasa"
        exit 1
    fi
    
    # Get adasa binary path
    ADASA_PATH=$(which adasa)
    print_info "Found adasa at: $ADASA_PATH"
    
    # Create user systemd directory
    USER_SYSTEMD_DIR="$HOME/.config/systemd/user"
    mkdir -p "$USER_SYSTEMD_DIR"
    
    # Create service file
    SERVICE_FILE="$USER_SYSTEMD_DIR/adasa.service"
    print_info "Creating service file: $SERVICE_FILE"
    
    cat > "$SERVICE_FILE" << EOF
[Unit]
Description=Adasa Process Manager (User Service)
Documentation=https://github.com/Okemwag/adasa
After=network.target

[Service]
Type=forking
ExecStart=$ADASA_PATH daemon start
ExecStop=$ADASA_PATH daemon stop
ExecReload=$ADASA_PATH daemon restart
Restart=on-failure
RestartSec=5s
TimeoutStopSec=30s

# Resource limits
LimitNOFILE=65536
LimitNPROC=4096

[Install]
WantedBy=default.target
EOF
    
    # Reload systemd user daemon
    print_info "Reloading systemd user daemon..."
    systemctl --user daemon-reload
    
    # Enable service
    print_info "Enabling Adasa user service..."
    systemctl --user enable adasa.service
    
    # Enable lingering to allow service to run when user is not logged in
    print_info "Enabling user lingering..."
    loginctl enable-linger "$USER" 2>/dev/null || print_warn "Could not enable lingering (requires root)"
    
    print_info "✓ User service installation complete!"
    echo ""
    print_info "To start Adasa, run:"
    echo "  systemctl --user start adasa"
    echo ""
    print_info "To check status:"
    echo "  systemctl --user status adasa"
    echo ""
    print_info "To enable auto-start on login:"
    echo "  systemctl --user enable adasa"
}

# Uninstall system service
uninstall_system() {
    print_info "Uninstalling Adasa system service..."
    
    # Prompt for user
    read -p "Enter the user the service was installed for (default: $SUDO_USER): " SERVICE_USER
    SERVICE_USER=${SERVICE_USER:-$SUDO_USER}
    
    # Stop service if running
    if systemctl is-active --quiet "adasa@$SERVICE_USER.service"; then
        print_info "Stopping service..."
        systemctl stop "adasa@$SERVICE_USER.service"
    fi
    
    # Disable service
    if systemctl is-enabled --quiet "adasa@$SERVICE_USER.service" 2>/dev/null; then
        print_info "Disabling service..."
        systemctl disable "adasa@$SERVICE_USER.service"
    fi
    
    # Remove service file
    SERVICE_FILE="/etc/systemd/system/adasa@.service"
    if [ -f "$SERVICE_FILE" ]; then
        print_info "Removing service file..."
        rm "$SERVICE_FILE"
    fi
    
    # Reload systemd
    systemctl daemon-reload
    
    print_info "✓ System service uninstalled"
}

# Uninstall user service
uninstall_user() {
    print_info "Uninstalling Adasa user service..."
    
    # Stop service if running
    if systemctl --user is-active --quiet adasa.service; then
        print_info "Stopping service..."
        systemctl --user stop adasa.service
    fi
    
    # Disable service
    if systemctl --user is-enabled --quiet adasa.service 2>/dev/null; then
        print_info "Disabling service..."
        systemctl --user disable adasa.service
    fi
    
    # Remove service file
    SERVICE_FILE="$HOME/.config/systemd/user/adasa.service"
    if [ -f "$SERVICE_FILE" ]; then
        print_info "Removing service file..."
        rm "$SERVICE_FILE"
    fi
    
    # Reload systemd user daemon
    systemctl --user daemon-reload
    
    print_info "✓ User service uninstalled"
}

# Show usage
show_usage() {
    cat << EOF
Adasa systemd service installer

Usage: $0 [COMMAND]

Commands:
  install-system    Install Adasa as a system-wide service (requires root)
  install-user      Install Adasa as a user service (no root required)
  uninstall-system  Uninstall system-wide service (requires root)
  uninstall-user    Uninstall user service
  help              Show this help message

Examples:
  # Install as user service (recommended for development)
  $0 install-user

  # Install as system service (recommended for production)
  sudo $0 install-system

  # Uninstall user service
  $0 uninstall-user

  # Uninstall system service
  sudo $0 uninstall-system

EOF
}

# Main script
main() {
    case "${1:-}" in
        install-system)
            if ! check_root; then
                print_error "System-wide installation requires root privileges"
                print_info "Please run: sudo $0 install-system"
                exit 1
            fi
            install_system
            ;;
        install-user)
            if check_root; then
                print_warn "Running as root. Consider using 'install-system' instead."
                print_warn "User service will be installed for root user."
            fi
            install_user
            ;;
        uninstall-system)
            if ! check_root; then
                print_error "System-wide uninstallation requires root privileges"
                print_info "Please run: sudo $0 uninstall-system"
                exit 1
            fi
            uninstall_system
            ;;
        uninstall-user)
            uninstall_user
            ;;
        help|--help|-h)
            show_usage
            ;;
        *)
            print_error "Unknown command: ${1:-}"
            echo ""
            show_usage
            exit 1
            ;;
    esac
}

main "$@"
