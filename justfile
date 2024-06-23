bench:
  cargo bench --features=bench

build:
  cargo build --features tracing

run: build
  zellij -l ./plugin-dev-workspace.kdl -s zjstatus-dev

test:
  cargo watch -x "nextest run --lib"

lint:
  cargo clippy --all-targets --all-features
  cargo audit

release version:
  cargo set-version {{version}}
  direnv exec . cargo build --release
  git commit -am "chore: bump version to v{{version}}"
  git tag -m "v{{version}}" v{{version}}
  git push origin main
  git push origin "v{{version}}"
