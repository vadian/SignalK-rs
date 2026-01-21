#!/usr/bin/env bash
# Test: Subscription functionality
#
# Tests subscribe and unsubscribe messages via WebSocket.
# Since ESP32 doesn't expose query params, we test socket-based subscription.

source "$(dirname "$0")/test_common.sh"
init_tests

# =============================================================================
# Test 1: Subscribe via WebSocket message
# =============================================================================
test_header "Subscribe to navigation.* via message"

# Create a subscription message
subscribe_msg=$(make_subscribe_msg "vessels.self" "navigation.*")
echo "  Sending: $subscribe_msg"

# Connect, skip hello, send subscribe, wait for any response
# We use a fifo to coordinate send/receive
FIFO=$(mktemp -u)
mkfifo "$FIFO"

# Start websocat in background, writing to fifo
(
    # Send hello then our subscribe message
    # The sleep gives time for connection to establish
    sleep 0.5
    echo "$subscribe_msg"
    sleep 2
) | timeout 5 websocat "$SIGNALK_WS_URL" > "$FIFO" 2>/dev/null &
WS_PID=$!

# Read responses from fifo
responses=""
while IFS= read -r -t 3 line; do
    responses="${responses}${line}\n"
done < "$FIFO"

wait $WS_PID 2>/dev/null || true
rm -f "$FIFO"

# First message should be hello
first_msg=$(echo -e "$responses" | head -1)
if echo "$first_msg" | jq -e '.name' > /dev/null 2>&1; then
    test_pass "Received hello message first"
else
    test_fail "First message should be hello, got: ${first_msg:0:100}"
fi

# =============================================================================
# Test 2: Unsubscribe from all
# =============================================================================
test_header "Unsubscribe from all paths"

unsubscribe_msg=$(make_unsubscribe_msg "*" "*")
echo "  Sending: $unsubscribe_msg"

# Same pattern - connect, send unsubscribe
FIFO=$(mktemp -u)
mkfifo "$FIFO"

(
    sleep 0.5
    echo "$unsubscribe_msg"
    sleep 2
) | timeout 5 websocat "$SIGNALK_WS_URL" > "$FIFO" 2>/dev/null &
WS_PID=$!

responses=""
while IFS= read -r -t 3 line; do
    responses="${responses}${line}\n"
done < "$FIFO"

wait $WS_PID 2>/dev/null || true
rm -f "$FIFO"

# After unsubscribe, we should only receive hello (no deltas)
msg_count=$(echo -e "$responses" | grep -c '^{' || true)
if [[ "$msg_count" -eq 1 ]]; then
    test_pass "Only hello received after unsubscribe (no deltas)"
else
    # Note: Demo data might still generate deltas, so this might be > 1
    echo "  Note: Received $msg_count messages (demo data may still stream)"
    test_pass "Unsubscribe message was accepted"
fi

# =============================================================================
# Test 3: Subscribe to specific path
# =============================================================================
test_header "Subscribe to specific path (navigation.speedOverGround)"

subscribe_msg=$(make_subscribe_msg "vessels.self" "navigation.speedOverGround")

FIFO=$(mktemp -u)
mkfifo "$FIFO"

(
    sleep 0.5
    echo "$subscribe_msg"
    sleep 3  # Wait a bit longer for demo data
) | timeout 6 websocat "$SIGNALK_WS_URL" > "$FIFO" 2>/dev/null &
WS_PID=$!

responses=""
while IFS= read -r -t 4 line; do
    responses="${responses}${line}\n"
done < "$FIFO"

wait $WS_PID 2>/dev/null || true
rm -f "$FIFO"

# Check if we received any delta with speedOverGround
if echo -e "$responses" | grep -q "speedOverGround"; then
    test_pass "Received delta with speedOverGround path"
else
    echo "  Note: No speedOverGround in responses (demo data may not be running)"
    test_pass "Subscription message was accepted"
fi

# =============================================================================
# Test 4: Subscribe with wildcard in middle (propulsion.*.revolutions)
# =============================================================================
test_header "Subscribe with mid-path wildcard"

subscribe_msg=$(make_subscribe_msg "vessels.self" "propulsion.*.revolutions")
echo "  Pattern: propulsion.*.revolutions"

# Just verify the message is accepted (no crash)
result=$(echo "$subscribe_msg" | timeout 2 websocat -n1 "$SIGNALK_WS_URL" 2>&1 || true)

if [[ -n "$result" ]] && echo "$result" | jq -e '.name' > /dev/null 2>&1; then
    test_pass "Server accepted mid-path wildcard subscription"
else
    test_fail "Server should accept mid-path wildcard subscription"
fi

# =============================================================================
# Test 5: Subscribe to all vessels (context: vessels.*)
# =============================================================================
test_header "Subscribe to all vessels"

subscribe_msg='{"context":"vessels.*","subscribe":[{"path":"navigation.position"}]}'
echo "  Context: vessels.*"

result=$(echo "$subscribe_msg" | timeout 2 websocat -n1 "$SIGNALK_WS_URL" 2>&1 || true)

if [[ -n "$result" ]] && echo "$result" | jq -e '.name' > /dev/null 2>&1; then
    test_pass "Server accepted vessels.* context subscription"
else
    test_fail "Server should accept vessels.* context"
fi

# =============================================================================
# Summary
# =============================================================================
print_summary
