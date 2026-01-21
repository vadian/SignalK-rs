#!/usr/bin/env bash
# Run all SignalK integration tests
#
# Usage:
#   ./run_all.sh                    # Test against localhost:4000
#   SIGNALK_HOST=192.168.1.100 ./run_all.sh  # Test against ESP32
#   SIGNALK_PORT=3000 ./run_all.sh  # Test against different port

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}╔═══════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║          SignalK Integration Tests                        ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "Target: ${GREEN}${SIGNALK_HOST:-localhost}:${SIGNALK_PORT:-4000}${NC}"
echo ""

# Check dependencies
if ! command -v websocat &> /dev/null; then
    echo -e "${RED}ERROR: websocat is not installed${NC}"
    echo "Install with: cargo install websocat"
    exit 1
fi

if ! command -v jq &> /dev/null; then
    echo -e "${RED}ERROR: jq is not installed${NC}"
    echo "Install with: apt install jq (or brew install jq)"
    exit 1
fi

# Check if server is running
HTTP_URL="http://${SIGNALK_HOST:-localhost}:${SIGNALK_PORT:-4000}"
if ! curl -sf "${HTTP_URL}/signalk" > /dev/null 2>&1; then
    echo -e "${RED}ERROR: SignalK server not responding at ${HTTP_URL}/signalk${NC}"
    echo ""
    echo "Start the server with:"
    echo "  make run           # Linux server"
    echo "  make run-esp       # ESP32 (dev)"
    echo ""
    exit 1
fi

echo -e "${GREEN}Server is running${NC}"
echo ""

# Track overall results
TOTAL_PASSED=0
TOTAL_FAILED=0
FAILED_TESTS=()

# Run each test script
for test_script in "$SCRIPT_DIR"/[0-9]*.sh; do
    if [[ -f "$test_script" ]]; then
        test_name=$(basename "$test_script" .sh)
        echo -e "\n${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
        echo -e "${BLUE}Running: ${test_name}${NC}"
        echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

        if bash "$test_script"; then
            TOTAL_PASSED=$((TOTAL_PASSED + 1))
        else
            TOTAL_FAILED=$((TOTAL_FAILED + 1))
            FAILED_TESTS+=("$test_name")
        fi
    fi
done

# Print overall summary
echo -e "\n${BLUE}╔═══════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                  Overall Summary                          ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "  Test scripts run: $((TOTAL_PASSED + TOTAL_FAILED))"
echo -e "  ${GREEN}Passed: ${TOTAL_PASSED}${NC}"

if [[ $TOTAL_FAILED -gt 0 ]]; then
    echo -e "  ${RED}Failed: ${TOTAL_FAILED}${NC}"
    echo ""
    echo -e "${RED}Failed tests:${NC}"
    for test in "${FAILED_TESTS[@]}"; do
        echo -e "  ${RED}- ${test}${NC}"
    done
    exit 1
else
    echo -e "  Failed: 0"
    echo ""
    echo -e "${GREEN}All tests passed!${NC}"
fi
