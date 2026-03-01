#!/usr/bin/env bash
#
# Race condition test: pipe message before plugin initialization completes.
# Runs as a SEPARATE docker invocation with its own Zellij session.
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/helpers.sh"

echo "=== zjstatus Race Condition Test ==="

ZELLIJ_SESSION="zjstatus-race-test"

if [[ ! -f "$PLUGIN_WASM" ]]; then
    echo "ERROR: Plugin not found at $PLUGIN_WASM"
    exit 1
fi

mkdir -p /root/.config/zellij /root/.cache/zellij
cp "$SCRIPT_DIR/test-layout.kdl" /root/.config/zellij/test-layout.kdl

cat > /root/.cache/zellij/permissions.kdl <<PERMS
"$PLUGIN_WASM" {
    ReadApplicationState
    ChangeApplicationState
    RunCommands
    ReadCliPipes
}
PERMS

# Start Zellij
script -qfc "zellij --session $ZELLIJ_SESSION -n /root/.config/zellij/test-layout.kdl options --disable-mouse-mode" /dev/null > /dev/null 2>&1 &
ZELLIJ_PID=$!

# Wait ONLY for session to appear (not for plugin init)
for i in $(seq 1 30); do
    if zellij list-sessions 2>/dev/null | grep -q "$ZELLIJ_SESSION"; then
        break
    fi
    if ! kill -0 $ZELLIJ_PID 2>/dev/null; then
        echo "ERROR: Zellij process died during startup"
        exit 1
    fi
    sleep 0.5
done

if ! zellij list-sessions 2>/dev/null | grep -q "$ZELLIJ_SESSION"; then
    echo "ERROR: Race test session did not start within 15s"
    kill $ZELLIJ_PID 2>/dev/null || true
    exit 1
fi

# Set up cleanup trap before sending pipes
cleanup_race() {
    zellij kill-session "$ZELLIJ_SESSION" 2>/dev/null || true
    if [[ -n "${ZELLIJ_PID:-}" ]]; then
        wait $ZELLIJ_PID 2>/dev/null || true
    fi
}
trap cleanup_race EXIT

# Send pipe IMMEDIATELY — plugin may not have permissions yet.
# This tests the pending_events buffer (events queued until PermissionRequestResult).
echo "  [test_race_pipe_before_init] sending pipe before plugin init"
set +e
send_pipe "zjstatus::notify::Race Test"
send_pipe "zjstatus::pipe::race_key::race_value"
send_pipe "zjstatus::set_status::1::🏁"

# Now wait for plugin to fully initialize
sleep 5

# Verify: session survived the race
assert_session_alive "race: session alive after early pipes"
# Verify: plugin still responds normally after the race
assert_pipe_responds "zjstatus::notify::post race" "race: plugin responds after early pipes"
set -e

# Cleanup handled by EXIT trap
print_summary
