# Run all cargo checks and tests.
build:
	cargo fmt
	cargo clippy
	RUSTDOCFLAGS="-D missing_docs" cargo doc --document-private-items --no-deps
	cargo test --release

benchmark-small:
	cargo run --release --bin trane-benchmark -- --library-dir tests/small_test_library \
		--advanced-course "trane::music::improvise_for_real::sing_the_numbers::3"

benchmark-large:
	cargo run --release --bin trane-benchmark -- --library-dir tests/large_test_library \
		--advanced-course "trane::music::improvise_for_real::jam_tracks::4::g_flat"