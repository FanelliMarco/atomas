#!/bin/bash

set -e

echo "=== Android SDK Installation and Setup Script ==="

# Configuration
# ANDROID_SDK_ROOT=${ANDROID_SDK_ROOT:-/opt/android-sdk}
ANDROID_SDK_ROOT="/opt/android-sdk"
CMD_LINE_VERSION="13114758"
ANDROID_API_LEVEL=${ANDROID_API_LEVEL:-30}
BUILD_TOOLS_VERSION=${BUILD_TOOLS_VERSION:-35.0.0}
JAVA_VERSION="21"

echo "Configuration:"
echo "  Android SDK Root: $ANDROID_SDK_ROOT"
echo "  Command Line Tools Version: $CMD_LINE_VERSION"
echo "  Android API Level: $ANDROID_API_LEVEL"
echo "  Build Tools Version: $BUILD_TOOLS_VERSION"
echo "  Java Version: $JAVA_VERSION"
echo ""

# Step 1: Install required packages
echo "Step 1: Installing required packages..."
apt-get update -qq
apt-get install -y \
    wget \
    unzip \
    openjdk-${JAVA_VERSION}-jdk \
    curl \
    git \
    lib32stdc++6 \
    lib32z1 \
    libc6-dev-i386 \
    qemu-kvm \
    bridge-utils \
    cpu-checker

echo "✓ Required packages installed"

# Step 2: Set up Java environment
echo "Step 2: Setting up Java environment..."
export JAVA_HOME="/usr/lib/jvm/java-${JAVA_VERSION}-openjdk-amd64"
echo "export JAVA_HOME=\"/usr/lib/jvm/java-${JAVA_VERSION}-openjdk-amd64\"" >> ~/.bashrc

echo "✓ Java environment configured"
java -version

# Step 3: Create Android SDK directory
echo "Step 3: Creating Android SDK directory..."
mkdir -p "$ANDROID_SDK_ROOT"
mkdir -p "$ANDROID_SDK_ROOT/cmdline-tools"

echo "✓ Android SDK directory created"

# Step 4: Download Android Command Line Tools
echo "Step 4: Downloading Android Command Line Tools..."
cd /tmp
wget -q --show-progress \
    "https://dl.google.com/android/repository/commandlinetools-linux-${CMD_LINE_VERSION}_latest.zip" \
    -O commandlinetools-linux.zip

echo "✓ Android Command Line Tools downloaded"

# Step 5: Extract and setup command line tools
echo "Step 5: Extracting and setting up command line tools..."
unzip -q commandlinetools-linux.zip -d "$ANDROID_SDK_ROOT/cmdline-tools/"
mv "$ANDROID_SDK_ROOT/cmdline-tools/cmdline-tools" "$ANDROID_SDK_ROOT/cmdline-tools/latest"
rm -f /tmp/commandlinetools-linux.zip

echo "✓ Command line tools extracted and configured"

# Step 6: Set up environment variables
echo "Step 6: Setting up environment variables..."
export ANDROID_SDK_ROOT="$ANDROID_SDK_ROOT"
export ANDROID_HOME="$ANDROID_SDK_ROOT"
export PATH="$ANDROID_SDK_ROOT/cmdline-tools/latest/bin:$PATH"
export PATH="$ANDROID_SDK_ROOT/platform-tools:$PATH"
export PATH="$ANDROID_SDK_ROOT/emulator:$PATH"
export PATH="$ANDROID_SDK_ROOT/build-tools/$BUILD_TOOLS_VERSION:$PATH"

# Add to bashrc for persistence
cat >> ~/.bashrc << EOF

# Android SDK Environment (added by setup script)
export ANDROID_SDK_ROOT="$ANDROID_SDK_ROOT"
export ANDROID_HOME="\$ANDROID_SDK_ROOT"
export PATH="\$ANDROID_SDK_ROOT/cmdline-tools/latest/bin:\$PATH"
export PATH="\$ANDROID_SDK_ROOT/platform-tools:\$PATH"
export PATH="\$ANDROID_SDK_ROOT/emulator:\$PATH"
export PATH="\$ANDROID_SDK_ROOT/build-tools/$BUILD_TOOLS_VERSION:\$PATH"
EOF

echo "✓ Environment variables configured"

# Step 7: Verify command line tools
echo "Step 7: Verifying command line tools installation..."
if [ -x "$ANDROID_SDK_ROOT/cmdline-tools/latest/bin/sdkmanager" ]; then
    echo "✓ sdkmanager found at: $ANDROID_SDK_ROOT/cmdline-tools/latest/bin/sdkmanager"
else
    echo "✗ sdkmanager not found!"
    exit 1
fi

if [ -x "$ANDROID_SDK_ROOT/cmdline-tools/latest/bin/avdmanager" ]; then
    echo "✓ avdmanager found at: $ANDROID_SDK_ROOT/cmdline-tools/latest/bin/avdmanager"
else
    echo "✗ avdmanager not found!"
    exit 1
fi

# Step 8: Accept licenses and install SDK components
echo "Step 8: Accepting licenses and installing SDK components..."
echo "Accepting Android SDK licenses..."
yes | "$ANDROID_SDK_ROOT/cmdline-tools/latest/bin/sdkmanager" --licenses > /dev/null 2>&1

echo "Installing SDK components..."
"$ANDROID_SDK_ROOT/cmdline-tools/latest/bin/sdkmanager" --install \
    "platforms;android-$ANDROID_API_LEVEL" \
    "build-tools;$BUILD_TOOLS_VERSION" \
    "platform-tools" \
    "emulator" \
    "system-images;android-$ANDROID_API_LEVEL;google_apis;x86_64"

echo "✓ SDK components installed"

# Step 9: Verify installation
echo "Step 9: Verifying installation..."
echo ""
echo "=== Installation Verification ==="

# Test commands
echo "Testing sdkmanager:"
"$ANDROID_SDK_ROOT/cmdline-tools/latest/bin/sdkmanager" --version

echo ""
echo "Testing avdmanager:"
"$ANDROID_SDK_ROOT/cmdline-tools/latest/bin/avdmanager" list target

echo ""
echo "Testing adb (after platform-tools installation):"
if [ -x "$ANDROID_SDK_ROOT/platform-tools/adb" ]; then
    "$ANDROID_SDK_ROOT/platform-tools/adb" version
else
    echo "ADB not yet available (will be available after sourcing bashrc)"
fi

echo ""
echo "=== Installation Complete ==="
echo ""
echo "✅ Android SDK successfully installed!"
echo ""
echo "📁 Installation directory: $ANDROID_SDK_ROOT"
echo "📋 Installed components:"
echo "   • Android API Level $ANDROID_API_LEVEL"
echo "   • Build Tools $BUILD_TOOLS_VERSION"
echo "   • Platform Tools (ADB, Fastboot)"
echo "   • Android Emulator"
echo "   • System Image (Google APIs x86_64)"
echo ""
echo "🔄 To use the tools in your current session, run:"
echo "   source ~/.bashrc"
echo ""
echo "🚀 After sourcing, you can use:"
echo "   avdmanager list target"
echo "   sdkmanager --list"
echo "   adb version"
echo ""

# Step 10: Create a quick test AVD
echo "Creating a test AVD for verification..."
export PATH="$ANDROID_SDK_ROOT/cmdline-tools/latest/bin:$PATH"

echo no | "$ANDROID_SDK_ROOT/cmdline-tools/latest/bin/avdmanager" create avd \
    --force \
    --name "test_avd" \
    --abi "x86_64" \
    --package "system-images;android-$ANDROID_API_LEVEL;google_apis;x86_64" \
    --device "pixel_6" > /dev/null 2>&1

echo "✓ Test AVD 'test_avd' created"
echo ""
echo "📱 To list available AVDs: avdmanager list avd"
echo "🚀 To start the test emulator: emulator -avd test_avd"

echo ""
echo "=== Setup completed successfully! ==="
