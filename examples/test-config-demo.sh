#!/bin/bash
# Demo script to test configuration file support

set -e

echo "=== Adasa Configuration File Demo ==="
echo ""

# Check if daemon is running
if ! ./target/release/adasa daemon status 2>/dev/null | grep -q "running"; then
    echo "Starting daemon..."
    ./target/release/adasa daemon start
    sleep 2
fi

echo "Daemon is running"
echo ""

# Create a test config file
cat > /tmp/adasa-test-config.json << 'EOF'
{
  "processes": [
    {
      "name": "echo-test-1",
      "script": "/bin/sleep",
      "args": ["300"],
      "instances": 2,
      "autorestart": true,
      "env": {
        "TEST_VAR": "test_value"
      }
    },
    {
      "name": "echo-test-2",
      "script": "/bin/sleep",
      "args": ["300"],
      "instances": 1,
      "autorestart": true
    }
  ]
}
EOF

echo "Created test configuration file at /tmp/adasa-test-config.json"
echo ""

# Start processes from config
echo "Starting processes from config file..."
./target/release/adasa start --config /tmp/adasa-test-config.json
echo ""

# Wait a moment for processes to start
sleep 2

# List processes
echo "Listing all processes:"
./target/release/adasa list
echo ""

# Test reload with updated config
cat > /tmp/adasa-test-config-reload.json << 'EOF'
{
  "processes": [
    {
      "name": "echo-test-1",
      "script": "/bin/sleep",
      "args": ["300"],
      "instances": 2,
      "autorestart": true
    },
    {
      "name": "echo-test-2",
      "script": "/bin/sleep",
      "args": ["300"],
      "instances": 1,
      "autorestart": true
    },
    {
      "name": "echo-test-3",
      "script": "/bin/sleep",
      "args": ["300"],
      "instances": 1,
      "autorestart": true
    }
  ]
}
EOF

echo "Reloading configuration with new process..."
./target/release/adasa reload /tmp/adasa-test-config-reload.json
echo ""

# Wait a moment
sleep 2

# List processes again
echo "Listing all processes after reload:"
./target/release/adasa list
echo ""

# Cleanup
echo "Cleaning up test processes..."
for id in $(./target/release/adasa list 2>/dev/null | grep "echo-test" | awk '{print $1}' | grep -E '^[0-9]+$'); do
    ./target/release/adasa delete $id 2>/dev/null || true
done

# Clean up temp files
rm -f /tmp/adasa-test-config.json /tmp/adasa-test-config-reload.json

echo ""
echo "=== Demo Complete ==="
