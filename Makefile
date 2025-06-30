# Run all cargo checks and tests.
build:
	cargo fmt
	cargo clippy
	RUSTDOCFLAGS="-D missing_docs" cargo doc --document-private-items --no-deps
	cargo test --release
