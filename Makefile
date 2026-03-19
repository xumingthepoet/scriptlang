.PHONY: check fmt test lint gate

check:
	cargo check --workspace

fmt:
	cargo fmt --all --check

test:
	cargo test --workspace -q

lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

gate: check fmt test lint
