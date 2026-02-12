#!/bin/bash

set -e

PROJECT_NAME="tcp-rs"
BIN_PATH="./target/debug/$PROJECT_NAME"
TUN_IFACE="tun0"
IP_ADDR="192.168.0.1/24"

echo "🔨 Building $PROJECT_NAME..."
cargo build

echo "🛡️  Granting CAP_NET_ADMIN..."
sudo setcap cap_net_admin=eip "$BIN_PATH"

echo "🚀 Starting binary..."
"$BIN_PATH" &
PID=$!

cleanup() {
    echo ""
    echo "🛑 Shutting down..."
    if kill -0 $PID 2>/dev/null; then
        kill $PID
        wait $PID 2>/dev/null
    fi
}
trap cleanup EXIT INT TERM

echo "⏳ Waiting for interface $TUN_IFACE..."
MAX_RETRIES=50
COUNT=0
while ! ip link show "$TUN_IFACE" > /dev/null 2>&1; do
    sleep 0.1
    COUNT=$((COUNT+1))
    if [ $COUNT -ge $MAX_RETRIES ]; then
        echo "❌ Error: Interface $TUN_IFACE creation timed out!"
        exit 1
    fi
done

echo "🔧 Configuring network ($IP_ADDR)..."
sudo ip addr add "$IP_ADDR" dev "$TUN_IFACE"
sudo ip link set up dev "$TUN_IFACE"

echo "✅ Ready! Logic is running with PID $PID"
echo "   (Press Ctrl+C to stop)"

wait $PID
