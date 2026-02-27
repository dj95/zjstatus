# Tab Emoji Status Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `{status}` placeholder to tab templates, controlled via pipe commands `set_status`/`clear_status`.

**Architecture:** CLI sends `zellij pipe` with pane_id → `pipe.rs` resolves pane_id to tab_index via `PaneManifest` → stores emoji in `ZellijState.tab_statuses` → `TabsWidget.render_tab()` substitutes `{status}` placeholder.

**Tech Stack:** Rust, zellij-tile 0.43.1, wasm32-wasip1. Tests: rstest, cargo nextest.

---

### Task 1: Add `tab_statuses` field to ZellijState

**Files:**
- Modify: `src/config.rs:14-27` (ZellijState struct)
- Modify: `src/bin/zjstatus.rs:91-103` (state initialization)

**Step 1: Add field to ZellijState**

In `src/config.rs`, add to the `ZellijState` struct:

```rust
pub tab_statuses: BTreeMap<usize, String>,
```

No new import needed — `BTreeMap` is already imported from `std::collections`.

**Step 2: Initialize field in zjstatus.rs**

In `src/bin/zjstatus.rs`, in the `load()` method where `ZellijState` is constructed, add:

```rust
tab_statuses: BTreeMap::new(),
```

**Step 3: Run tests to verify nothing breaks**

Run: `cargo nextest run --lib`
Expected: all existing tests PASS (field has `Default`-compatible empty BTreeMap)

**Step 4: Commit**

```
feat(status): add tab_statuses field to ZellijState
```

---

### Task 2: Add `resolve_tab_index` and pipe commands in `pipe.rs`

**Files:**
- Modify: `src/pipe.rs:44-83` (process_line match block)
- Test: `src/pipe.rs` (new `#[cfg(test)]` module)

**Step 1: Write failing tests for pipe commands**

Add at the bottom of `src/pipe.rs`:

```rust
#[cfg(test)]
mod test {
    use std::collections::{BTreeMap, HashMap};
    use zellij_tile::prelude::{PaneInfo, PaneManifest};
    use rstest::rstest;

    use crate::config::ZellijState;
    use super::{process_line, resolve_tab_index};

    fn make_state_with_panes() -> ZellijState {
        let mut panes = HashMap::new();
        panes.insert(0, vec![
            PaneInfo { id: 10, ..PaneInfo::default() },
            PaneInfo { id: 11, ..PaneInfo::default() },
        ]);
        panes.insert(1, vec![
            PaneInfo { id: 20, ..PaneInfo::default() },
        ]);

        let mut state = ZellijState::default();
        state.panes = PaneManifest { panes };
        state
    }

    #[test]
    fn test_resolve_tab_index_found() {
        let state = make_state_with_panes();
        assert_eq!(resolve_tab_index(&state.panes, 20), Some(1));
    }

    #[test]
    fn test_resolve_tab_index_not_found() {
        let state = make_state_with_panes();
        assert_eq!(resolve_tab_index(&state.panes, 99), None);
    }

    #[test]
    fn test_set_status_valid() {
        let mut state = make_state_with_panes();
        let result = process_line(&mut state, "zjstatus::set_status::10::🤖");
        assert!(result);
        assert_eq!(state.tab_statuses.get(&0), Some(&"🤖".to_string()));
    }

    #[test]
    fn test_set_status_invalid_pane_id() {
        let mut state = make_state_with_panes();
        let result = process_line(&mut state, "zjstatus::set_status::abc::🤖");
        assert!(!result);
        assert!(state.tab_statuses.is_empty());
    }

    #[test]
    fn test_set_status_unknown_pane_id() {
        let mut state = make_state_with_panes();
        let result = process_line(&mut state, "zjstatus::set_status::99::🤖");
        assert!(result);
        assert!(state.tab_statuses.is_empty());
    }

    #[test]
    fn test_set_status_empty_emoji_clears() {
        let mut state = make_state_with_panes();
        state.tab_statuses.insert(0, "🤖".to_string());
        let result = process_line(&mut state, "zjstatus::set_status::10::");
        assert!(result);
        assert!(state.tab_statuses.get(&0).is_none());
    }

    #[test]
    fn test_clear_status() {
        let mut state = make_state_with_panes();
        state.tab_statuses.insert(1, "✅".to_string());
        let result = process_line(&mut state, "zjstatus::clear_status::20");
        assert!(result);
        assert!(state.tab_statuses.get(&1).is_none());
    }

    #[test]
    fn test_clear_status_idempotent() {
        let mut state = make_state_with_panes();
        let result = process_line(&mut state, "zjstatus::clear_status::20");
        assert!(result);
        assert!(state.tab_statuses.is_empty());
    }

    #[test]
    fn test_set_status_too_few_parts() {
        let mut state = make_state_with_panes();
        let result = process_line(&mut state, "zjstatus::set_status::10");
        assert!(!result);
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run --lib pipe::test`
Expected: FAIL — `resolve_tab_index` not found, new match arms not implemented

**Step 3: Implement `resolve_tab_index` and pipe commands**

Add function before `process_line` in `src/pipe.rs`:

```rust
use zellij_tile::prelude::PaneManifest;

fn resolve_tab_index(panes: &PaneManifest, pane_id: u32) -> Option<usize> {
    for (tab_index, pane_list) in &panes.panes {
        if pane_list.iter().any(|p| p.id == pane_id) {
            return Some(*tab_index);
        }
    }
    None
}
```

Add new match arms in `process_line`, inside the `match parts[1]` block before `_ => {}`:

```rust
"set_status" => {
    if parts.len() < 4 {
        return false;
    }
    let pane_id = match parts[2].parse::<u32>() {
        Ok(id) => id,
        Err(_) => return false,
    };
    let emoji = parts[3];
    if emoji.is_empty() {
        if let Some(tab_idx) = resolve_tab_index(&state.panes, pane_id) {
            state.tab_statuses.remove(&tab_idx);
        }
    } else if let Some(tab_idx) = resolve_tab_index(&state.panes, pane_id) {
        state.tab_statuses.insert(tab_idx, emoji.to_string());
    }
    should_render = true;
}
"clear_status" => {
    let pane_id = match parts[2].parse::<u32>() {
        Ok(id) => id,
        Err(_) => return false,
    };
    if let Some(tab_idx) = resolve_tab_index(&state.panes, pane_id) {
        state.tab_statuses.remove(&tab_idx);
    }
    should_render = true;
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo nextest run --lib pipe::test`
Expected: all PASS

**Step 5: Run full test suite**

Run: `cargo nextest run --lib`
Expected: all PASS

**Step 6: Commit**

```
feat(status): add set_status/clear_status pipe commands
```

---

### Task 3: Add `{status}` placeholder to `render_tab`

**Files:**
- Modify: `src/widgets/tabs.rs:253-292` (render_tab method)
- Modify: `src/widgets/tabs.rs:124-125` (process method — render_tab call)
- Modify: `src/widgets/tabs.rs:186` (process_click — render_tab call)

**Step 1: Update `render_tab` signature**

Change `render_tab` signature from:
```rust
fn render_tab(&self, tab: &TabInfo, panes: &PaneManifest, mode: &ModeInfo) -> String {
```
to:
```rust
fn render_tab(&self, tab: &TabInfo, panes: &PaneManifest, mode: &ModeInfo, tab_statuses: &BTreeMap<usize, String>) -> String {
```

Add import at top of file:
```rust
use std::collections::BTreeMap;
```

**Step 2: Add `{status}` substitution in `render_tab`**

Inside `render_tab`, after the `{name}` replacement block (after line 269), add:

```rust
if content.contains("{status}") {
    let status = tab_statuses
        .get(&tab.position)
        .map(|s| s.as_str())
        .unwrap_or("");
    content = content.replace("{status}", status);
}
```

**Step 3: Update call sites**

The `process` method (Widget trait impl) doesn't have access to `state.tab_statuses`. The `Widget::process` signature is `fn process(&self, name: &str, state: &ZellijState) -> String` — `state` IS available.

In `process()` method, update the `render_tab` call (line ~125):
```rust
let content = self.render_tab(tab, &state.panes, &state.mode, &state.tab_statuses);
```

In `process_click()` method, update the `render_tab` call (line ~186):
```rust
let mut rendered_content = self.render_tab(tab, &state.panes, &state.mode, &state.tab_statuses);
```

Note: `process_click` has signature `fn process_click(&self, name: &str, state: &ZellijState, pos: usize)` so `state.tab_statuses` is available.

**Step 4: Run tests**

Run: `cargo nextest run --lib`
Expected: all PASS (existing tab tests don't use `{status}`)

**Step 5: Run clippy**

Run: `cargo clippy --all-features --lib`
Expected: no warnings

**Step 6: Commit**

```
feat(status): add {status} placeholder to tab templates
```

---

### Task 4: Add cleanup on TabUpdate

**Files:**
- Modify: `src/bin/zjstatus.rs:288-296` (handle_event TabUpdate)

**Step 1: Add cleanup logic**

In `handle_event`, in the `Event::TabUpdate` arm, after `self.state.tabs = tab_info;` add:

```rust
let valid_positions: std::collections::BTreeSet<usize> =
    self.state.tabs.iter().map(|t| t.position).collect();
self.state.tab_statuses.retain(|pos, _| valid_positions.contains(pos));
```

**Step 2: Run tests**

Run: `cargo nextest run --lib`
Expected: all PASS

**Step 3: Commit**

```
feat(status): cleanup tab_statuses on TabUpdate
```

---

### Task 5: Add CLI script

**Files:**
- Create: `scripts/zellij-tab-status`

**Step 1: Write the script**

```bash
#!/usr/bin/env bash
#
# zellij-tab-status - Manage emoji status in zjstatus tab bar
#
# Usage:
#   zellij-tab-status 🤖           # Set status emoji
#   zellij-tab-status --clear      # Remove status emoji
#   zellij-tab-status --version    # Show version
#   zellij-tab-status --help       # Show help

set -euo pipefail

SCRIPT_VERSION="1.0.0"

show_help() {
    cat <<'EOF'
zellij-tab-status - Manage emoji status in zjstatus tab bar

Usage:
  zellij-tab-status <emoji>        Set status emoji
  zellij-tab-status --clear, -c    Remove status emoji
  zellij-tab-status --version, -v  Show version
  zellij-tab-status --help, -h     Show help

Requires zjstatus with {status} placeholder in tab templates.
Uses $ZELLIJ_PANE_ID from environment automatically.

Examples:
  zellij-tab-status 🤖             # Set "working" status
  zellij-tab-status ✅             # Set "done" status
  zellij-tab-status --clear        # Remove status
EOF
}

send_command() {
    local cmd="$1"
    zellij pipe --name zjstatus -- "$cmd" < /dev/null 2>/dev/null || true
}

# Commands that work outside zellij
case "${1:-}" in
    -h|--help)
        show_help
        exit 0
        ;;
    -v|--version)
        echo "zellij-tab-status $SCRIPT_VERSION"
        exit 0
        ;;
esac

# Require zellij
if [[ -z "${ZELLIJ:-}" ]]; then
    echo "Warning: Not in zellij session" >&2
    exit 0
fi

if [[ -z "${ZELLIJ_PANE_ID:-}" ]]; then
    echo "Error: ZELLIJ_PANE_ID not set" >&2
    exit 1
fi

case "${1:-}" in
    "")
        echo "Error: emoji argument required" >&2
        show_help
        exit 1
        ;;
    -c|--clear)
        send_command "zjstatus::clear_status::${ZELLIJ_PANE_ID}"
        ;;
    -*)
        echo "Unknown option: $1" >&2
        show_help
        exit 1
        ;;
    *)
        send_command "zjstatus::set_status::${ZELLIJ_PANE_ID}::$1"
        ;;
esac
```

**Step 2: Make executable**

Run: `chmod +x scripts/zellij-tab-status`

**Step 3: Commit**

```
feat(status): add zellij-tab-status CLI script
```

---

### Task 6: Manual integration test

**Step 1: Build plugin**

Run: `cargo build --target wasm32-wasip1 --release`

**Step 2: Add `{status}` to a test layout**

Edit `plugin-dev-workspace.kdl` (or a local test layout), add `{status}` to tab templates:

```kdl
tab_active   "#[fg=#89B4FA,bold] {status}{name} "
tab_normal   "#[fg=#6C7086] {status}{name} "
```

**Step 3: Launch and test**

Run: `zellij -l plugin-dev-workspace.kdl`

In a pane, run:
```bash
scripts/zellij-tab-status 🤖
# Tab should show: 🤖my-tab

scripts/zellij-tab-status --clear
# Tab should show: my-tab
```

**Step 4: Commit any layout fixes if needed**

---

### Task 7: Final clippy + tests

**Step 1:** Run: `cargo clippy --all-features --lib`
**Step 2:** Run: `cargo nextest run --lib`
**Step 3:** Verify all pass, fix any issues
