.PHONY: check fmt test lint coverage gate

check:
	cargo check --workspace

fmt:
	cargo fmt --all --check

test:
	cargo test --workspace -q

coverage:
	cargo llvm-cov --package sl-core --package sl-parser --package sl-compiler --package sl-runtime --lib --fail-under-lines 89.9 --fail-under-functions 90

lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

gate: check fmt test lint coverage
