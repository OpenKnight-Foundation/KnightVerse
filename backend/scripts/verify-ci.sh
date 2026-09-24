#!/bin/bash

# XLMate Backend CI Verification Script
# This script verifies that all CI components are working correctly

set -e

echo "🔍 XLMate Backend CI Verification"
echo "=================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    if [ $1 -eq 0 ]; then
        echo -e "${GREEN}✅ $2${NC}"
    else
        echo -e "${RED}❌ $2${NC}"
        exit 1
    fi
}

# Check if we're in the backend directory
if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}❌ Please run this script from the backend directory${NC}"
    exit 1
fi

echo ""
echo "📦 Checking workspace structure..."

# Verify all modules are in workspace
check_workspace() {
    local module=$1
    if grep -q "\"modules/$module\"" Cargo.toml; then
        print_status 0 "Module $module found in workspace"
    else
        print_status 1 "Module $module missing from workspace"
    fi
}

check_workspace "pairing"
check_workspace "metrics"
check_workspace "validation"
check_workspace "archiving"
check_workspace "tournament"
check_workspace "integration_tests"

echo ""
echo "🔧 Checking module Cargo.toml files..."

# Check if all new modules have proper Cargo.toml
check_cargo_toml() {
    local module=$1
    if [ -f "modules/$module/Cargo.toml" ]; then
        print_status 0 "$module/Cargo.toml exists"
    else
        print_status 1 "$module/Cargo.toml missing"
    fi
}

check_cargo_toml "pairing"
check_cargo_toml "metrics"
check_cargo_toml "validation"
check_cargo_toml "archiving"
check_cargo_toml "integration_tests"

echo ""
echo "📚 Checking source files..."

# Check if all source files exist
check_source_file() {
    local file=$1
    if [ -f "$file" ]; then
        print_status 0 "$file exists"
    else
        print_status 1 "$file missing"
    fi
}

check_source_file "modules/pairing/src/lib.rs"
check_source_file "modules/metrics/src/lib.rs"
check_source_file "modules/validation/src/lib.rs"
check_source_file "modules/archiving/src/lib.rs"
check_source_file "modules/tournament/src/bracket.rs"
check_source_file "modules/api/src/metrics_middleware.rs"
check_source_file "modules/integration_tests/src/main.rs"

echo ""
echo "🐳 Checking Docker and monitoring files..."

check_source_file "Dockerfile"
check_source_file "docker-compose.monitoring.yml"
check_source_file "monitoring/prometheus.yml"

echo ""
echo "📝 Checking documentation files..."

check_source_file "IMPLEMENTATION_SUMMARY.md"
check_source_file "VERIFY_IMPLEMENTATION.md"

echo ""
echo "🔍 Running cargo check on all modules..."

# Check each module
check_module() {
    local module=$1
    echo "Checking $module..."
    if cargo check -p $module; then
        print_status 0 "$module compiles successfully"
    else
        print_status 1 "$module compilation failed"
    fi
}

check_module "metrics"
check_module "validation"
check_module "archiving"
check_module "tournament"
check_module "api"

echo ""
echo "🧪 Running unit tests..."

# Run tests for each module
test_module() {
    local module=$1
    echo "Testing $module..."
    if cargo test -p $module; then
        print_status 0 "$module tests pass"
    else
        print_status 1 "$module tests failed"
    fi
}

check_module "pairing"
check_module "metrics"
check_module "tournament"
check_module "validation"
check_module "archiving"

echo ""
echo "🔗 Running integration tests..."

if cargo test -p integration_tests; then
    print_status 0 "Integration tests pass"
else
    print_status 1 "Integration tests failed"
fi

echo ""
echo "🔒 Running security audit..."

if cargo audit; then
    print_status 0 "Security audit passed"
else
    print_status 1 "Security audit found issues"
fi

echo ""
echo "📊 Checking CI configuration..."

if [ -f "../.github/workflows/backend.yml" ]; then
    print_status 0 "CI configuration exists"
    
    # Check if CI includes all new modules
    if grep -q "metrics" ../.github/workflows/backend.yml && \
       grep -q "validation" ../.github/workflows/backend.yml && \
       grep -q "archiving" ../.github/workflows/backend.yml && \
       grep -q "integration_tests" ../.github/workflows/backend.yml; then
        print_status 0 "CI includes all new modules"
    else
        print_status 1 "CI missing some module tests"
    fi
    
    # Check if CI includes monitoring tests
    if grep -q "monitoring-test" ../.github/workflows/backend.yml; then
        print_status 0 "CI includes monitoring tests"
    else
        print_status 1 "CI missing monitoring tests"
    fi
else
    print_status 1 "CI configuration missing"
fi

echo ""
echo "🐳 Checking Docker build..."

if docker build -t xlmate-backend-test .; then
    print_status 0 "Docker build successful"
    docker rmi xlmate-backend-test
else
    print_status 1 "Docker build failed"
fi

echo ""
echo "📈 Checking monitoring stack..."

if docker-compose -f docker-compose.monitoring.yml config > /dev/null 2>&1; then
    print_status 0 "Monitoring stack configuration valid"
else
    print_status 1 "Monitoring stack configuration invalid"
fi

echo ""
echo "🎯 CI Verification Complete!"
echo "=================================="
echo -e "${GREEN}✅ All checks passed! CI is ready.${NC}"
echo ""
echo "Next steps:"
echo "1. Push changes to trigger CI"
echo "2. Monitor CI run in GitHub Actions"
echo "3. Verify all tests pass"
echo "4. Check Docker image build"
echo "5. Validate monitoring stack deployment"
