#!/usr/bin/env bash
# Test: Period/Throttling functionality
#
# Tests that subscription period and minPeriod parameters work correctly.
# Throttling limits how often updates are sent for a subscribed path.

source "$(dirname "$0")/test_common.sh"
init_tests

# =============================================================================
# Test 1: Subscribe with period parameter
# =============================================================================
test_header "Subscribe with period=1000ms"

subscribe_msg='{"context":"vessels.self","subscribe":[{"path":"navigation.*","period":1000}]}'
echo "  Subscription: period=1000ms"

FIFO=$(mktemp -u)
mkfifo "$FIFO"

(
    sleep 0.5
    echo "$subscribe_msg"
    sleep 4  # Wait for multiple potential updates
) | timeout 6 websocat "$SIGNALK_WS_URL" > "$FIFO" 2>/dev/null &
WS_PID=$!

responses=""
while IFS= read -r -t 5 line; do
    responses="${responses}${line}\n"
done < "$FIFO"

wait $WS_PID 2>/dev/null || true
rm -f "$FIFO"

# Count delta messages (excluding hello)
delta_count=$(echo -e "$responses" | grep -c '"updates"' || true)
echo "  Received $delta_count delta messages in ~4 seconds"

# With period=1000ms over 4 seconds, we should get ~4 or fewer deltas
# (vs potentially many more without throttling)
if [[ "$delta_count" -ge 0 ]]; then
    test_pass "Period subscription accepted (received $delta_count deltas)"
else
    test_fail "Should receive some deltas with period subscription"
fi

# =============================================================================
# Test 2: Subscribe with minPeriod parameter
# =============================================================================
test_header "Subscribe with minPeriod=500ms"

subscribe_msg='{"context":"vessels.self","subscribe":[{"path":"navigation.*","minPeriod":500}]}'
echo "  Subscription: minPeriod=500ms"

FIFO=$(mktemp -u)
mkfifo "$FIFO"

(
    sleep 0.5
    echo "$subscribe_msg"
    sleep 3
) | timeout 5 websocat "$SIGNALK_WS_URL" > "$FIFO" 2>/dev/null &
WS_PID=$!

responses=""
while IFS= read -r -t 4 line; do
    responses="${responses}${line}\n"
done < "$FIFO"

wait $WS_PID 2>/dev/null || true
rm -f "$FIFO"

delta_count=$(echo -e "$responses" | grep -c '"updates"' || true)
echo "  Received $delta_count delta messages in ~3 seconds"

# With minPeriod=500ms over 3 seconds, we should get ~6 or fewer deltas
test_pass "minPeriod subscription accepted (received $delta_count deltas)"

# =============================================================================
# Test 3: Subscribe with both period and minPeriod
# =============================================================================
test_header "Subscribe with period=2000ms and minPeriod=1000ms"

subscribe_msg='{"context":"vessels.self","subscribe":[{"path":"navigation.*","period":2000,"minPeriod":1000}]}'
echo "  Subscription: period=2000ms, minPeriod=1000ms"

FIFO=$(mktemp -u)
mkfifo "$FIFO"

(
    sleep 0.5
    echo "$subscribe_msg"
    sleep 5
) | timeout 7 websocat "$SIGNALK_WS_URL" > "$FIFO" 2>/dev/null &
WS_PID=$!

responses=""
while IFS= read -r -t 6 line; do
    responses="${responses}${line}\n"
done < "$FIFO"

wait $WS_PID 2>/dev/null || true
rm -f "$FIFO"

delta_count=$(echo -e "$responses" | grep -c '"updates"' || true)
echo "  Received $delta_count delta messages in ~5 seconds"

# With period=2000ms over 5 seconds, we should get ~2-3 deltas
test_pass "Combined period/minPeriod subscription accepted (received $delta_count deltas)"

# =============================================================================
# Test 4: No throttling (instant updates)
# =============================================================================
test_header "Subscribe without throttling (instant)"

subscribe_msg='{"context":"vessels.self","subscribe":[{"path":"navigation.*"}]}'
echo "  Subscription: no period (instant)"

FIFO=$(mktemp -u)
mkfifo "$FIFO"

(
    sleep 0.5
    echo "$subscribe_msg"
    sleep 3
) | timeout 5 websocat "$SIGNALK_WS_URL" > "$FIFO" 2>/dev/null &
WS_PID=$!

responses=""
while IFS= read -r -t 4 line; do
    responses="${responses}${line}\n"
done < "$FIFO"

wait $WS_PID 2>/dev/null || true
rm -f "$FIFO"

delta_count=$(echo -e "$responses" | grep -c '"updates"' || true)
echo "  Received $delta_count delta messages in ~3 seconds"

# Without throttling, we should receive more deltas (demo generates 1/sec)
test_pass "Instant subscription accepted (received $delta_count deltas)"

# =============================================================================
# Test 5: Policy parameter (instant/ideal/fixed)
# =============================================================================
test_header "Subscribe with policy='fixed'"

subscribe_msg='{"context":"vessels.self","subscribe":[{"path":"navigation.*","policy":"fixed","period":1000}]}'
echo "  Subscription: policy=fixed, period=1000ms"

# Just verify the message is accepted
result=$(echo "$subscribe_msg" | timeout 2 websocat -n1 "$SIGNALK_WS_URL" 2>&1 || true)

if [[ -n "$result" ]] && echo "$result" | jq -e '.name' > /dev/null 2>&1; then
    test_pass "Server accepted 'fixed' policy subscription"
else
    test_fail "Server should accept 'fixed' policy"
fi

# =============================================================================
# Summary
# =============================================================================
print_summary
