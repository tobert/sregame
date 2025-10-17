#!/bin/bash
# Sync Windows build (exe + assets) from WSL to Windows machine

# Configuration
REMOTE_HOST="192.168.1.4"
REMOTE_BASE="/home/atobey/src/sregame"
BUILD_DIR="target/x86_64-pc-windows-msvc/debug"
LOCAL_PATH="."

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Syncing Windows build from WSL...${NC}"

# Sync the exe
echo "Syncing sregame.exe..."
rsync -ave ssh \
  "${REMOTE_HOST}:${REMOTE_BASE}/${BUILD_DIR}/sregame.exe" \
  "${LOCAL_PATH}/"

# Sync the assets
echo "Syncing assets..."
rsync -ave ssh \
  --delete \
  "${REMOTE_HOST}:${REMOTE_BASE}/${BUILD_DIR}/assets/" \
  "${LOCAL_PATH}/assets/"

# Sync any DLLs if present
echo "Syncing DLLs (if any)..."
rsync -ave ssh \
  --include='*.dll' \
  --exclude='*' \
  "${REMOTE_HOST}:${REMOTE_BASE}/${BUILD_DIR}/" \
  "${LOCAL_PATH}/" 2>/dev/null || true

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Sync completed successfully!${NC}"
    echo -e "${GREEN}Run ./sregame.exe to play${NC}"
else
    echo "✗ Sync failed"
    exit 1
fi
