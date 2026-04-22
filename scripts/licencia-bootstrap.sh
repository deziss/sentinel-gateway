#!/bin/bash
set -e

# =============================================================================
# Sentinel Gateway — Licencia Bootstrap Script
# Helps configure the connection to the Licencia platform.
# =============================================================================

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${GREEN}Sentinel Gateway — Licencia Bootstrap${NC}"
echo "------------------------------------------------"

# 1. Ask for Licencia URL
read -p "Enter Licencia Platform URL (default: http://localhost:8000): " LICENCIA_URL
LICENCIA_URL=${LICENCIA_URL:-http://localhost:8000}

# 2. Ask for API Key
read -p "Enter Licencia Master API Key: " LICENCIA_API_KEY

if [ -z "$LICENCIA_API_KEY" ]; then
    echo -e "${RED}Error: API Key is required for SaaS mode.${NC}"
    exit 1
fi

# 3. Ask for Deployment Mode
echo "Select Deployment Mode:"
echo "1) Platform (SaaS / Multi-tenant)"
echo "2) PaaS (Self-hosted / Single-tenant unlocked)"
echo "3) Community (OSS / Restricted)"
read -p "Selection [1-3]: " MODE_SEL

case $MODE_SEL in
    1) DEPLOYMENT_MODE="platform";;
    2) DEPLOYMENT_MODE="paas";;
    3) DEPLOYMENT_MODE="local";;
    *) DEPLOYMENT_MODE="platform";;
esac

# 4. Generate Instance ID if not present
INSTANCE_ID=$(uuidgen 2>/dev/null || cat /proc/sys/kernel/random/uuid)

# 5. Save to .env
echo "Saving configuration to .env..."

cat <<EOF > .env
# Sentinel Gateway Configuration
DEPLOYMENT_MODE=$DEPLOYMENT_MODE
LICENCIA_URL=$LICENCIA_URL
LICENCIA_API_KEY=$LICENCIA_API_KEY
INSTANCE_ID=$INSTANCE_ID

# For PaaS mode bypass
# SHA-256 of "sentinel-paas:$INSTANCE_ID"
DEVELOPER_SECRET=$(echo -n "sentinel-paas:$INSTANCE_ID" | sha256sum | awk '{print $1}')
EOF

echo -e "${GREEN}Success! Configuration saved to .env${NC}"
echo "You can now run: docker compose -f docker-compose.yml -f docker-compose.saas.yml up -d"
