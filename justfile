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
    zellij \
      -s zjstatus-dev \
      --config ./tests/zjstatus/config.kdl \
      -n ./tests/zjstatus/layout.kdl
    ;;
  esac

# Watch and run tests with nextest.
test:
  cargo watch -x "nextest run --lib --bin zjstatus"

# Lint with clippy and cargo audit.
lint:
  cargo clippy --all-features --lib
  cargo audit

# Create and push a new release version, e.g. v0.25.0-ade.1.
release:
  #!/usr/bin/env bash
  set -euo pipefail
  # The upstream release this is based on plus a fork counter, so it never clashes with upstream tags.
  git fetch --quiet --tags origin
  git fetch --quiet --tags upstream
  base="$( git describe --tags --abbrev=0 --match 'v[0-9]*' --exclude '*-*' )"
  last="$( git tag --list "${base}-ade.*" | sed 's/.*-ade\.//' | sort -n | tail -n 1 )"
  export VERSION="${base}-ade.$(( ${last:-0} + 1 ))"
  cargo set-version "${VERSION:1}"
  cargo build --release
  git commit -am "chore: bump version to $VERSION"
  git tag -m "$VERSION" "$VERSION"
  git push origin main
  git push origin "$VERSION"
