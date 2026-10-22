# Trane Improvement Plan

Review date: 2026-10-21. Reviewed revision: `6f5f84a`.

## Scope

This plan covers speed improvements only. No scheduling policy, scoring math, graph
semantics, or mastery-window behavior changes. Where caching is introduced, it preserves
the exact outputs of the existing computation.

This plan supersedes the 2026-09-05 review, whose priority items 1–5 are merged
(`#436`–`#439`). The previously open experiment ("verify selected coverage") remains
open and untouched by this plan; see git history for its details.

All evidence below comes from the repository flamegraph (simulation run) and source
inspection. Items are grouped by effort-to-reward ratio, best first.

## Priority Order

| Priority | Group | Improvement                                          | Effort  | Expected reward                    |
| -------- | ----- | ---------------------------------------------------- | ------- | ---------------------------------- |
| A1       | Quick | Lazy error construction in hot getters (DONE)        | Minutes | ~3–5% (2.7% eager formatting)      |
| A2       | Quick | Cache the four uncached SQLite statements (DONE)    | Minutes | Small; removes reprepare overhead  |
| B1       | Cache | Per-batch memoization of dependency satisfaction     | Medium  | Largest structural win (~18%+2.3%) |
| B2       | Cache | Read cache for reward retrieval                      | Medium  | ~3.4% (2 queries per score miss)   |
| C1       | Load  | Course builder write-path cleanup                    | Medium  | Library import time                |
| C2       | Load  | Course library load-path cleanup                     | Medium  | Library load time                  |
| D1       | Opt   | SQLite connection pragmas                            | Small   | Small; measure first               |

## Group B — Structural Wins (identical outputs)

### B1. Per-batch memoization of dependency-satisfaction checks

During the DFS in `get_candidates_from_graph_helper`, the chain
`get_valid_dependents` → `all_satisfied_dependencies` → `satisfied_dependency`
(`src/scheduler.rs:538-605`) re-resolves and re-checks the same dependency IDs many
times across different dependents. Each check re-runs `resolve_effective_dependencies`
(fresh `visited` set, recursive set building) and `satisfied_effective_dependency`
(blacklist locks, supersession, score lookups). Together these account for roughly 20%
of samples.

Add a memo map keyed by dependency ID and filter, scoped to a single
`get_exercise_batch` call. Scores, blacklist, and supersession only change between
batches via `score_exercise`, so results within one batch computation are identical.
This is caching, not an algorithm change.

Caveat to document in code: the score caches are time-freshness-based, so the memo must
be created and discarded within one batch computation and never reused across calls.

### B2. Read cache for reward retrieval

`LocalPracticeRewards::get_rewards_helper` (`src/practice_rewards.rs:161-187`) runs a
SQLite query on every call. It is only gated by the scorer's score cache, so every
exercise-score cache miss costs two queries (lesson and course rewards), 3.4% of
samples.

Mirror the existing write-side `RewardCache` for reads, invalidated in
`record_unit_rewards_helper`, the only in-process writer.

## Group C — Import-Time Wins

These only affect building and loading course libraries, not steady-state scheduling.

### C1. Course builder write path (`src/course_builder.rs`)

- Remove the duplicate `create_dir_all` per asset (lines 45 and 55).
- Share one root `VfsPath` instead of constructing a new `PhysicalFS` per
  exercise/lesson/course (lines 98, 155, 206).
- Replace per-exercise and per-lesson template `.clone()` with `Arc` (lines 151, 202).
- Reduce `verify_paths` stat overhead (two stat syscalls per asset path today,
  `src/data.rs:273-280`, `:723-753`).

### C2. Course library load path (`src/course_library.rs:422-487`)

- Skip non-manifest files during `walk_dir` instead of visiting every asset file.
- Hoist the per-entry `format!("{}/")` ignored-path allocations out of the loop
  (lines 462-465).
- Reduce the `is_dir`/`is_file` double-stat pattern in `process_course_manifest`
  (lines 321-337) and `process_lesson_manifest` (lines 252-272).

## Group D — Optional, Measure First

### D1. SQLite connection pragmas

`new_connection()` (`src/utils.rs:19-27`) sets WAL and `synchronous=OFF`. Consider
adding `busy_timeout` (correctness for multi-process access) and a larger `cache_size`.
Measure before and after; expected effect is small.

Deliberately excluded: batching the per-score write transactions in `score_exercise`
(`src/scheduler.rs:1104-1166`). Scores are recorded one row at a time by user action,
and durability motivates the current commit-per-score behavior.

## Validation

After each group:

1. `cargo test --release` — all 297 tests must pass.
2. `make benchmark-small` and `make benchmark-large` — compare before/after timings.
3. Regenerate the flamegraph after Group B to confirm the `all_satisfied_dependencies`
   shrink.
4. `make lint` before finishing each group.

Suggested order: B → C → D, with one commit per group so each win is measurable
independently.
