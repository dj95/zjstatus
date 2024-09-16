# Default recipe, when nothing's selected
[private]
default:
  just --choose

# Run benchmarks and compare to previous runs.
bench:
  cargo bench --features=bench

# Build zjstatus with the tracing feature enabled.
build:
  cargo build --features tracing

# Build zjstatus with tracing and start a zellij session with the dev layout.
run: build
  zellij -l ./plugin-dev-workspace.kdl -s zjstatus-dev

# Watch and run tests with nextest.
test:
  cargo watch -x "nextest run --lib"

# Lint with clippy and cargo audit.
lint:
  cargo clippy --all-targets --all-features
  cargo audit

# Create and push a new release version.
release version:
  cargo set-version {{version}}
  direnv exec . cargo build --release
  git commit -am "chore: bump version to v{{version}}"
  git tag -m "v{{version}}" v{{version}}
  git push origin main
  git push origin "v{{version}}"
