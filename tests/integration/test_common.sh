#!/usr/bin/env bash
# Common utilities for SignalK integration tests
#
# Source this file in test scripts:
#   source "$(dirname "$0")/test_common.sh"

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default server configuration
: "${SIGNALK_HOST:=localhost}"
: "${SIGNALK_PORT:=4000}"
: "${SIGNALK_WS_URL:=ws://${SIGNALK_HOST}:${SIGNALK_PORT}/signalk/v1/stream}"
: "${SIGNALK_HTTP_URL:=http://${SIGNALK_HOST}:${SIGNALK_PORT}}"
: "${TIMEOUT:=5}"

# Test counters
TESTS_PASSED=0
TESTS_FAILED=0
TESTS_TOTAL=0

# Check if websocat is installed
check_dependencies() {
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
}

# Check if server is running
check_server() {
    local url="${SIGNALK_HTTP_URL}/signalk"
    if ! curl -sf "$url" > /dev/null 2>&1; then
        echo -e "${RED}ERROR: SignalK server not responding at ${url}${NC}"
        echo "Start the server with: make run"
        return 1
    fi
    return 0
}

# Print test header
test_header() {
    local name="$1"
    echo -e "\n${BLUE}=== TEST: ${name} ===${NC}"
    TESTS_TOTAL=$((TESTS_TOTAL + 1))
}

# Assert equality
assert_eq() {
    local expected="$1"
    local actual="$2"
    local message="${3:-Values should be equal}"

    if [[ "$expected" == "$actual" ]]; then
        echo -e "${GREEN}  PASS: ${message}${NC}"
        return 0
    else
        echo -e "${RED}  FAIL: ${message}${NC}"
        echo -e "${RED}    Expected: ${expected}${NC}"
        echo -e "${RED}    Actual:   ${actual}${NC}"
        return 1
    fi
}

# Assert that JSON contains a field
assert_json_has() {
    local json="$1"
    local field="$2"
    local message="${3:-JSON should have field: ${field}}"

    if echo "$json" | jq -e ".$field" > /dev/null 2>&1; then
        echo -e "${GREEN}  PASS: ${message}${NC}"
        return 0
    else
        echo -e "${RED}  FAIL: ${message}${NC}"
        echo -e "${RED}    Field '$field' not found in: ${json:0:200}...${NC}"
        return 1
    fi
}

# Assert JSON field equals value
assert_json_eq() {
    local json="$1"
    local field="$2"
    local expected="$3"
    local message="${4:-JSON field $field should equal $expected}"

    local actual
    actual=$(echo "$json" | jq -r ".$field" 2>/dev/null || echo "null")

    if [[ "$actual" == "$expected" ]]; then
        echo -e "${GREEN}  PASS: ${message}${NC}"
        return 0
    else
        echo -e "${RED}  FAIL: ${message}${NC}"
        echo -e "${RED}    Expected: ${expected}${NC}"
        echo -e "${RED}    Actual:   ${actual}${NC}"
        return 1
    fi
}

# Assert string contains substring
assert_contains() {
    local haystack="$1"
    local needle="$2"
    local message="${3:-String should contain: ${needle}}"

    if [[ "$haystack" == *"$needle"* ]]; then
        echo -e "${GREEN}  PASS: ${message}${NC}"
        return 0
    else
        echo -e "${RED}  FAIL: ${message}${NC}"
        echo -e "${RED}    Needle: ${needle}${NC}"
        echo -e "${RED}    Haystack: ${haystack:0:200}...${NC}"
        return 1
    fi
}

# Mark test as passed
test_pass() {
    local message="${1:-Test passed}"
    echo -e "${GREEN}  PASS: ${message}${NC}"
    TESTS_PASSED=$((TESTS_PASSED + 1))
}

# Mark test as failed
test_fail() {
    local message="${1:-Test failed}"
    echo -e "${RED}  FAIL: ${message}${NC}"
    TESTS_FAILED=$((TESTS_FAILED + 1))
}

# Print test summary
print_summary() {
    echo -e "\n${BLUE}========================================${NC}"
    echo -e "${BLUE}  Test Summary${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo -e "  Total:  ${TESTS_TOTAL}"
    echo -e "  ${GREEN}Passed: ${TESTS_PASSED}${NC}"
    if [[ $TESTS_FAILED -gt 0 ]]; then
        echo -e "  ${RED}Failed: ${TESTS_FAILED}${NC}"
    else
        echo -e "  Failed: ${TESTS_FAILED}"
    fi
    echo -e "${BLUE}========================================${NC}"

    if [[ $TESTS_FAILED -gt 0 ]]; then
        return 1
    fi
    return 0
}

# Connect to WebSocket and get first message (hello)
# Uses timeout to prevent hanging if server doesn't close connection
ws_get_hello() {
    local url="${1:-$SIGNALK_WS_URL}"
    local timeout_sec="${2:-5}"
    local tmpfile
    tmpfile=$(mktemp)
    # Use temp file to avoid SIGPIPE issues with pipes
    timeout "$timeout_sec" websocat "$url" > "$tmpfile" 2>/dev/null &
    local pid=$!
    # Wait a moment for first message, then kill
    sleep 0.5
    kill $pid 2>/dev/null || true
    wait $pid 2>/dev/null || true
    head -1 "$tmpfile"
    rm -f "$tmpfile"
}

# Connect with query parameters and get hello
ws_get_hello_with_params() {
    local params="$1"
    local timeout_sec="${2:-5}"
    local url="${SIGNALK_WS_URL}?${params}"
    local tmpfile
    tmpfile=$(mktemp)
    timeout "$timeout_sec" websocat "$url" > "$tmpfile" 2>/dev/null &
    local pid=$!
    sleep 0.5
    kill $pid 2>/dev/null || true
    wait $pid 2>/dev/null || true
    head -1 "$tmpfile"
    rm -f "$tmpfile"
}

# Send a message and wait for response
# Usage: echo '{"subscribe":[...]}' | ws_send_recv
ws_send_recv() {
    local timeout="${1:-$TIMEOUT}"
    timeout "$timeout" websocat "$SIGNALK_WS_URL" 2>/dev/null || true
}

# Run websocat with a sequence of sends/receives
# Reads from stdin, outputs responses
ws_session() {
    local url="${1:-$SIGNALK_WS_URL}"
    websocat "$url" 2>/dev/null
}

# Create a subscribe message JSON
make_subscribe_msg() {
    local context="$1"
    local path="$2"
    local period="${3:-}"
    local min_period="${4:-}"

    local sub="{\"path\":\"$path\""
    [[ -n "$period" ]] && sub="$sub,\"period\":$period"
    [[ -n "$min_period" ]] && sub="$sub,\"minPeriod\":$min_period"
    sub="$sub}"

    echo "{\"context\":\"$context\",\"subscribe\":[$sub]}"
}

# Create an unsubscribe message JSON
make_unsubscribe_msg() {
    local context="$1"
    local path="$2"

    echo "{\"context\":\"$context\",\"unsubscribe\":[{\"path\":\"$path\"}]}"
}

# Create a delta message JSON
make_delta_msg() {
    local context="$1"
    local path="$2"
    local value="$3"
    local source="${4:-test.script}"

    cat <<EOF
{
  "context": "$context",
  "updates": [{
    "\$source": "$source",
    "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%S.000Z)",
    "values": [{
      "path": "$path",
      "value": $value
    }]
  }]
}
EOF
}

# Initialize - call at start of each test script
init_tests() {
    check_dependencies
    if ! check_server; then
        echo -e "${YELLOW}Skipping tests - server not running${NC}"
        exit 0
    fi
    echo -e "${GREEN}Server is running at ${SIGNALK_HTTP_URL}${NC}"
}
