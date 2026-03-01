#!/usr/bin/env bash
#
# Helpers for zjstatus integration tests.
# Source this file from test scripts.
#

PASS=0
FAIL=0
ZELLIJ_SESSION="${ZELLIJ_SESSION:-zjstatus-test}"
ZELLIJ_PID=""
PLUGIN_WASM="/test/plugin.wasm"
ZELLIJ_TEST_TIMEOUT="${ZELLIJ_TEST_TIMEOUT:-10}"

# --- Setup / Teardown ---

setup_zellij() {
    local layout="" no_wait=false session=""

    # Parse named arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --no-wait)  no_wait=true; shift ;;
            --session)  session="$2"; shift 2 ;;
            *)          layout="$1"; shift ;;
        esac
    done

    if [[ -n "$session" ]]; then
        ZELLIJ_SESSION="$session"
    fi

    if [[ ! -f "$PLUGIN_WASM" ]]; then
        echo "ERROR: Plugin not found at $PLUGIN_WASM"
        exit 1
    fi

    # Pre-approve permissions (no UI in headless mode)
    mkdir -p /root/.config/zellij /root/.cache/zellij

    if [[ -n "$layout" ]]; then
        cp "$layout" /root/.config/zellij/test-layout.kdl
    fi

    # zjstatus requests: ReadApplicationState, ChangeApplicationState, RunCommands
    # ReadCliPipes needed so plugin receives CLI pipe messages
    cat > /root/.cache/zellij/permissions.kdl <<PERMS
"$PLUGIN_WASM" {
    ReadApplicationState
    ChangeApplicationState
    RunCommands
    ReadCliPipes
}
PERMS

    # Start Zellij headlessly via script (PTY emulation).
    # stderr goes to /dev/null — script PTY does NOT propagate plugin panics
    # to stderr anyway. Panic detection uses session liveness instead.
    if [[ -n "$layout" ]]; then
        script -qfc "zellij --session $ZELLIJ_SESSION -n /root/.config/zellij/test-layout.kdl options --disable-mouse-mode" /dev/null > /dev/null 2>&1 &
    else
        script -qfc "zellij --session $ZELLIJ_SESSION options --disable-mouse-mode" /dev/null > /dev/null 2>&1 &
    fi
    ZELLIJ_PID=$!

    # Wait for session to be ready
    local i
    for i in $(seq 1 30); do
        if zellij list-sessions 2>/dev/null | grep -q "$ZELLIJ_SESSION"; then
            echo "Zellij session ready (attempt $i)"
            break
        fi
        if ! kill -0 $ZELLIJ_PID 2>/dev/null; then
            echo "ERROR: Zellij process died"
            exit 1
        fi
        sleep 0.5
    done

    if ! zellij list-sessions 2>/dev/null | grep -q "$ZELLIJ_SESSION"; then
        echo "ERROR: Zellij session did not start within 15s"
        kill $ZELLIJ_PID 2>/dev/null || true
        exit 1
    fi

    # Wait for plugin WASM compilation and initialization
    if [[ "$no_wait" != true ]]; then
        sleep 5
    fi
}

teardown_zellij() {
    zellij kill-session "$ZELLIJ_SESSION" 2>/dev/null || true
    if [[ -n "$ZELLIJ_PID" ]]; then
        wait $ZELLIJ_PID 2>/dev/null || true
    fi
}

# --- Assertions ---

assert_session_alive() {
    local msg="${1:-session is alive}"
    if zellij list-sessions 2>/dev/null | grep -q "$ZELLIJ_SESSION"; then
        echo "  PASS: $msg"
        ((PASS++)) || true
        return 0
    else
        echo "  FAIL: $msg (session not found — likely crashed/panicked)"
        ((FAIL++)) || true
        return 1
    fi
}

assert_pipe_responds() {
    local pipe_msg="$1" msg="${2:-pipe responds without timeout}"
    # If plugin crashed, this will timeout. Success = plugin is alive and processing.
    if timeout "${ZELLIJ_TEST_TIMEOUT}s" zellij pipe --plugin "file:$PLUGIN_WASM" -- "$pipe_msg" < /dev/null 2>/dev/null; then
        echo "  PASS: $msg"
        ((PASS++)) || true
        return 0
    else
        echo "  FAIL: $msg (pipe timed out or failed — plugin may have crashed)"
        ((FAIL++)) || true
        return 1
    fi
}

assert_eq() {
    local actual="$1" expected="$2" msg="$3"
    if [[ "$actual" == "$expected" ]]; then
        echo "  PASS: $msg"
        ((PASS++)) || true
    else
        echo "  FAIL: $msg"
        echo "    expected: '$expected'"
        echo "    actual:   '$actual'"
        ((FAIL++)) || true
    fi
}

assert_tab_count() {
    local expected="$1" msg="${2:-tab count is $1}"
    local actual attempt
    for attempt in $(seq 1 5); do
        actual=$(timeout 5 zellij action query-tab-names 2>/dev/null | wc -l)
        if [[ "$actual" == "$expected" ]]; then
            break
        fi
        sleep 0.5
    done
    assert_eq "$actual" "$expected" "$msg"
}

# --- Pipe helpers ---

send_pipe() {
    local rc=0
    timeout "${ZELLIJ_TEST_TIMEOUT}s" zellij pipe --plugin "file:$PLUGIN_WASM" -- "$1" < /dev/null 2>/dev/null || rc=$?
    if [[ $rc -ne 0 ]]; then
        echo "  WARNING: send_pipe exit code $rc for: $1"
    fi
    return $rc
}

# --- Tab helpers ---

close_extra_tabs() {
    local tab_count max_iter=30 iter=0
    tab_count=$(timeout 10 zellij action query-tab-names 2>/dev/null | wc -l)
    while [[ "$tab_count" -gt 1 ]] && [[ "$iter" -lt "$max_iter" ]]; do
        ((iter++)) || true
        timeout 10 zellij action go-to-tab "$tab_count" 2>/dev/null || true
        sleep 0.5
        timeout 10 zellij action close-tab 2>/dev/null || true
        sleep 1
        # Verify session still exists
        if ! zellij list-sessions 2>/dev/null | grep -q "$ZELLIJ_SESSION"; then
            echo "  WARNING: session died during tab cleanup"
            return 1
        fi
        tab_count=$(timeout 5 zellij action query-tab-names 2>/dev/null | wc -l)
    done
    if [[ "$iter" -ge "$max_iter" ]] && [[ "$tab_count" -gt 1 ]]; then
        echo "  WARNING: close_extra_tabs exhausted $max_iter iterations, $tab_count tabs remain"
        timeout 5 zellij action go-to-tab 1 2>/dev/null || true
        sleep 0.3
        return 1
    fi
    sleep 0.3
}

# --- Summary ---

print_summary() {
    echo ""
    echo "==============================="
    echo "Results: $PASS passed, $FAIL failed"
    echo "==============================="
    [[ "$FAIL" -gt 0 ]] && return 1 || return 0
}
