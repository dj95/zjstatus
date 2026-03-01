#!/usr/bin/env bash
#
# Race condition test: pipe message before plugin initialization completes.
# Runs as a SEPARATE docker invocation with its own Zellij session.
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/helpers.sh"

echo "=== zjstatus Race Condition Test ==="

# Start Zellij without waiting for plugin init (--no-wait)
setup_zellij --session zjstatus-race-test --no-wait "$SCRIPT_DIR/test-layout.kdl"
trap teardown_zellij EXIT

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
