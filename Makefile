.PHONY: build test test-integration

WASM_TARGET = wasm32-wasip1
WASM_ARTIFACT = target/$(WASM_TARGET)/release/zjstatus.wasm
DOCKER_IMAGE = zjstatus-test

build:
	cargo build --target $(WASM_TARGET) --release

test:
	cargo nextest run --lib

test-integration: build
	docker build -f Dockerfile.test -t $(DOCKER_IMAGE) .
	docker run --rm \
		-v "$$(pwd)/$(WASM_ARTIFACT):/test/plugin.wasm:ro" \
		-v "$$(pwd)/tests/integration:/test/tests:ro" \
		$(DOCKER_IMAGE) \
		/test/tests/docker-test-runner.sh
	docker run --rm \
		-v "$$(pwd)/$(WASM_ARTIFACT):/test/plugin.wasm:ro" \
		-v "$$(pwd)/tests/integration:/test/tests:ro" \
		$(DOCKER_IMAGE) \
		/test/tests/test-race-runner.sh
