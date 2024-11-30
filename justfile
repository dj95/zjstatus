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
run target="zjstatus": build
  #!/usr/bin/env bash
  case "{{target}}" in
  "zjframes")
    zellij -s zjframes-dev --config ./tests/zjframes/config.kdl -n ./tests/zjframes/layout.kdl
    ;;
  *)
    zellij -s zjstatus-dev --config ./tests/zjstatus/config.kdl -n ./tests/zjstatus/layout.kdl
    ;;
  esac

# Watch and run tests with nextest.
test:
  cargo watch -x "nextest run --lib"

# Lint with clippy and cargo audit.
lint:
  cargo clippy --all-features --lib
  cargo audit

# Create and push a new release version.
release:
  #!/usr/bin/env bash
  export VERSION="$( git cliff --bumped-version )"
  cargo set-version "${VERSION:1}"
  direnv exec . cargo build --release
  git commit -am "chore: bump version to $VERSION"
  git tag -m "$VERSION" "$VERSION"
  git push origin main
  git push origin "$VERSION"
