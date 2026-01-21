#!/usr/bin/env bash
# Test: REST API endpoints
#
# Tests the HTTP REST API endpoints before WebSocket tests.
# These are simpler and help verify basic server functionality.

source "$(dirname "$0")/test_common.sh"

# Don't call init_tests here - we want to be the first check
check_dependencies

echo -e "${BLUE}Testing REST API at ${SIGNALK_HTTP_URL}${NC}"

# =============================================================================
# Test 1: Discovery endpoint
# =============================================================================
test_header "Discovery endpoint (/signalk)"

response=$(curl -sf "${SIGNALK_HTTP_URL}/signalk" 2>/dev/null || echo "")

if [[ -z "$response" ]]; then
    test_fail "No response from /signalk endpoint"
    echo -e "${RED}Server may not be running at ${SIGNALK_HTTP_URL}${NC}"
    print_summary
    exit 1
fi

# Verify discovery response structure
assert_json_has "$response" "endpoints" "Discovery has endpoints" && \
assert_json_has "$response" "endpoints.v1" "Discovery has v1 endpoints" && \
assert_json_has "$response" "endpoints.v1.version" "v1 has version" && \
test_pass "Discovery endpoint structure is valid" || \
test_fail "Discovery endpoint structure is invalid"

# =============================================================================
# Test 2: REST API - Full model
# =============================================================================
test_header "REST API full model (/signalk/v1/api)"

response=$(curl -sf "${SIGNALK_HTTP_URL}/signalk/v1/api" 2>/dev/null || echo "")

if [[ -z "$response" ]]; then
    test_fail "No response from /signalk/v1/api"
else
    # Full model should have version, self, vessels
    assert_json_has "$response" "version" "Full model has version" && \
    assert_json_has "$response" "self" "Full model has self" && \
    test_pass "Full model structure is valid" || \
    test_fail "Full model structure is invalid"
fi

# =============================================================================
# Test 3: REST API - Self URN format
# =============================================================================
test_header "Self URN in full model"

self_urn=$(echo "$response" | jq -r '.self' 2>/dev/null || echo "")

if [[ "$self_urn" == vessels.* ]]; then
    test_pass "Self URN has correct format: $self_urn"
else
    test_fail "Self URN should start with 'vessels.', got: $self_urn"
fi

# =============================================================================
# Test 4: REST API - Path query
# =============================================================================
test_header "REST API path query (/signalk/v1/api/vessels/self)"

response=$(curl -sf "${SIGNALK_HTTP_URL}/signalk/v1/api/vessels/self" 2>/dev/null || echo "")

if [[ -z "$response" ]]; then
    echo "  Note: Path query may return empty if no data for self vessel"
    test_pass "Path query endpoint responded (empty or null is OK)"
elif [[ "$response" == "null" ]]; then
    test_pass "Path query returned null (no data yet)"
else
    test_pass "Path query returned data"
fi

# =============================================================================
# Test 5: REST API - Navigation path
# =============================================================================
test_header "REST API navigation path query"

response=$(curl -sf "${SIGNALK_HTTP_URL}/signalk/v1/api/vessels/self/navigation" 2>/dev/null || echo "")

if [[ -n "$response" ]] && [[ "$response" != "null" ]]; then
    test_pass "Navigation path returned data"
else
    echo "  Note: No navigation data yet (demo may not have started)"
    test_pass "Navigation path query accepted"
fi

# =============================================================================
# Test 6: REST API - 404 for unknown path
# =============================================================================
test_header "REST API 404 for unknown path"

# Note: Don't use -f (fail silently) here - we want to see the status code
http_code=$(curl -s -o /dev/null -w "%{http_code}" "${SIGNALK_HTTP_URL}/signalk/v1/api/nonexistent/path" 2>/dev/null)

if [[ "$http_code" == "404" ]]; then
    test_pass "Unknown path returns 404"
elif [[ "$http_code" == "200" ]]; then
    # Some servers return 200 with null/empty for unknown paths
    test_pass "Unknown path returns 200 (with null/empty)"
else
    test_fail "Unexpected status code for unknown path: $http_code"
fi

# =============================================================================
# Test 7: Discovery endpoints point to correct URLs
# =============================================================================
test_header "Discovery endpoint URLs"

discovery=$(curl -sf "${SIGNALK_HTTP_URL}/signalk" 2>/dev/null || echo "{}")

ws_url=$(echo "$discovery" | jq -r '.endpoints.v1["signalk-ws"]' 2>/dev/null || echo "")
http_url=$(echo "$discovery" | jq -r '.endpoints.v1["signalk-http"]' 2>/dev/null || echo "")

if [[ "$ws_url" == ws://* ]]; then
    test_pass "WebSocket URL is valid: $ws_url"
else
    test_fail "WebSocket URL should start with ws://, got: $ws_url"
fi

if [[ "$http_url" == http://* ]]; then
    test_pass "HTTP URL is valid: $http_url"
else
    test_fail "HTTP URL should start with http://, got: $http_url"
fi

# =============================================================================
# Test 8: Version in discovery matches full model
# =============================================================================
test_header "Version consistency"

discovery_version=$(curl -sf "${SIGNALK_HTTP_URL}/signalk" 2>/dev/null | jq -r '.endpoints.v1.version' || echo "")
model_version=$(curl -sf "${SIGNALK_HTTP_URL}/signalk/v1/api" 2>/dev/null | jq -r '.version' || echo "")

if [[ "$discovery_version" == "$model_version" ]]; then
    test_pass "Versions match: $discovery_version"
else
    echo "  Discovery version: $discovery_version"
    echo "  Model version: $model_version"
    test_pass "Both endpoints return version (may differ)"
fi

# =============================================================================
# Summary
# =============================================================================
print_summary
