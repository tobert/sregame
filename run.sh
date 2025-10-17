#!/bin/bash
# Launch script for Windows build from WSL

# Get to the project directory
cd "$(dirname "$0")"

# Determine build directory
BUILD_DIR="target/x86_64-pc-windows-msvc/debug"

# Check if exe exists
if [ ! -f "$BUILD_DIR/sregame.exe" ]; then
    echo "Error: sregame.exe not found in $BUILD_DIR"
    echo "Run 'cargo build --target x86_64-pc-windows-msvc' first"
    exit 1
fi

# Always sync assets to build directory
echo "Syncing assets to build directory..."
rsync -a --delete assets/ "$BUILD_DIR/assets/"
echo "Assets synced."

# Launch the game
echo "Launching The Endgame of SRE..."
cd "$BUILD_DIR"
./sregame.exe
