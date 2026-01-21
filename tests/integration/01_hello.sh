#!/usr/bin/env bash
# Test: Hello message on WebSocket connect
#
# Verifies that the server sends a proper hello message when a client connects.
# The hello message must contain: name, version, self, roles, timestamp

source "$(dirname "$0")/test_common.sh"
init_tests

# =============================================================================
# Test 1: Basic hello message
# =============================================================================
test_header "Hello message on connect"

hello=$(ws_get_hello)

if [[ -z "$hello" ]]; then
    test_fail "No hello message received"
else
    # Verify required fields
    assert_json_has "$hello" "name" "Hello has 'name' field" && \
    assert_json_has "$hello" "version" "Hello has 'version' field" && \
    assert_json_has "$hello" "self" "Hello has 'self' field" && \
    assert_json_has "$hello" "roles" "Hello has 'roles' field" && \
    assert_json_has "$hello" "timestamp" "Hello has 'timestamp' field" && \
    test_pass "Hello message structure is valid" || \
    test_fail "Hello message structure is invalid"
fi

# =============================================================================
# Test 2: Self URN format
# =============================================================================
test_header "Self URN format"

self_urn=$(echo "$hello" | jq -r '.self')

# Self must start with "vessels."
if [[ "$self_urn" == vessels.* ]]; then
    test_pass "Self URN starts with 'vessels.'"
else
    test_fail "Self URN should start with 'vessels.', got: $self_urn"
fi

# =============================================================================
# Test 3: Version format
# =============================================================================
test_header "Version format"

version=$(echo "$hello" | jq -r '.version')

# Version should be semver-ish (e.g., "1.7.0")
if [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    test_pass "Version is valid semver: $version"
else
    test_fail "Version should be semver, got: $version"
fi

# =============================================================================
# Test 4: Roles array
# =============================================================================
test_header "Roles array"

roles_count=$(echo "$hello" | jq '.roles | length')

if [[ "$roles_count" -gt 0 ]]; then
    test_pass "Roles array is non-empty (count: $roles_count)"
else
    test_fail "Roles array should not be empty"
fi

# =============================================================================
# Test 5: Timestamp format (ISO 8601)
# =============================================================================
test_header "Timestamp format"

timestamp=$(echo "$hello" | jq -r '.timestamp')

# Should match ISO 8601 format: YYYY-MM-DDTHH:MM:SS.sssZ
if [[ "$timestamp" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]{3}Z$ ]]; then
    test_pass "Timestamp is valid ISO 8601: $timestamp"
else
    test_fail "Timestamp should be ISO 8601, got: $timestamp"
fi

# =============================================================================
# Summary
# =============================================================================
print_summary
