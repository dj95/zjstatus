# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

zjstatus — customizable statusbar plugin for the [Zellij](https://zellij.dev/) terminal multiplexer. Compiles to WASM (`wasm32-wasip1`). The repo also contains **zjframes** — a companion plugin that toggles pane frames based on conditions.

## Build & Test Commands

```bash
# Build (release, WASM target)
cargo build --target wasm32-wasip1 --release

# Run tests (uses cargo-nextest, NOT cargo test)
cargo nextest run --lib

# Clippy (CI enforces -Dwarnings)
cargo clippy --all-features --lib

# Local development with Zellij
zellij -l plugin-dev-workspace.kdl
```

Rust toolchain: **1.91.0**, edition 2024. Target: `wasm32-wasip1`. CI runs clippy and nextest in `.github/workflows/lint.yml`.

## Architecture

Two binaries (`src/bin/`), one library (`src/lib.rs`):

**Binaries:**
- `zjstatus.rs` — main statusbar plugin. Implements `ZellijPlugin` trait (load → update → render cycle). Holds `State` with `ZellijState`, `ModuleConfig`, and a `BTreeMap<String, Arc<dyn Widget>>`.
- `zjframes.rs` — pane frame toggler. Separate lightweight plugin.

**Library modules:**
- `config.rs` — `ZellijState` (runtime state: tabs, mode, panes, command results), `ModuleConfig` (parsed layout config with left/center/right parts), `UpdateEventMask` (bitmask for selective re-rendering).
- `render.rs` — `FormattedPart` struct with ANSI styling (fg/bg/effects). Uses `#[cached]` for parsed format strings. Renders widgets by replacing `{widget_name}` placeholders in format strings via regex.
- `widgets/` — each widget implements `trait Widget { fn process(&self, name, state) -> String; fn process_click(&self, name, state, pos); }`. Widgets: command, datetime, mode, tabs, session, swap_layout, pipe, notification.
- `border.rs` — border rendering config.
- `frames.rs` — frame visibility logic.
- `pipe.rs` — pipe message protocol handling.

**Key patterns:**
- Events are queued in `pending_events` until permissions are granted, then replayed.
- `UpdateEventMask` bitmask controls which widgets re-render on which events, avoiding unnecessary work.
- `FormattedPart` parsing is cached with `#[cached]` (SizedCache).
- Configuration comes from Zellij's KDL layout files via `userspace_configuration` BTreeMap.
- Tests use `rstest` for parameterized cases. Test modules are `#[cfg(test)]` inline.

## Configuration

Users configure zjstatus through KDL layout files. The plugin reads `userspace_configuration` from Zellij and parses format strings like `#[fg=blue,bg=red] {mode} {tabs}"`. Widget placeholders `{name}` are replaced during render.
