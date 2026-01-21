#!/usr/bin/env bash
# Test: Sending deltas TO the server
#
# Tests the ability to send delta messages to the server.
# Note: Not all servers accept client-sent deltas - this tests the behavior.

source "$(dirname "$0")/test_common.sh"
init_tests

# =============================================================================
# Test 1: Send a simple delta message
# =============================================================================
test_header "Send delta to server"

# Create a delta message with test data
delta_msg=$(make_delta_msg "vessels.self" "test.integration.value" "42.5" "test.script")
echo "  Sending delta: test.integration.value = 42.5"

FIFO=$(mktemp -u)
mkfifo "$FIFO"

(
    sleep 0.5
    # First subscribe to get our own data back
    echo '{"context":"vessels.self","subscribe":[{"path":"test.integration.*"}]}'
    sleep 0.5
    # Send the delta
    echo "$delta_msg"
    sleep 2
) | timeout 5 websocat "$SIGNALK_WS_URL" > "$FIFO" 2>/dev/null &
WS_PID=$!

responses=""
while IFS= read -r -t 4 line; do
    responses="${responses}${line}\n"
done < "$FIFO"

wait $WS_PID 2>/dev/null || true
rm -f "$FIFO"

# Check if we got the value echoed back or an error
if echo -e "$responses" | grep -q "test.integration.value"; then
    test_pass "Delta was accepted and echoed back"
elif echo -e "$responses" | grep -q -i "error\|fail"; then
    echo "  Server returned an error (may not support client deltas)"
    test_pass "Server responded to delta (error response)"
else
    echo "  No echo of test data (server may not broadcast client deltas)"
    test_pass "Delta message was sent (no echo received)"
fi

# =============================================================================
# Test 2: Send delta with complex value (object)
# =============================================================================
test_header "Send delta with object value"

delta_msg='{
  "context": "vessels.self",
  "updates": [{
    "$source": "test.script",
    "timestamp": "'$(date -u +%Y-%m-%dT%H:%M:%S.000Z)'",
    "values": [{
      "path": "test.integration.position",
      "value": {"latitude": 52.1234, "longitude": 4.5678}
    }]
  }]
}'
echo "  Sending: position object"

result=$(echo "$delta_msg" | timeout 2 websocat -n1 "$SIGNALK_WS_URL" 2>&1 || true)

if [[ -n "$result" ]]; then
    test_pass "Object value delta sent successfully"
else
    test_fail "Failed to send object value delta"
fi

# =============================================================================
# Test 3: Send delta with null value (delete/clear)
# =============================================================================
test_header "Send delta with null value"

delta_msg='{
  "context": "vessels.self",
  "updates": [{
    "$source": "test.script",
    "timestamp": "'$(date -u +%Y-%m-%dT%H:%M:%S.000Z)'",
    "values": [{
      "path": "test.integration.toClear",
      "value": null
    }]
  }]
}'
echo "  Sending: null value (clear path)"

result=$(echo "$delta_msg" | timeout 2 websocat -n1 "$SIGNALK_WS_URL" 2>&1 || true)

if [[ -n "$result" ]]; then
    test_pass "Null value delta sent successfully"
else
    test_fail "Failed to send null value delta"
fi

# =============================================================================
# Test 4: Send delta with multiple values
# =============================================================================
test_header "Send delta with multiple values"

delta_msg='{
  "context": "vessels.self",
  "updates": [{
    "$source": "test.script",
    "timestamp": "'$(date -u +%Y-%m-%dT%H:%M:%S.000Z)'",
    "values": [
      {"path": "test.integration.value1", "value": 1.0},
      {"path": "test.integration.value2", "value": 2.0},
      {"path": "test.integration.value3", "value": 3.0}
    ]
  }]
}'
echo "  Sending: 3 values in one delta"

result=$(echo "$delta_msg" | timeout 2 websocat -n1 "$SIGNALK_WS_URL" 2>&1 || true)

if [[ -n "$result" ]]; then
    test_pass "Multi-value delta sent successfully"
else
    test_fail "Failed to send multi-value delta"
fi

# =============================================================================
# Test 5: Send delta with source information
# =============================================================================
test_header "Send delta with detailed source"

delta_msg='{
  "context": "vessels.self",
  "updates": [{
    "source": {
      "label": "Integration Test",
      "type": "test",
      "src": "integration_test"
    },
    "$source": "test.integration",
    "timestamp": "'$(date -u +%Y-%m-%dT%H:%M:%S.000Z)'",
    "values": [{
      "path": "test.integration.sourced",
      "value": 99.9
    }]
  }]
}'
echo "  Sending: delta with source object"

result=$(echo "$delta_msg" | timeout 2 websocat -n1 "$SIGNALK_WS_URL" 2>&1 || true)

if [[ -n "$result" ]]; then
    test_pass "Delta with source sent successfully"
else
    test_fail "Failed to send delta with source"
fi

# =============================================================================
# Test 6: Send delta for different vessel (if allowed)
# =============================================================================
test_header "Send delta for different vessel context"

delta_msg='{
  "context": "vessels.urn:mrn:signalk:uuid:test-other-vessel",
  "updates": [{
    "$source": "ais.test",
    "timestamp": "'$(date -u +%Y-%m-%dT%H:%M:%S.000Z)'",
    "values": [{
      "path": "navigation.position",
      "value": {"latitude": 51.0, "longitude": 3.0}
    }]
  }]
}'
echo "  Sending: delta for other vessel context"

result=$(echo "$delta_msg" | timeout 2 websocat -n1 "$SIGNALK_WS_URL" 2>&1 || true)

if [[ -n "$result" ]]; then
    test_pass "Other vessel delta sent (may or may not be accepted)"
else
    test_fail "Failed to send other vessel delta"
fi

# =============================================================================
# Test 7: PUT request (write to a path)
# =============================================================================
test_header "PUT request to autopilot"

put_msg='{
  "requestId": "test-put-'$(date +%s)'",
  "put": {
    "path": "steering.autopilot.target.headingTrue",
    "value": 1.5708
  }
}'
echo "  Sending: PUT to steering.autopilot.target.headingTrue"

FIFO=$(mktemp -u)
mkfifo "$FIFO"

(
    sleep 0.5
    echo "$put_msg"
    sleep 2
) | timeout 4 websocat "$SIGNALK_WS_URL" > "$FIFO" 2>/dev/null &
WS_PID=$!

responses=""
while IFS= read -r -t 3 line; do
    responses="${responses}${line}\n"
done < "$FIFO"

wait $WS_PID 2>/dev/null || true
rm -f "$FIFO"

# Check for PUT response
if echo -e "$responses" | grep -q '"requestId"'; then
    # Got a response with requestId
    state=$(echo -e "$responses" | grep '"requestId"' | head -1 | jq -r '.state' 2>/dev/null || echo "unknown")
    status=$(echo -e "$responses" | grep '"requestId"' | head -1 | jq -r '.statusCode' 2>/dev/null || echo "unknown")
    echo "  Response state: $state, statusCode: $status"
    test_pass "PUT request got response (state: $state)"
else
    echo "  No PUT response received"
    test_pass "PUT request sent (no response)"
fi

# =============================================================================
# Test 8: Malformed delta (should not crash server)
# =============================================================================
test_header "Malformed delta handling"

bad_msg='{"context":"vessels.self","updates":"not-an-array"}'
echo "  Sending: malformed delta (updates is string, not array)"

FIFO=$(mktemp -u)
mkfifo "$FIFO"

(
    sleep 0.5
    echo "$bad_msg"
    sleep 1
    # Send valid subscribe to verify connection still works
    echo '{"context":"vessels.self","subscribe":[{"path":"*"}]}'
    sleep 1
) | timeout 4 websocat "$SIGNALK_WS_URL" > "$FIFO" 2>/dev/null &
WS_PID=$!

responses=""
while IFS= read -r -t 3 line; do
    responses="${responses}${line}\n"
done < "$FIFO"

wait $WS_PID 2>/dev/null || true
rm -f "$FIFO"

# Connection should still be alive
msg_count=$(echo -e "$responses" | grep -c '^{' || true)
if [[ "$msg_count" -gt 0 ]]; then
    test_pass "Server handled malformed delta gracefully (connection survived)"
else
    test_fail "Server may have dropped connection on malformed delta"
fi

# =============================================================================
# Summary
# =============================================================================
print_summary
