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

# Run all test scripts in order.
# Disable set -e so assertion failures don't abort the runner —
# we want to run ALL tests and report the summary.
set +e
for test_script in "$SCRIPT_DIR"/test_*.sh; do
    if [[ -f "$test_script" ]]; then
        echo ""
        echo "--- Running: $(basename "$test_script") ---"
        if ! bash -n "$test_script" 2>/dev/null; then
            echo "  FAIL: syntax error in $(basename "$test_script"), skipping"
            ((FAIL++)) || true
            continue
        fi
        source "$test_script"
    fi
done
set -e

# --- Summary ---
print_summary
