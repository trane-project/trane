# Improvement Plan: Batch Writes Above Storage Modules

## Goal

Reduce end-to-end runtime of scheduler-heavy simulations, especially `cargo test --release --test
large_tests all_exercises_scheduled_random`, by decreasing transaction/commit overhead on the write
path without merging the SQLite databases.

## Current Context

The main confirmed low-refactor scheduler optimizations attempted in this conversation did not
produce meaningful wins:

- Precomputing lesson reward once per lesson regressed performance because it eagerly computed
  reward state before exercise-cache hits.
- Batch-local dependency/supersession caches in the scheduler produced little or no meaningful
  benchmark improvement.

The remaining high-value area is the write path in `score_exercise`.

## Current Write Path

Each call to `DepthFirstScheduler::score_exercise` currently performs multiple independent writes:

- `practice_deltas.record_exercise_delta(...)`
- `practice_stats.record_exercise_score(...)`
- `practice_rewards.record_unit_rewards(...)`

These are logically related updates for one scored exercise, but they are written through separate
storage modules and committed independently. Even though the benchmark ultimately performs the same
number of logical inserts, the cost of many small SQLite transactions is likely much higher than
fewer larger transactions.

## Recommended Direction

Implement write batching **above** the storage modules, not inside them.

This means:

- The scheduler or session layer accumulates pending writes in memory.
- Storage modules remain simple persistence layers.
- Flush happens explicitly at meaningful boundaries.

This was chosen as the better architecture over storage-local buffers because:

- batching policy belongs to the workflow/session layer
- it keeps storage modules focused on persistence
- it makes flush boundaries explicit
- it makes coordinated flush across stats/deltas/rewards possible

## Proposed Architecture

Introduce a batch-write coordinator in the higher-level flow that owns pending records for:

- practice stats writes
- practice delta writes
- practice reward writes

Possible shape:

- a `PendingWrites` / `WriteBatch` struct in the scheduler/session layer
- append new records during each `score_exercise`
- flush when one of the following happens:
  - pending write count reaches a threshold
  - session ends
  - graceful shutdown occurs

Do **not** hide long-lived pending buffers inside each storage module as the primary design.

## Correctness Constraint

The critical requirement is that reads after writes must still behave correctly during the session.

If writes are deferred, subsequent logic must still see fresh state for:

- latest exercise scores
- deltas
- rewards

That means the implementation needs either:

1. in-memory read-through behavior for pending writes, or
2. a higher-level session state that uses pending writes directly and does not rely on immediate DB
   visibility

This point is essential. Batching is only useful if it preserves scheduler behavior.

## Suggested First Scope

Start with batching `practice_stats` first.

Reason:

- it is written on every scored exercise
- it is likely the hottest guaranteed write
- it allows validation of the batching approach before extending it to deltas and rewards

If that produces meaningful improvement, extend the same model to:

- `practice_deltas`
- `practice_rewards`

## Flush Policy

Use explicit flush boundaries in the higher-level runtime:

- flush every N writes, for example 32 or 64
- flush on session end
- flush on graceful shutdown

For shutdown:

- `Ctrl-C` / terminal interrupt should trigger a graceful shutdown path
- GUI close should trigger the same graceful shutdown path
- that path should flush pending writes before exit

Note:

- crashes, `SIGKILL`, power loss, and hard termination cannot be fully protected against
- batching reduces durability granularity, so the flush threshold should be small enough to keep the
  loss window acceptable

## Benchmarking Guidance

Use the existing end-to-end benchmark target for truth:

- `cargo test --release --test large_tests all_exercises_scheduled_random`

But also consider adding a narrower benchmark or timing harness for:

- repeated `score_exercise` throughput
- scheduler-only `get_exercise_batch` throughput

This will make it easier to separate write-path improvements from scheduler-read-path improvements.

## Explicit Non-Goals For This Pass

- Do not merge the SQLite databases.
- Do not continue the eager lesson-reward optimization; it regressed benchmark performance.
- Do not continue the batch-local dependency cache optimization unless new evidence suggests a real gain.

## Summary

The next serious optimization target is write batching at the scheduler/session layer. The expected
win comes from reducing commit frequency, not reducing the number of logical records. The most
promising first step is batching `practice_stats` while preserving immediate logical visibility of
new writes during the session.
