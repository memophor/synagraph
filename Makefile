# SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
# Common developer workflows. Use `make <target>`.

.PHONY: fmt lint test check run all ci

fmt:
	cargo fmt

lint:
	cargo clippy --all-targets --all-features -- -D warnings

test:
	cargo test

check:
	cargo check --all-targets --all-features

run:
	cargo run

all: fmt lint test

ci:
	cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test
