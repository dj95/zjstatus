#!/usr/bin/env bash
#
# Basic tests — plugin loading and pipe commands.
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
