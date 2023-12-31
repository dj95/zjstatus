bench:
  #!/usr/bin/env bash
  benchmarks="$(cargo bench --target wasm32-wasi --features=bench --no-run --color=always 2>&1 | tee /dev/tty | grep -oP 'target/.*.wasm')"

  echo "$benchmarks" \
    | xargs -I{} wasmtime --dir $PWD/target::target {} --bench --color=always

build:
  cargo build

run: build
  zellij -l ./plugin-dev-workspace.kdl -s zjstatus-dev

test:
  cargo component test -- --nocapture

lint:
  cargo clippy --all-targets -- -D warnings
  cargo audit

release version:
  cargo set-version {{version}}
  direnv exec . cargo build --release
  git commit -am "chore: bump version to v{{version}}"
  git tag -m "v{{version}}" v{{version}}
  git cliff --current
