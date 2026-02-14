# AGENTS: Repository Guide for Agentic Coding

This file gives practical instructions and style expectations for automated agents working in this
repository (trane). It covers build / lint / test commands (including how to run one test), and the
project's code-style and error-handling conventions so agents make consistent, review-ready edits.

## Repository basics

- Language: Rust (edition 2024).
- Build system: Cargo. Root manifest: `Cargo.toml`.
- CI: GitHub Actions workflows live under `.github/workflows/` (see `build.yml` and
  `coverage.yaml`). They show canonical commands used in CI and should be followed locally for
  reproducible results.

## Quick commands

- Build release: `cargo build --release`.
- Run full test suite (as CI does): `cargo test --release`.
- Run tests in debug (faster during development): `cargo test`.
- Run library tests only during quick iterations: `cargo test --lib`.
- Run a single test by name (filter): `cargo test <filter>`.
  - Exact test name: `cargo test <test_name> -- --exact`.
  - Run a test from a specific module / path: `cargo test module::submodule::test_name`.
  - If you need test output printed to stdout, add `-- --nocapture` after the cargo args.
- Formatting check: `cargo fmt --all -- --check` (CI uses this to fail on format
  issues).
- Format in-place: `cargo fmt`.
- Lint with clippy (CI): `cargo clippy -- -D warnings`.
- Generate docs (CI runs rustdoc lints):
  - `RUSTDOCFLAGS="-D missing_docs -D rustdoc::missing_doc_code_examples" cargo doc --workspace --all-features --no-deps --document-private-items`
- Running a single test (examples)
  - Run the unit test named `library_root` defined in `src/lib.rs`:
    - `cargo test library_root -- --exact`
  - Run tests that match `scheduler_options` pattern:
    - `cargo test scheduler_options`
  - If a test is in an integration test file under `tests/`, cargo still filters by
    name: `cargo test my_integration_test -- --nocapture`.

## Project conventions and style

These rules are intended so that agents produce changes that match project expectations and pass CI
without human intervention.

### Formatting

- Use `rustfmt` (Cargo provides `cargo fmt`). The CI enforces `cargo fmt --check`.
- Keep line lengths reasonable (rustfmt default). Prefer clarity over squeezing multiple expressions
  into one line.

### Imports

- Group imports roughly as: `std` -> external crates -> internal crate (`crate::...`).
- Use explicit imports where practical. Avoid glob imports (`use foo::*;`) except in
  tests or very small private modules where they materially improve clarity. Note:
  `lib.rs` currently permits `clippy::wildcard_imports` in lint exceptions but new
  code should prefer explicit imports.
- When importing multiple items from the same module prefer the grouped form:
  - `use crate::data::{CourseManifest, ExerciseManifest};`
- Use `crate::...` for intra-crate imports (this repository follows that pattern).

### Module and file layout

- File names: snake_case (e.g., `exercise_scorer.rs`).
- Modules should have module-level documentation (//! at top) explaining purpose and
  rationale; this project is documented liberally and expects maintainable docs.

### Naming

- Follow the standard Rust naming conventions:
  - Types and traits: PascalCase (e.g., `LocalPreferencesManager`, `SchedulerData`).
  - Functions and variables: snake_case (e.g., `create_scheduler_options`).
  - Constants: SCREAMING_SNAKE or UPPER_SNAKE for module-level consts (project uses
    `TRANE_CONFIG_DIR_PATH`, `PRACTICE_STATS_PATH`). Keep them `pub` only when needed.

### Error handling

- Prefer domain-specific error enums exported from `src/error.rs` for public APIs.
  - Example: `PreferenceManager` methods return `Result<T, PreferencesManagerError>`.
- Internally (private functions), `anyhow::Result` is commonly used for convenience.
  Convert to domain errors when crossing a public boundary.
- When creating or propagating errors use `anyhow::Context` to attach helpful messages
  (e.g., `fs::File::open(path).context("failed to open config file")?`).
- Use `anyhow::bail!` or `anyhow::ensure!` for early returns when appropriate.
- Use `thiserror` to define error enums with `#[source]` for wrapped errors. Keep
  user-facing error messages concise and include the failing subject when helpful.

## Result and Option usage

- Public interfaces typically return `Result<T, DomainError>`. Avoid returning
  `anyhow::Error` from public APIs; use concrete error types defined in `error.rs`.
- Prefer `Option<T>` when absence is a normal state; use `Result` for operations that
  can fail due to I/O, parsing, or other exceptional conditions.

## Data changes

- Changes to the core structures (manifests) should be made with care and ideally be
  backward-compatible. If a breaking change is necessary stop and ask for human review before
  proceeding.

## Ownership and concurrency

- Use `Arc<RwLock<...>>` for shared, mutable resources across components as the
  codebase does (see `Trane` struct in `src/lib.rs`). Prefer `parking_lot::RwLock`
  (already in dependencies) for performance and ergonomics.

## Testing

- Unit tests live inline (#[cfg(test)]) and use `tempfile` for temporary files/dirs.
- Follow existing patterns: use `anyhow::Result` for test functions returning Result.
- When adding tests prefer deterministic behavior; avoid reliance on network or
  external services. If external calls are required, mark them or mock them.
- Integration tests exist under `tests/`. The testing strategy is described in detail in
  `basic_tests.rs`.

## Clippy

- CI enables `clippy::pedantic` with a few explicitly disabled lints in
  `src/lib.rs`. Agents should run `cargo clippy` locally and fix warnings.
- If a proposed change triggers a pedantic lint, prefer changing the code to satisfy
  the lint or add a focused `#[allow(...)]` with a short justification comment.

## Documentation

- Public types, functions, and traits should have doc comments explaining semantics.
- Keep module-level rationale and examples where they help (crate already contains thorough
  top-level docs in `src/lib.rs`).
- CI runs `cargo doc` with `RUSTDOCFLAGS` to deny missing docs and missing code examples.
- Code blocks contain short and terse comments for the purpose of easily navigating the code for
  both humans and agents. Any generated code should follow this convention. Below are some examples
  from the code.

```rust
    /// Returns whether the superseded unit can be considered as superseded by the superseding
    /// units.
    pub(super) fn is_superseded(&self, superseded_id: Ustr, superseding_ids: &UstrSet) -> bool {
        // Units with no superseding units are not superseded.
        if superseding_ids.is_empty() {
            return false;
        }

        // All the exercises from the superseded unit must have been seen at least once.
        if !self.all_valid_exercises_have_scores(superseded_id) {
            return false;
        }

        // All the superseding units must have a score equal or greater than the superseding score.
        let scores = superseding_ids
            .iter()
            .filter_map(|id| self.get_unit_score(*id).unwrap_or_default())
            .collect::<Vec<_>>();
        scores
            .iter()
            .all(|score| *score >= self.data.options.superseding_score)
    }
```

```rust
    /// Returns the number of trials that were considered when computing the score for the given
    /// exercise.
    pub(super) fn get_num_trials(&self, exercise_id: Ustr) -> Result<Option<usize>> {
        // Return the cached value if it exists.
        let cached_score = self.exercise_cache.borrow().get(&exercise_id).cloned();
        if let Some(cached_score) = cached_score {
            return Ok(Some(cached_score.num_trials));
        }

        // Compute the exercise's score, which populates the cache. Then, retrieve the number of
        // trials from the cache.
        self.get_exercise_score(exercise_id)?;
        let cached_score = self.exercise_cache.borrow().get(&exercise_id).cloned();
        Ok(cached_score.map(|s| s.num_trials))
    }
```

## Commit and PR guidance for agents

- Unless instructed otherwise, do not create commits or PRs.
- Make small, focused changes in a single PR. Ensure `cargo fmt` and `cargo clippy` succeed locally
  before proposing changes.
- When changing public APIs, update docs and `src/error.rs` if new domain errors are added.
- Add or update unit tests for behavior changes; keep tests reproducible and fast.

## Files of interest (for agents)

### General structure and tools

- `Cargo.toml` - dependency and edition information.
- `src/lib.rs` - crate entry, top-level lints and many exceptions; follow its patterns.
- `src/error.rs` - domain error definitions. Use it for public-facing errors.
- `.github/workflows/build.yml` - canonical build, test, and lint commands used in CI.
- `.github/workflows/coverage.yaml` - canonical coverage command.

### Core modules

All modules are located directly under `src/` and following standard Rust module conventions.

- `data`: core data structures for defining exercises, lessons, courses, options, etc.
- `graph`: defines the graph of dependencies, how it's created and accessed.
- `course_library`: defines the set of courses available to the student and how they are stored and
  retrieved.
- `scheduler`: core scheduling logic for selecting exercises.
  - This includes code to filter selected exercises into the final batch and to propagate rewards
    through the graph.
- `reward_scorer`: defines how rewards are scored based on practice stats and unit rewards.
- `blacklist`: manages blacklisted units (units that are ignored and marked as mastered).
- `exercise_scorer`: scores exercises based on past performance.
- `unit_scorer`: scores units based on their exercises and rewards. Caches scores for efficiency.
- `practice_stats`: stores student progress
- `practice_rewards`: stores unit rewards.

### Other modules

- `error`: domain error definitions for public APIs.
- `preferences_manager`: manages user preferences and settings.
- `repository_manager`: manages how to add course repositories and retrieve courses from them.
- `filter_manager`: saves user-defined filters for exercise selection.
- `course_builder`: utilities for making building courses easier.
- `review_list`: manages exercises for later review.
- `study_session_manager`: manages study sessions and their state.
- `scheduler_options`: defines options for the scheduler and how they are stored and retrieved.
- `test_utils`: utilities for testing.

## If you are blocked

- Run `cargo test -q` to get a succinct failing test list.
- Run `cargo clippy --fix -Z unstable-options` only if you understand changes and CI
  will accept them — do not rely on auto-fixes for pedantic lints.
- If you are unsure about a change or get stuck in a loop, stop and ask for human review before
  proceeding.

