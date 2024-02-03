# Run all build steps.
build: build-ffi build-cargo

# Build and verify the FFI bindings.
build-ffi:
	typeshare ./ --lang=typescript --output-file=ffi/trane.ts
	tsc ffi/trane.ts

# Run all cargo checks and tests.
build-cargo:
	cargo fmt
	cargo clippy
	RUSTDOCFLAGS="-D missing_docs" cargo doc --document-private-items --no-deps
	cargo test --release
