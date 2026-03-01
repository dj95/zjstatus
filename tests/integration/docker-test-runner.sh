#!/usr/bin/env bash
#
# Docker test runner for zjstatus integration tests.
# Runs INSIDE the Docker container.
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/helpers.sh"

echo "=== zjstatus Integration Tests ==="

# --- Setup ---
setup_zellij "$SCRIPT_DIR/test-layout.kdl"
trap teardown_zellij EXIT

# Run all test scripts in order
for test_script in "$SCRIPT_DIR"/test_*.sh; do
    if [[ -f "$test_script" ]]; then
        echo ""
        echo "--- Running: $(basename "$test_script") ---"
        source "$test_script"
    fi
done

# --- Summary ---
print_summary
