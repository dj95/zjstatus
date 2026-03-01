# Docker-based Integration Tests Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Docker-based integration tests for zjstatus that run Zellij headlessly in a container and validate plugin loading, pipe protocol commands, tab switching, and edge cases — catching runtime panics that unit tests miss.

**Architecture:** Reuse the proven approach from sibling project `zellij-tab-status`: Docker container with ubuntu:24.04 + Zellij 0.43.1, headless execution via `script` PTY emulation, tests communicate with the plugin via `zellij pipe --plugin "file:/test/plugin.wasm"`. WASM artifact is built on host and mounted into the container. Each test is a shell function in a single test runner script. Panic detection via session liveness checks (not stderr — `script` PTY doesn't propagate plugin panics to stderr).

**Tech Stack:** Docker (ubuntu:24.04), Zellij 0.43.1, Bash, GNU Make, GitHub Actions

**Reference:** `~/code/zellij-tab-status/` — `Dockerfile.test`, `scripts/docker-test-runner.sh`, `scripts/integration-test.sh`, `.github/workflows/ci.yml`

---

### Task 1: Create Dockerfile.test

**Files:**
- Create: `Dockerfile.test`

**Step 1: Write the Dockerfile**

```dockerfile
FROM ubuntu:24.04

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       curl ca-certificates util-linux \
    && curl -L https://github.com/zellij-org/zellij/releases/download/v0.43.1/zellij-x86_64-unknown-linux-musl.tar.gz \
       | tar xz -C /usr/local/bin \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /test
```

Packages: `curl` for downloading Zellij, `ca-certificates` for HTTPS, `util-linux` for `script` (PTY emulation).

**Step 2: Verify it builds**

Run: `docker build -f Dockerfile.test -t zjstatus-test .`
Expected: Successful build, exit code 0, under 60s.

**Step 3: Commit**

```bash
git add Dockerfile.test
git commit -m "feat: add Dockerfile.test for integration tests

Ubuntu 24.04 with Zellij 0.43.1 for headless integration testing."
```

---

### Task 2: Create test layout KDL

**Files:**
- Create: `tests/integration/test-layout.kdl`

**Step 1: Write the minimal KDL layout**

```kdl
layout {
    pane split_direction="vertical" {
        pane
    }

    pane size=1 borderless=true {
        plugin location="file:/test/plugin.wasm" {
            format_left  "#[fg=blue]{mode} {tabs}"
            format_right "{datetime}"
            mode_normal "#[fg=green]NORMAL"
            tab_normal "#[fg=white]{name}"
            tab_active "#[fg=yellow,bold]{name}"
            datetime "{:%H:%M}"
            command_cmd_test "echo hello"
            command_cmd_test_interval "5"
        }
    }
}
```

This layout includes: mode, tabs, datetime, and command widgets — covering all main widget types. Plugin path `file:/test/plugin.wasm` matches the Docker mount point.

**Step 2: Commit**

```bash
git add tests/integration/test-layout.kdl
git commit -m "feat: add test layout for integration tests

Minimal KDL layout with mode, tabs, datetime, and command widgets."
```

---

### Task 3: Create helpers.sh

**Files:**
- Create: `tests/integration/helpers.sh`

**Step 1: Write the helpers script**

Key design decisions vs v1:
- `send_pipe` uses `--plugin "file:$PLUGIN_WASM"` (not `-p zjstatus` — plugin has no alias)
- Panic detection via `assert_session_alive` (session liveness check) instead of stderr grep (`script` PTY doesn't propagate plugin stderr)
- `assert_pipe_responds` for positive assertions (plugin processes commands and doesn't hang)
- `ZELLIJ_TEST_TIMEOUT` parameterizes all timeouts

```bash
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
    local layout="${1:-}"

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
        script -qfc "zellij --session $ZELLIJ_SESSION --layout /root/.config/zellij/test-layout.kdl options --disable-mouse-mode" /dev/null > /dev/null 2>&1 &
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
    sleep 5
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
    local actual
    actual=$(zellij action query-tab-names 2>/dev/null | wc -l)
    assert_eq "$actual" "$expected" "$msg"
}

# --- Pipe helpers ---

send_pipe() {
    timeout "${ZELLIJ_TEST_TIMEOUT}s" zellij pipe --plugin "file:$PLUGIN_WASM" -- "$1" < /dev/null 2>/dev/null || true
}

# --- Tab helpers ---

close_extra_tabs() {
    local tab_count
    tab_count=$(zellij action query-tab-names 2>/dev/null | wc -l)
    while [[ "$tab_count" -gt 1 ]]; do
        zellij action go-to-tab "$tab_count" 2>/dev/null || true
        sleep 0.2
        zellij action close-tab 2>/dev/null || true
        sleep 0.5
        tab_count=$(zellij action query-tab-names 2>/dev/null | wc -l)
    done
    zellij action go-to-tab 1 2>/dev/null || true
    sleep 0.3
}

# --- Summary ---

print_summary() {
    echo ""
    echo "==============================="
    echo "Results: $PASS passed, $FAIL failed"
    echo "==============================="
    return $FAIL
}
```

**Step 2: Make it executable**

Run: `chmod +x tests/integration/helpers.sh`

**Step 3: Commit**

```bash
git add tests/integration/helpers.sh
git commit -m "feat: add integration test helpers

Setup/teardown for headless Zellij, assertions (session_alive,
pipe_responds, eq, tab_count), pipe/tab helpers, summary."
```

---

### Task 4: Create the Docker test runner script

**Files:**
- Create: `tests/integration/docker-test-runner.sh`

**Step 1: Write the runner script**

This script runs INSIDE the Docker container. It sets up Zellij headlessly with the test layout and runs the test suite.

```bash
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
```

**Step 2: Make it executable**

Run: `chmod +x tests/integration/docker-test-runner.sh`

**Step 3: Commit**

```bash
git add tests/integration/docker-test-runner.sh
git commit -m "feat: add Docker test runner for integration tests

Orchestrates headless Zellij setup and runs all test_*.sh scripts."
```

---

### Task 5: Create Phase 2 basic tests

**Files:**
- Create: `tests/integration/test_01_basic.sh`

**Step 1: Write basic tests**

Each test has both a session-liveness check AND a positive assertion where applicable.

```bash
#!/usr/bin/env bash
#
# Phase 2: Basic tests — plugin loading and pipe commands.
# Sourced by docker-test-runner.sh (helpers.sh already loaded).
#

# --- test_plugin_loads ---
echo "  [test_plugin_loads] Zellij started with zjstatus without crashing"
assert_session_alive "plugin loaded without crash"

# --- test_pipe_notify ---
echo "  [test_pipe_notify] zjstatus::notify::message"
assert_pipe_responds "zjstatus::notify::Hello World" "notify: plugin processes pipe"
sleep 0.5
assert_session_alive "notify: session alive after notify"

# --- test_pipe_set_content ---
echo "  [test_pipe_set_content] zjstatus::pipe::name::content"
assert_pipe_responds "zjstatus::pipe::test_var::test_value" "pipe set_content: plugin processes pipe"
sleep 0.5
assert_session_alive "pipe set_content: session alive"

# --- test_pipe_set_status ---
echo "  [test_pipe_set_status] zjstatus::set_status + clear_status"
assert_pipe_responds "zjstatus::set_status::1::🔴" "set_status: plugin processes pipe"
sleep 0.5
assert_session_alive "set_status: session alive"
assert_pipe_responds "zjstatus::clear_status::1" "clear_status: plugin processes pipe"
sleep 0.5
assert_session_alive "clear_status: session alive"

# --- test_pipe_invalid_format ---
echo "  [test_pipe_invalid_format] invalid pipe message (too few parts)"
send_pipe "zjstatus"
sleep 1
assert_session_alive "invalid format: session alive (plugin ignored bad input)"
# Positive check: plugin still responds to valid pipe after bad input
assert_pipe_responds "zjstatus::notify::still alive" "invalid format: plugin still responds"

# --- test_pipe_unknown_command ---
echo "  [test_pipe_unknown_command] unknown command"
send_pipe "zjstatus::nonexistent::arg"
sleep 1
assert_session_alive "unknown command: session alive"
assert_pipe_responds "zjstatus::notify::still alive" "unknown command: plugin still responds"
```

**Step 2: Make it executable**

Run: `chmod +x tests/integration/test_01_basic.sh`

**Step 3: Verify tests run locally**

Run:
```bash
cargo build --target wasm32-wasip1 --release && \
docker build -f Dockerfile.test -t zjstatus-test . && \
docker run --rm \
  -v "$(pwd)/target/wasm32-wasip1/release/zjstatus.wasm:/test/plugin.wasm:ro" \
  -v "$(pwd)/tests/integration:/test/tests:ro" \
  zjstatus-test \
  /test/tests/docker-test-runner.sh
```
Expected: All basic tests pass, exit code 0.

**Step 4: Commit**

```bash
git add tests/integration/test_01_basic.sh
git commit -m "feat: add Phase 2 basic integration tests

6 tests: plugin_loads, pipe_notify, pipe_set_content, pipe_set_status,
pipe_invalid_format, pipe_unknown_command. Each with session-alive and
positive pipe-responds assertions."
```

---

### Task 6: Create Phase 3 edge-case tests

**Files:**
- Create: `tests/integration/test_02_edge_cases.sh`

**Step 1: Write edge-case tests**

```bash
#!/usr/bin/env bash
#
# Phase 3: Edge-case tests — tab switching, many tabs, command widget.
# Sourced by docker-test-runner.sh (helpers.sh already loaded).
#

# --- test_tab_switching ---
echo "  [test_tab_switching] switching between 3 tabs"
zellij action new-tab 2>/dev/null || true
sleep 0.5
zellij action new-tab 2>/dev/null || true
sleep 0.5
assert_tab_count "3" "created 3 tabs"
# Switch between tabs
zellij action go-to-tab 2 2>/dev/null || true
sleep 0.3
zellij action go-to-tab 3 2>/dev/null || true
sleep 0.3
zellij action go-to-tab 1 2>/dev/null || true
sleep 0.5
assert_session_alive "tab switching: session alive"
assert_pipe_responds "zjstatus::notify::after switch" "tab switching: plugin responds after switches"
close_extra_tabs

# --- test_command_widget ---
echo "  [test_command_widget] command widget execution"
# The test layout has command_cmd_test "echo hello" with interval 5
sleep 6
assert_session_alive "command widget: session alive after command execution"
assert_pipe_responds "zjstatus::notify::after cmd" "command widget: plugin responds"

# --- test_many_tabs ---
echo "  [test_many_tabs] creating 15 tabs"
for i in $(seq 1 15); do
    zellij action new-tab 2>/dev/null || true
    sleep 0.2
done
sleep 2
assert_tab_count "16" "16 tabs exist (1 original + 15 new)"
assert_session_alive "many tabs: session alive with 16 tabs"
assert_pipe_responds "zjstatus::notify::many tabs" "many tabs: plugin responds with 16 tabs"
close_extra_tabs
assert_tab_count "1" "cleaned up to 1 tab"

# --- test_close_all_tabs_except_one ---
echo "  [test_close_all_tabs_except_one] create 5 tabs then close 4"
for i in $(seq 1 4); do
    zellij action new-tab 2>/dev/null || true
    sleep 0.3
done
assert_tab_count "5" "5 tabs created"
# Close tabs back to 1
close_extra_tabs
assert_tab_count "1" "back to 1 tab"
assert_session_alive "close tabs: session alive"
assert_pipe_responds "zjstatus::notify::after close" "close tabs: plugin responds after mass close"
```

**Step 2: Make it executable**

Run: `chmod +x tests/integration/test_02_edge_cases.sh`

**Step 3: Verify tests run locally**

Run:
```bash
docker run --rm \
  -v "$(pwd)/target/wasm32-wasip1/release/zjstatus.wasm:/test/plugin.wasm:ro" \
  -v "$(pwd)/tests/integration:/test/tests:ro" \
  zjstatus-test \
  /test/tests/docker-test-runner.sh
```
Expected: All tests pass (basic + edge-case), exit code 0.

**Step 4: Commit**

```bash
git add tests/integration/test_02_edge_cases.sh
git commit -m "feat: add Phase 3 edge-case integration tests

4 tests: tab_switching, command_widget, many_tabs (15),
close_all_tabs_except_one. With tab count and pipe assertions."
```

---

### Task 7: Create race condition test (separate runner)

**Files:**
- Create: `tests/integration/test-race-runner.sh`

The race condition test needs its OWN Zellij session where we send pipe IMMEDIATELY after session start (no sleep for init). This cannot be part of the main test suite because by then the plugin is fully initialized.

**Step 1: Write the race test runner**

```bash
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
script -qfc "zellij --session $ZELLIJ_SESSION --layout /root/.config/zellij/test-layout.kdl options --disable-mouse-mode" /dev/null > /dev/null 2>&1 &
ZELLIJ_PID=$!

# Wait ONLY for session to appear (not for plugin init)
for i in $(seq 1 30); do
    if zellij list-sessions 2>/dev/null | grep -q "$ZELLIJ_SESSION"; then
        break
    fi
    sleep 0.5
done

# Send pipe IMMEDIATELY — plugin may not have permissions yet.
# This tests the pending_events buffer (events queued until PermissionRequestResult).
echo "  [test_race_pipe_before_init] sending pipe before plugin init"
send_pipe "zjstatus::notify::Race Test"
send_pipe "zjstatus::pipe::race_key::race_value"
send_pipe "zjstatus::set_status::1::🏁"

# Now wait for plugin to fully initialize
sleep 5

# Verify: session survived the race
assert_session_alive "race: session alive after early pipes"
# Verify: plugin still responds normally after the race
assert_pipe_responds "zjstatus::notify::post race" "race: plugin responds after early pipes"

# Cleanup
trap '' EXIT
zellij kill-session "$ZELLIJ_SESSION" 2>/dev/null || true
wait $ZELLIJ_PID 2>/dev/null || true

print_summary
```

**Step 2: Make it executable**

Run: `chmod +x tests/integration/test-race-runner.sh`

**Step 3: Commit**

```bash
git add tests/integration/test-race-runner.sh
git commit -m "feat: add race condition test (pipe before plugin init)

Sends pipe messages immediately after session start, before plugin
has received permissions. Tests the pending_events buffer."
```

---

### Task 8: Create Makefile

**Files:**
- Create: `Makefile`

**Step 1: Write the Makefile**

```makefile
.PHONY: build test test-integration

WASM_TARGET = wasm32-wasip1
WASM_ARTIFACT = target/$(WASM_TARGET)/release/zjstatus.wasm
DOCKER_IMAGE = zjstatus-test

build:
	cargo build --target $(WASM_TARGET) --release

test:
	cargo nextest run --lib

test-integration: build
	docker build -f Dockerfile.test -t $(DOCKER_IMAGE) .
	docker run --rm \
		-v "$$(pwd)/$(WASM_ARTIFACT):/test/plugin.wasm:ro" \
		-v "$$(pwd)/tests/integration:/test/tests:ro" \
		$(DOCKER_IMAGE) \
		/test/tests/docker-test-runner.sh
	docker run --rm \
		-v "$$(pwd)/$(WASM_ARTIFACT):/test/plugin.wasm:ro" \
		-v "$$(pwd)/tests/integration:/test/tests:ro" \
		$(DOCKER_IMAGE) \
		/test/tests/test-race-runner.sh
```

**Step 2: Verify it works**

Run: `make test-integration`
Expected: Builds WASM, builds Docker image, runs main tests + race test, exit code 0.

**Step 3: Commit**

```bash
git add Makefile
git commit -m "feat: add Makefile with build, test, and test-integration targets

test-integration runs main suite + race condition test in separate containers."
```

---

### Task 9: Add integration tests to CI

**Files:**
- Modify: `.github/workflows/lint.yml`

**Step 1: Add integration-test job to lint.yml**

After the existing `tests` job, add a new `integration-test` job with `continue-on-error: true` for initial stabilization (remove after Phase 2 is green):

```yaml
  integration-test:
    runs-on: ubuntu-latest
    needs: tests
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd # v4

      - name: Install WASI SDK
        run: |
          wget https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-20/wasi-sdk-20.0-linux.tar.gz
          tar -xzf wasi-sdk-20.0-linux.tar.gz
          mv wasi-sdk-20.0 /opt/wasi-sdk
        env:
          WASI_SDK_PATH: /opt/wasi-sdk

      - name: Install Rust
        uses: dtolnay/rust-toolchain@4f647fc679bcd3b11499ccb42104547c83dabe96 # stable
        with:
          toolchain: "1.91.0"
          target: wasm32-wasip1

      - name: Build WASM
        run: cargo build --target wasm32-wasip1 --release

      - name: Build test Docker image
        run: docker build -f Dockerfile.test -t zjstatus-test .

      - name: Run integration tests
        run: |
          docker run --rm \
            -v "$(pwd)/target/wasm32-wasip1/release/zjstatus.wasm:/test/plugin.wasm:ro" \
            -v "$(pwd)/tests/integration:/test/tests:ro" \
            zjstatus-test \
            /test/tests/docker-test-runner.sh

      - name: Run race condition test
        run: |
          docker run --rm \
            -v "$(pwd)/target/wasm32-wasip1/release/zjstatus.wasm:/test/plugin.wasm:ro" \
            -v "$(pwd)/tests/integration:/test/tests:ro" \
            zjstatus-test \
            /test/tests/test-race-runner.sh
```

**Step 2: Verify CI YAML is valid**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/lint.yml'))"`
Expected: No errors.

**Step 3: Commit**

```bash
git add .github/workflows/lint.yml
git commit -m "ci: add integration test job to lint workflow

Builds WASM, spins up Docker with headless Zellij, runs main suite +
race test. Blocking — failures prevent merge."
```

---

### Task 10: Create regression test template

**Files:**
- Create: `tests/integration/test_99_regression_template.sh.example`

**Step 1: Write the template**

```bash
#!/usr/bin/env bash
#
# Regression test template for zjstatus.
# Copy this file to test_99_regression_issueN.sh and fill in the scenario.
#
# Sourced by docker-test-runner.sh (helpers.sh already loaded).
#

# --- test_regression_issueN ---
echo "  [test_regression_issueN] description of the bug"

# Reproduce the bug scenario:
# send_pipe "zjstatus::..."
# zellij action ...
# sleep 1

# Verify plugin survived:
# assert_session_alive "issueN: session alive"
# assert_pipe_responds "zjstatus::notify::check" "issueN: plugin responds"

# Verify expected behavior:
# assert_tab_count "1" "issueN: expected tab count"
```

**Step 2: Commit**

```bash
git add tests/integration/test_99_regression_template.sh.example
git commit -m "docs: add regression test template for future bug reproduction"
```

---

### Task 11: Full end-to-end verification

**Step 1: Build WASM**

Run: `cargo build --target wasm32-wasip1 --release`
Expected: Successful build.

**Step 2: Run full integration test suite**

Run: `make test-integration`
Expected: All tests pass (basic + edge-case + race), exit code 0.

**Step 3: Run unit tests to verify no regressions**

Run: `cargo nextest run --lib`
Expected: All existing unit tests pass.

**Step 4: Run clippy**

Run: `cargo clippy --all-features --lib`
Expected: No warnings.

**Step 5: Run integration tests 3 times to check for flakiness**

Run:
```bash
for i in 1 2 3; do echo "=== Run $i ===" && make test-integration; done
```
Expected: All 3 runs pass. If any fail, investigate and fix flakiness before merging.

**Step 6: Final commit (if any fixes needed)**

If any adjustments were made during verification, commit them.
Commit any fixes needed during verification.
