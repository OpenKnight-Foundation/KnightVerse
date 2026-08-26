#!/bin/bash

# Game Contract Deployment Script
# This script deploys the XLMate Game Contract to Stellar Testnet
#
# SECURITY: The Stellar secret key is loaded from:
#   1. STELLAR_SECRET_KEY environment variable, or
#   2. .env file in this directory (STELLAR_SECRET_KEY=...), or
#   3. Interactive prompt (stdin) if neither is available.
#
# This avoids exposing the secret key in shell history or process lists.

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}🚀 XLMate Game Contract Deployment Script${NC}"
echo "=================================="

# Check if soroban-cli is installed
if ! command -v soroban &> /dev/null; then
    echo -e "${RED}❌ Soroban CLI is not installed. Installing...${NC}"
    cargo install --locked soroban-cli
    echo -e "${GREEN}✅ Soroban CLI installed successfully${NC}"
else
    echo -e "${GREEN}✅ Soroban CLI is already installed${NC}"
fi

# Check if WASM file exists
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WASM_FILE="$SCRIPT_DIR/target/wasm32-unknown-unknown/release/game_contract.wasm"
if [ ! -f "$WASM_FILE" ]; then
    echo -e "${YELLOW}🔨 Building contract...${NC}"
    cd "$SCRIPT_DIR" && cargo build --release --target wasm32-unknown-unknown
    echo -e "${GREEN}✅ Contract built successfully${NC}"
else
    echo -e "${GREEN}✅ Contract WASM file already exists${NC}"
fi

# ── Secure Secret Key Retrieval ──────────────────────────────────────────────
# Priority: 1) Environment variable  2) .env file  3) Interactive stdin

SECRET_KEY=""

# 1) Check STELLAR_SECRET_KEY environment variable
if [ -n "$STELLAR_SECRET_KEY" ]; then
    SECRET_KEY="$STELLAR_SECRET_KEY"
    echo -e "${GREEN}✅ Loaded secret key from STELLAR_SECRET_KEY environment variable${NC}"
fi

# 2) Try loading from .env file (if not already set)
if [ -z "$SECRET_KEY" ] && [ -f "$SCRIPT_DIR/.env" ]; then
    set -a
    source "$SCRIPT_DIR/.env"
    set +a
    if [ -n "$STELLAR_SECRET_KEY" ]; then
        SECRET_KEY="$STELLAR_SECRET_KEY"
        echo -e "${GREEN}✅ Loaded secret key from .env file${NC}"
    fi
fi

# 3) Fall back to interactive prompt (read -s hides input)
if [ -z "$SECRET_KEY" ]; then
    echo -e "${YELLOW}🔑 No secret key found in .env or environment.${NC}"
    echo -e "${YELLOW}   Enter your Stellar secret key (input will be hidden):${NC}"
    read -r -s -p "Secret Key: " SECRET_KEY
    echo ""
    if [ -z "$SECRET_KEY" ]; then
        echo -e "${RED}❌ No secret key provided. Aborting.${NC}"
        echo "  1. Copy .env.example to .env and edit your key:"
        echo "     cp $SCRIPT_DIR/.env.example $SCRIPT_DIR/.env"
        echo "  2. Set the STELLAR_SECRET_KEY variable in .env"
        exit 1
    fi
    echo -e "${GREEN}✅ Secret key received (not saved to disk)${NC}"
fi

# ── End Secure Key Retrieval ──────────────────────────────────────────────────

# Configure network
echo -e "${YELLOW}🌐 Configuring Stellar Testnet network...${NC}"
soroban config network --global testnet \
  --rpc-url https://soroban-testnet.stellar.org:443 \
  --network-passphrase "Test SDF Network ; September 2015"

echo -e "${GREEN}✅ Network configured${NC}"

# Deploy contract
echo -e "${YELLOW}🚀 Deploying contract to Stellar Testnet...${NC}"
CONTRACT_ID=$(soroban contract deploy \
  --wasm "$WASM_FILE" \
  --source "$SECRET_KEY" \
  --network testnet)

# Clear secret key from memory immediately after use
unset SECRET_KEY
SECRET_KEY=""

echo -e "${GREEN}✅ Contract deployed successfully!${NC}"
echo -e "${GREEN}📋 Contract ID: $CONTRACT_ID${NC}"

# Save contract ID to file
echo "$CONTRACT_ID" > "$SCRIPT_DIR/contract_id.txt"
echo -e "${GREEN}💾 Contract ID saved to contract_id.txt${NC}"

# Verify deployment
echo -e "${YELLOW}🔍 Verifying deployment...${NC}"
soroban contract info \
  --id "$CONTRACT_ID" \
  --network testnet

echo -e "${GREEN}✅ Deployment verified successfully!${NC}"
echo ""
echo -e "${GREEN}🎮 Next steps:${NC}"
echo "1. Use the contract ID in your frontend application"
echo "2. Test contract functions using soroban contract invoke"
echo "3. Check DEPLOYMENT.md for usage examples"
echo ""
echo -e "${YELLOW}⚠️  Remember: This is deployed to Testnet. Use test XLM only!${NC}"
