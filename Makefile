# Run all build steps.
build: generate-bindings build-cargo

# Build and verify the TypeScript bindings.
generate-bindings:
	rm -rf bindings
	cargo test --lib export_bindings
	tsc --allowJs --noEmit bindings/*

# Run all cargo checks and tests.
build-cargo:
	cargo fmt
	cargo clippy
	RUSTDOCFLAGS="-D missing_docs" cargo doc --document-private-items --no-deps
	cargo test --release
