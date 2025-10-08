# NB: the project will be built if make is invoked without any arguments.
.PHONY: default
default: build

.PHONY: build
build:
	cargo build

.PHONY: sqlite ipfs
run:
	./bin/run.sh

.PHONY: check
check:
	cargo check

.PHONY: ipfs-clean sqlite-clean clean
clean:
	cargo clean

.PHONY: fmt
fmt:
	cargo fmt --all

.PHONY: fmt-check
fmt-check:
	cargo fmt --all -- --check

.PHONY: clippy
clippy:
	cargo clippy --all-targets --all-features --tests -- -D warnings

.PHONY: sqlite
sqlite:
	./bin/sqlite.sh create && \
			./bin/sqlite.sh migrate && \
				./bin/sqlite.sh queries

.PHONY: sqlite-clean
sqlite-clean:
	./bin/sqlite.sh clean

.PHONY: ipfs
ipfs-clean:
	./bin/ipfs.sh clean

.PHONY: test
test:
	cargo test --all --workspace --bins --tests --benches
