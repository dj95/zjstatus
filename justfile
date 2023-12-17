bench:
  cargo wasi bench --

build:
  cargo build

test:
  cargo wasi test -- --nocapture

lint:
  cargo clippy --all-targets --all-features -- -D warnings
  cargo audit

release version:
  cargo set-version {{version}}
  cargo build --release
  git commit -am "chore: bump version to {{version}}"
  git tag -m "{{version}}" {{version}}
  git cliff --current
