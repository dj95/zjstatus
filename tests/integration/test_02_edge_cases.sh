#!/usr/bin/env bash
#
# Edge-case tests — tab switching, many tabs, command widget.
# Sourced by docker-test-runner.sh (helpers.sh already loaded).
#

# --- test_tab_switching ---
echo "  [test_tab_switching] switching between 3 tabs"
zellij action new-tab 2>/dev/null
sleep 0.5
zellij action new-tab 2>/dev/null
sleep 0.5
assert_tab_count "3" "created 3 tabs"
# Switch between tabs
zellij action go-to-tab 2 2>/dev/null
sleep 0.3
zellij action go-to-tab 3 2>/dev/null
sleep 0.3
zellij action go-to-tab 1 2>/dev/null
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
echo "  [test_many_tabs] creating 7 tabs"
for i in $(seq 1 7); do
    zellij action new-tab 2>/dev/null
    sleep 0.5
done
sleep 2
assert_tab_count "8" "8 tabs exist (1 original + 7 new)"
assert_session_alive "many tabs: session alive with 8 tabs"
assert_pipe_responds "zjstatus::notify::many tabs" "many tabs: plugin responds with 8 tabs"
close_extra_tabs
assert_tab_count "1" "cleaned up to 1 tab"

# --- test_close_all_tabs_except_one ---
echo "  [test_close_all_tabs_except_one] create 5 tabs then close 4"
for i in $(seq 1 4); do
    zellij action new-tab 2>/dev/null
    sleep 0.3
done
assert_tab_count "5" "5 tabs created"
# Close tabs back to 1
close_extra_tabs
assert_tab_count "1" "back to 1 tab"
assert_session_alive "close tabs: session alive"
assert_pipe_responds "zjstatus::notify::after close" "close tabs: plugin responds after mass close"
