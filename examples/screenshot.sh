#!/usr/bin/env bash
# Makes the screenshot in FORK.md (assets/tab-groups.png): the bar of examples/groups.kdl,
# rendered by the plugin's own code (examples/screenshot.rs), drawn by a browser.
# Needs wasmtime, Python 3, ImageMagick, Google Chrome (or $CHROME) and the
# JetBrainsMono Nerd Font Mono font.
set -euo pipefail
cd "$(dirname "$0")/.."
chrome="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cargo build --quiet --example screenshot
# The example never calls Zellij, whose imports the plugin library still declares.
wasmtime run -W unknown-imports-trap=y target/wasm32-wasip1/debug/examples/screenshot.wasm >"$work/bar.ansi"
python3 examples/screenshot.py <"$work/bar.ansi" >"$work/bar.html"
"$chrome" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=2 \
  --window-size=1100,400 --screenshot="$work/bar.png" "file://$work/bar.html" 2>/dev/null
magick "$work/bar.png" -trim +repage -bordercolor '#11111b' -border 32 assets/tab-groups.png
echo assets/tab-groups.png
