#!/bin/bash

set -e

# Source monitoring functions if available
if [ -f "./emulator-monitoring.sh" ]; then
    source ./emulator-monitoring.sh
else
    echo "Warning: emulator-monitoring.sh not found, using basic monitoring"
    wait_for_boot() {
        local port=$1
        echo "Waiting for emulator on port $port to boot..."
        while ! adb -s emulator-$port shell getprop sys.boot_completed 2>/dev/null | grep -q "1"; do
            echo "Waiting for boot to complete on emulator-$port..."
            sleep 5
        done
        echo "Emulator on port $port boot completed!"
    }
    update_state() {
        echo "State update: $1"
    }
fi

# Basic config
NUM_EMULATORS=${NUM_EMULATORS:-1}
BASE_CONSOLE_PORT=${BASE_CONSOLE_PORT:-5554}
BASE_ADB_PORT=${BASE_ADB_PORT:-5555}
ADB_SERVER_PORT=${ADB_SERVER_PORT:-5037}

# Fixed resources (per request)
OPT_MEMORY=512
OPT_CORES=2
OPT_PARTITION_SIZE=1024

# Android configuration
ANDROID_API_LEVEL=${ANDROID_API_LEVEL:-30}
ANDROID_ABI=${ABI:-x86_64}
ANDROID_DEVICE=${DEVICE_ID:-"pixel_6"}
PACKAGE_PATH=${PACKAGE_PATH:-"system-images;android-${ANDROID_API_LEVEL};google_apis;${ANDROID_ABI}"}
AVD_NAME_PREFIX=${AVD_NAME_PREFIX:-"avd"}

# Always use swiftshader
GPU_MODE="swiftshader_indirect"

# Display 1440x2560 or 
DISPLAY_WIDTH=${DISPLAY_WIDTH:-480}
DISPLAY_HEIGHT=${DISPLAY_HEIGHT:-800}
DISPLAY_DENSITY=${DISPLAY_DENSITY:-213}

SNAPSHOT_ENABLED=${SNAPSHOT_ENABLED:-"false"}
OPT_SKIP_AUTH=${SKIP_AUTH:-true}

AUTH_FLAG=""
export USER=${USER:-root}

echo "=== Android Emulator Startup Script (2025, simplified) ==="
echo "Configuration:"
echo "  Number of Emulators: $NUM_EMULATORS"
echo "  API Level: $ANDROID_API_LEVEL"
echo "  ABI: $ANDROID_ABI"
echo "  Device: $ANDROID_DEVICE"
echo "  Memory per emulator: ${OPT_MEMORY}MB"
echo "  CPU Cores per emulator: $OPT_CORES"
echo "  GPU Mode: $GPU_MODE"
echo "  Package: $PACKAGE_PATH"
echo ""

# Start ADB server
echo "Starting ADB server on port $ADB_SERVER_PORT..."
adb -a -P "$ADB_SERVER_PORT" server nodaemon &
ADB_PID=$!
sleep 2

# Port forwarding for container env
if [ -f /proc/net/route ]; then
    echo "Container environment detected, setting up port forwarding..."

    # Get container IP using multiple methods (more reliable)
    LOCAL_IP=$(hostname -I | awk '{print $1}' 2>/dev/null)

    if [ ! -z "$LOCAL_IP" ] && [ "$LOCAL_IP" != "127.0.0.1" ]; then
        echo "Forwarding ports from $LOCAL_IP to localhost..."
        for i in $(seq 0 $((NUM_EMULATORS - 1))); do
            CONSOLE_PORT=$((BASE_CONSOLE_PORT + i * 2))
            ADB_PORT=$((BASE_ADB_PORT + i * 2))
            echo "Setting up port forwarding for emulator $i: console=$CONSOLE_PORT, adb=$ADB_PORT"
            socat tcp-listen:"$CONSOLE_PORT",bind="$LOCAL_IP",fork tcp:127.0.0.1:"$CONSOLE_PORT" &
            socat tcp-listen:"$ADB_PORT",bind="$LOCAL_IP",fork tcp:127.0.0.1:"$ADB_PORT" &
        done
        echo "Port forwarding setup complete for IP: $LOCAL_IP"
    else
        echo "Warning: Could not determine container IP for port forwarding"
    fi
else
    echo "Not in container environment, skipping port forwarding"
fi

# Check if system image is installed
if ! sdkmanager --list_installed | grep -q "$PACKAGE_PATH"; then
    echo "Installing system image: $PACKAGE_PATH"
    yes | sdkmanager "$PACKAGE_PATH"
fi

# Setup authentication
if [ "$OPT_SKIP_AUTH" == "true" ]; then
    AUTH_FLAG="-skip-adb-auth"
fi

# Array to store emulator PIDs
declare -a EMULATOR_PIDS

# Cleanup function
cleanup() {
    echo "Cleaning up processes..."
    [ ! -z "$ADB_PID" ] && kill "$ADB_PID" 2>/dev/null || true
    for pid in "${EMULATOR_PIDS[@]}"; do
        [ ! -z "$pid" ] && kill "$pid" 2>/dev/null || true
    done
    pkill -f "socat.*tcp-listen" 2>/dev/null || true
    exit 0
}
trap cleanup EXIT INT TERM

# Create AVD
create_avd() {
    local avd_name=$1
    echo "Checking for existing AVD: $avd_name"
    if avdmanager list avd | grep -q "$avd_name"; then
        echo "Using existing AVD: $avd_name"
    else
        echo "Creating new AVD: $avd_name"
        echo no | avdmanager create avd \
            --force \
            --name "$avd_name" \
            --abi "$ANDROID_ABI" \
            --package "$PACKAGE_PATH" \
            --device "$ANDROID_DEVICE"

        # Configure AVD settings
        AVD_DIR="$HOME/.android/avd/${avd_name}.avd"
        if [ -d "$AVD_DIR" ]; then
            echo "Configuring AVD settings for $avd_name..."
            {
                echo "hw.ramSize=${OPT_MEMORY}"
                echo "hw.cpu.ncore=${OPT_CORES}"
                echo "disk.dataPartition.size=${OPT_PARTITION_SIZE}MB"
                echo "hw.lcd.width=${DISPLAY_WIDTH}"
                echo "hw.lcd.height=${DISPLAY_HEIGHT}"
                echo "hw.lcd.density=${DISPLAY_DENSITY}"
                echo "hw.keyboard=yes"
                echo "hw.dPad=no"
                echo "hw.camera.back=webcam0"
                echo "hw.camera.front=webcam0"
                echo "hw.gps=no"
                echo "hw.audioInput=yes"
                echo "hw.audioOutput=yes"
                echo "hw.sensors.proximity=yes"
                echo "hw.sensors.orientation=yes"
                echo "hw.accelerometer=yes"
                echo "hw.battery=yes"
                echo "hw.sdCard=yes"
            } >> "$AVD_DIR/config.ini"
        fi
    fi
}

# Start emulator
start_emulator() {
    local avd_name=$1
    local console_port=$2
    local adb_port=$3
    local instance_num=$4

    echo ""
    echo "=== Starting Android Emulator Instance $instance_num ==="
    local EMULATOR_CMD="emulator -avd $avd_name"
    EMULATOR_CMD="$EMULATOR_CMD -port $console_port"
    EMULATOR_CMD="$EMULATOR_CMD -gpu $GPU_MODE"
    EMULATOR_CMD="$EMULATOR_CMD -memory $OPT_MEMORY"
    EMULATOR_CMD="$EMULATOR_CMD -cores $OPT_CORES"
    EMULATOR_CMD="$EMULATOR_CMD -partition-size $OPT_PARTITION_SIZE"
    EMULATOR_CMD="$EMULATOR_CMD -no-boot-anim -no-window -writable-system"
    if [ ! -z "$AUTH_FLAG" ]; then
        EMULATOR_CMD="$EMULATOR_CMD $AUTH_FLAG"
    fi
    EMULATOR_CMD="$EMULATOR_CMD -accel auto -netdelay none -netspeed full -cache-size 1024"
    EMULATOR_CMD="$EMULATOR_CMD -qemu -cpu host"

    echo "Command: $EMULATOR_CMD"
    $EMULATOR_CMD &
    local emulator_pid=$!
    EMULATOR_PIDS[$instance_num]=$emulator_pid
    wait_for_boot $console_port &
    echo "Emulator instance $instance_num started with PID $emulator_pid"
}

# Main
echo ""
echo "Starting $NUM_EMULATORS emulator(s)..."
for i in $(seq 0 $((NUM_EMULATORS - 1))); do
    CONSOLE_PORT=$((BASE_CONSOLE_PORT + i * 2))
    ADB_PORT=$((BASE_ADB_PORT + i * 2))
    if [ "$NUM_EMULATORS" -eq 1 ]; then
        AVD_NAME="${AVD_NAME_PREFIX}_1"
    else
        AVD_NAME="${AVD_NAME_PREFIX}_instance_$i"
    fi
    create_avd "$AVD_NAME"
    start_emulator "$AVD_NAME" "$CONSOLE_PORT" "$ADB_PORT" "$i"
    if [ "$NUM_EMULATORS" -gt 1 ] && [ "$i" -lt $((NUM_EMULATORS - 1)) ]; then
        sleep 5
    fi
done

echo ""
echo "All emulator instances started successfully!"
update_state "ANDROID_RUNNING"
wait
