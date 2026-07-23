.PHONY: fmt fmt-check lint check test test-full build clean help

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

lint:
	cargo clippy --all-targets --all-features -- -D warnings

check:
	cargo check --all-targets --all-features

test:
	cargo test --all

test-full:
	cargo test --all --all-features

build:
	cargo build --release

clean:
	cargo clean

help:
	@echo "Usage: make <target>"
	@echo ""
	@echo "  fmt         Format all code (auto-fix)"
	@echo "  fmt-check   Check formatting"
	@echo "  lint        Run clippy (all targets, all features, -D warnings)"
	@echo "  check       cargo check (all targets, all features)"
	@echo "  test        Run all tests"
	@echo "  test-full   Run all tests with all features"
	@echo "  build       Release build"
	@echo "  clean       Remove build artifacts"
