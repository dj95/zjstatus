#!/bin/sh
# Build the release wasm, then run this. It fails when a drive script renders
# bytes other than the files in tests/snapshots/out.
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
wasm="$root/target/wasm32-wasip1/release/zjstatus.wasm"
if [ ! -f "$wasm" ]; then
  echo "missing $wasm" >&2
  echo "cargo build --release --target wasm32-wasip1 --bin zjstatus" >&2
  exit 1
fi
command -v zellij-plugin-snapshot >/dev/null || {
  echo "install Zellij Plugin Snapshot 0.2.3: cargo install zellij-plugin-snapshot --version 0.2.3 --locked" >&2
  exit 1
}
out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT
for script in "$root"/tests/snapshots/scripts/*.yaml; do
  zellij-plugin-snapshot "$script" --out "$out"
done
diff -ru "$root/tests/snapshots/out" "$out"
