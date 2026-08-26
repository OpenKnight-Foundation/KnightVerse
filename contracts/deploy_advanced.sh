#!/bin/bash

# Advanced Soroban Deployment Script with Rollback Support
# This script provides production-ready deployment with verification and rollback capabilities

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEPLOY_LOG="$SCRIPT_DIR/deployment-history.log"
CONTRACTS_DIR="$SCRIPT_DIR/.."
BACKUP_DIR="$SCRIPT_DIR/backups"
MAX_BACKUPS=10

# Network configuration
declare -A NETWORK_CONFIG
NETWORK_CONFIG[testnet_rpc]="https://soroban-testnet.stellar.org:443"
NETWORK_CONFIG[testnet_passphrase]="Test SDF Network ; September 2015"
NETWORK_CONFIG[futurenet_rpc]="https://rpc-futurenet.stellar.org:443"
NETWORK_CONFIG[futurenet_passphrase]="Test SDF Future Network ; October 2022"

echo -e "${GREEN}🚀 XLMate Advanced Soroban Deployment Script${NC}"
echo "================================================"

# Function to log messages
log_message() {
    local level=$1
    shift
    local message="$@"
    local timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    echo -e "${timestamp} [${level}] ${message}" | tee -a "$DEPLOY_LOG"
}

# Function to setup network
setup_network() {
    local network=$1
    log_message "INFO" "Configuring network: $network"
    
    local rpc_url="${NETWORK_CONFIG[${network}_rpc]}"
    local passphrase="${NETWORK_CONFIG[${network}_passphrase]}"
    
    soroban config network add "$network" \
        --rpc-url "$rpc_url" \
        --network-passphrase "$passphrase"
    
    log_message "INFO" "Network $network configured successfully"
}

# Function to create backup of current deployment
create_backup() {
    local network=$1
    local contract_name=$2
    local current_contract_id=$3
    
    mkdir -p "$BACKUP_DIR"
    
    local backup_file="$BACKUP_DIR/${contract_name}_${network}_$(date +%Y%m%d_%H%M%S).backup"
    
    cat > "$backup_file" << EOF
contract_name=$contract_name
network=$network
contract_id=$current_contract_id
timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
git_commit=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
EOF
    
    log_message "INFO" "Backup created: $backup_file"
    
    # Clean old backups
    cleanup_old_backups "$contract_name" "$network"
}

# Function to cleanup old backups
cleanup_old_backups() {
    local contract_name=$1
    local network=$2
    
    local backup_count=$(ls -1 "$BACKUP_DIR"/${contract_name}_${network}_*.backup 2>/dev/null | wc -l)
    
    if [ "$backup_count" -gt "$MAX_BACKUPS" ]; then
        local to_delete=$((backup_count - MAX_BACKUPS))
        ls -1t "$BACKUP_DIR"/${contract_name}_${network}_*.backup | tail -n "$to_delete" | xargs rm -f
        log_message "INFO" "Cleaned up $to_delete old backup(s)"
    fi
}

# Function to rollback to previous deployment
rollback() {
    local network=$1
    local contract_name=$2
    
    log_message "WARN" "Initiating rollback for $contract_name on $network"
    
    # Find latest backup
    local latest_backup=$(ls -1t "$BACKUP_DIR"/${contract_name}_${network}_*.backup 2>/dev/null | head -n 1)
    
    if [ -z "$latest_backup" ]; then
        log_message "ERROR" "No backup found for rollback"
        exit 1
    fi
    
    log_message "INFO" "Rolling back to: $latest_backup"
    
    # Source backup file
    source "$latest_backup"
    
    echo -e "${YELLOW}🔄 Rolling back $contract_name to $contract_id${NC}"
    log_message "INFO" "Rollback complete. Contract ID: $contract_id"
    
    echo -e "${GREEN}✅ Rollback successful${NC}"
}

# Function to verify deployment
verify_deployment() {
    local network=$1
    local contract_id=$2
    local contract_name=$3
    
    log_message "INFO" "Verifying deployment of $contract_name ($contract_id)"
    
    # Check if contract is accessible
    local attempts=0
    local max_attempts=5
    
    while [ $attempts -lt $max_attempts ]; do
        if soroban contract info --id "$contract_id" --network "$network" >/dev/null 2>&1; then
            log_message "INFO" "Contract verification successful"
            echo -e "${GREEN}✅ Contract verified on $network${NC}"
            return 0
        fi
        
        attempts=$((attempts + 1))
        log_message "WARN" "Verification attempt $attempts/$max_attempts failed, retrying..."
        sleep 3
    done
    
    log_message "ERROR" "Contract verification failed after $max_attempts attempts"
    return 1
}

# Function to get current contract ID (if deployed)
get_current_contract_id() {
    local network=$1
    local contract_name=$2
    local contract_file="$SCRIPT_DIR/.contract-${contract_name}-${network}.id"
    
    if [ -f "$contract_file" ]; then
        cat "$contract_file"
    else
        echo ""
    fi
}

# Function to save contract ID
save_contract_id() {
    local network=$1
    local contract_name=$2
    local contract_id=$3
    local contract_file="$SCRIPT_DIR/.contract-${contract_name}-${network}.id"
    
    echo "$contract_id" > "$contract_file"
    log_message "INFO" "Contract ID saved to $contract_file"
}

# Function to deploy a single contract
deploy_contract() {
    local network=$1
    local contract_name=$2
    local wasm_file="$CONTRACTS_DIR/target/wasm32-unknown-unknown/release/${contract_name}.wasm"
    local identity=$3
    
    echo -e "${BLUE}📦 Deploying contract: $contract_name${NC}"
    
    # Check if WASM file exists
    if [ ! -f "$wasm_file" ]; then
        log_message "ERROR" "WASM file not found: $wasm_file"
        echo -e "${RED}❌ Build the contract first: cargo build --release --target wasm32-unknown-unknown${NC}"
        return 1
    fi
    
    # Get current contract ID for backup
    local current_id=$(get_current_contract_id "$network" "$contract_name")
    
    if [ -n "$current_id" ]; then
        log_message "INFO" "Current deployment: $current_id"
        create_backup "$network" "$contract_name" "$current_id"
    fi
    
    # Deploy contract
    log_message "INFO" "Deploying $contract_name to $network..."
    
    local contract_id
    contract_id=$(soroban contract deploy \
        --wasm "$wasm_file" \
        --source "$identity" \
        --network "$network" 2>&1)
    
    if [ $? -eq 0 ]; then
        log_message "INFO" "Deployment successful: $contract_id"
        echo -e "${GREEN}✅ Contract deployed: $contract_id${NC}"
        
        # Save contract ID
        save_contract_id "$network" "$contract_name" "$contract_id"
        
        # Verify deployment
        if verify_deployment "$network" "$contract_id" "$contract_name"; then
            return 0
        else
            log_message "WARN" "Verification failed, initiating rollback..."
            rollback "$network" "$contract_name"
            return 1
        fi
    else
        log_message "ERROR" "Deployment failed: $contract_id"
        echo -e "${RED}❌ Deployment failed${NC}"
        return 1
    fi
}

# Function to build contracts
build_contracts() {
    echo -e "${YELLOW}🔨 Building contracts...${NC}"
    log_message "INFO" "Starting contract build"
    
    cd "$CONTRACTS_DIR"
    cargo build --release --target wasm32-unknown-unknown
    
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✅ Contracts built successfully${NC}"
        log_message "INFO" "Build completed successfully"
    else
        echo -e "${RED}❌ Build failed${NC}"
        log_message "ERROR" "Build failed"
        exit 1
    fi
}

# Function to show deployment history
show_history() {
    echo -e "${BLUE}📜 Deployment History:${NC}"
    echo "--------------------------------"
    
    if [ -f "$DEPLOY_LOG" ]; then
        tail -n 20 "$DEPLOY_LOG"
    else
        echo "No deployment history found"
    fi
}

# Function to list backups
list_backups() {
    echo -e "${BLUE}📦 Available Backups:${NC}"
    echo "--------------------------------"
    
    if [ -d "$BACKUP_DIR" ]; then
        ls -lh "$BACKUP_DIR"/*.backup 2>/dev/null || echo "No backups found"
    else
        echo "No backup directory found"
    fi
}

# Main execution
main() {
    local action=${1:-"deploy"}
    local network=${2:-"testnet"}
    local contract_name=${3:-""}
    local identity=${4:-"testnet-deployer"}
    
    case "$action" in
        deploy)
            echo -e "${YELLOW}🌐 Network: $network${NC}"
            
            # Setup network
            setup_network "$network"
            
            # Build contracts if not specified
            if [ -z "$contract_name" ]; then
                build_contracts
                
                # Deploy all contracts
                for wasm_file in "$CONTRACTS_DIR"/target/wasm32-unknown-unknown/release/*.wasm; do
                    if [ -f "$wasm_file" ]; then
                        local name=$(basename "$wasm_file" .wasm)
                        deploy_contract "$network" "$name" "$identity"
                    fi
                done
            else
                # Build specific contract
                cd "$CONTRACTS_DIR"
                cargo build --release --target wasm32-unknown-unknown -p "$contract_name"
                
                # Deploy specific contract
                deploy_contract "$network" "$contract_name" "$identity"
            fi
            
            echo -e "${GREEN}🎉 Deployment complete!${NC}"
            ;;
        
        rollback)
            if [ -z "$contract_name" ]; then
                echo -e "${RED}❌ Please specify contract name for rollback${NC}"
                echo "Usage: $0 rollback <network> <contract_name> [identity]"
                exit 1
            fi
            rollback "$network" "$contract_name"
            ;;
        
        verify)
            if [ -z "$contract_name" ]; then
                echo -e "${RED}❌ Please specify contract name to verify${NC}"
                exit 1
            fi
            
            local contract_id=$(get_current_contract_id "$network" "$contract_name")
            if [ -z "$contract_id" ]; then
                echo -e "${RED}❌ No deployment found for $contract_name on $network${NC}"
                exit 1
            fi
            
            verify_deployment "$network" "$contract_id" "$contract_name"
            ;;
        
        history)
            show_history
            ;;
        
        backups)
            list_backups
            ;;
        
        build)
            build_contracts
            ;;
        
        *)
            echo -e "${YELLOW}Usage:${NC}"
            echo "  $0 deploy [network] [contract_name] [identity]  - Deploy contracts"
            echo "  $0 rollback [network] [contract_name]           - Rollback to previous deployment"
            echo "  $0 verify [network] [contract_name]             - Verify deployment"
            echo "  $0 history                                      - Show deployment history"
            echo "  $0 backups                                      - List available backups"
            echo "  $0 build                                        - Build all contracts"
            echo ""
            echo -e "${YELLOW}Networks:${NC} testnet, futurenet"
            echo -e "${YELLOW}Example:${NC} $0 deploy testnet game_contract testnet-deployer"
            ;;
    esac
}

# Run main function
main "$@"
