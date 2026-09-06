# Trane Improvement Plan

Review date: 2026-09-05. Reviewed revision: `be41416`.

## Scope

The strongest new opportunities are small implementation fixes, not another scheduling rewrite.
This review identified three reproducible issues, a scoring-query optimization, two smaller
performance opportunities, and one scheduling-policy experiment worth investigating cautiously.

The review recovered the deleted `FSRS_PLAN.md`, `plan.md`, `improvements.md`, and seven rounds of
`improvement_plan.md`: 22 distinct content versions. Recommendations below exclude the mechanisms
proposed in those documents, including unimplemented and discarded variants.

All recommended fixes preserve Trane's graph traversal, mastery windows, encompassing
relationships, and general-purpose mastery grades. No implementation changes were made during
this review.

## Priority Order

| Priority   | Improvement                                                      | Scope                         | Evidence                                         |
| ---------- | ---------------------------------------------------------------- | ----------------------------- | ------------------------------------------------ |
| 1          | Restore JSON read buffering (DONE)                               | Four parsing sites            | Measured large parsing speedup                   |
| 2          | Skip scoring queries whose results cannot matter                 | One scoring path              | Exact reduction in query count                   |
| 3          | Deduplicate exercise IDs before selection                        | Candidate collection          | Reproduced duplicate output                      |
| 4          | Expire time-dependent score caches and align clocks              | Small cross-module correction | Reproduced stale mastery                         |
| 5          | Reduce filesystem checks and remove redundant indexes            | Independent small cleanups    | Source-backed, not benchmarked                   |
| Experiment | Check whether suppressed reviews have selected covering material | Bounded postselection check   | Demonstrable mismatch; learning benefit unproven |

Recommendation: implement the first four fixes before changing learning heuristics. Keep the
selected-coverage repair as a separate, explicitly experimental change.

## 2. Skip Irrelevant Scoring Queries

On an exercise-cache miss, [`get_exercise_score`](src/scheduler/unit_scorer.rs#L199) fetches trials,
deltas, lesson rewards, and course rewards before determining whether all that information is
useful.

Existing rules already establish:

- With zero trials, the exercise scorer returns the unseen-exercise result without examining
  deltas: [`src/exercise_scorer.rs:451`](src/exercise_scorer.rs#L451).
- With two or fewer trials, rewards never apply:
  [`src/reward_scorer.rs:117`](src/reward_scorer.rs#L117).

| Retrieved trials | Current storage queries | Necessary queries |
| ---------------- | ----------------------: | ----------------: |
| 0                |                       4 |                 1 |
| 1-2              |                       4 |                 2 |
| 3+               |                       4 |                 4 |

### Proposed Change

After the existing cache-hit fast path and trial retrieval, skip unused delta/reward retrieval
and reward aggregation. Keep the reward-eligibility rule centralized rather than duplicating an
unexplained threshold.

For 10,000 unseen exercises actually scored on cold caches, this means 40,000 to 10,000 storage
queries, without changing scores or scheduling policy. This is an operation-count reduction, not
a measured runtime claim.

This is distinct from the previously rejected eager lesson-reward precomputation: it does no
additional work before cache hits and introduces no new cache.

### Constraints And Validation

- Independently trimmed trial and delta histories can differ. Skipping deltas is safe for empty
  trials, not automatically for every short trial history.
- Preserve cached score, urgency, velocity, and trial count.
- Verify exact output equality for histories with 0, 1, 2, 3, and 20 trials, including populated
  parent rewards and independently trimmed histories.
- Count storage calls on cold caches and confirm cache-hit work remains unchanged.
- Mature exercises with three or more retrieved trials receive no query-count improvement.

## 3. Deduplicate Before Sampling

Overlapping review-list entries are expanded independently in
[`src/scheduler.rs:922`](src/scheduler.rs#L922). A course, one of its lessons, and one of that
lesson's exercises can therefore contribute the same exercise multiple times.

Weighted sampling operates on candidate entries, not unique exercise IDs. The later deduplication
only prevents relearn entries from duplicating normal selections.

### Reproduction

Create one course, one lesson, and one exercise. Add all three units to the review list. With no
practice history or failures, the returned batch was:

```text
["0::0::0", "0::0::0"]
```

This occurred in 20 of 20 batches: two entries, one unique exercise, and no relearning
contribution.

### Proposed Change

Deduplicate the candidate union by exercise ID before knockout and weighted selection. Resolve
differing path metadata consistently, rather than letting review-list iteration order decide it.

Deduplicating only the final output is insufficient: duplicate candidates would still get
multiple sampling opportunities and could distort window allocation.

The expected benefit is removal of unintended repeated presentations and path-dependent
sampling bias. This does not ban intentional retries after an actual attempt.

### Scientific Context

[Karpicke and Roediger (2008)](https://pubmed.ncbi.nlm.nih.gov/18276894/) found that repeated
retrieval can improve delayed retention. The recommendation is not that repetition is wasteful;
it is that overlapping organizational paths should not silently prescribe extra repetitions.

The effect on learning efficiency is unmeasured and depends on what replaces the accidental
duplicate.

### Validation

- Cover overlapping course, lesson, and exercise review-list entries.
- Cover repeated lesson IDs in a lesson filter.
- Assert uniqueness before sampling, not only in the returned batch.
- Preserve intentional later attempts through the relearn mechanism.
- Check deterministic handling of differing candidate metadata for the same exercise.

## 4. Make Cached Scores Respect Time

This is the most important learning-related correctness issue found in this review.

Exercise mastery and urgency depend on time, but their caches do not expire. Changing the
timestamp override also leaves caches intact:
[`src/scheduler/unit_scorer.rs:88`](src/scheduler/unit_scorer.rs#L88) and
[`src/scheduler/unit_scorer.rs:179`](src/scheduler/unit_scorer.rs#L179). Cached lesson and course
aggregates inherit the problem.

### Reproduction

Record one grade-five trial, evaluate scores, advance the effective clock 30 days without
practicing, then invalidate the exercise.

| State                      | Exercise | Lesson | Course |
| -------------------------- | -------: | -----: | -----: |
| Immediately after practice |    5.000 |  5.000 |  5.000 |
| After advancing 30 days    |    5.000 |  5.000 |  5.000 |
| After cache invalidation   |    2.699 |  2.699 |  2.699 |

The harness used `T = 1800000000`, advanced by exactly 2,592,000 seconds, and observed a
recomputed score of `2.6993408` for all three unit types after invalidating only the exercise ID.

The problem is not a disagreement about the forgetting model. The cache prevents the existing
model from running.

There is a related clock inconsistency: reward decay and recent-performance protections use
wall-clock time even when exercise scoring uses simulated time:
[`src/reward_scorer.rs:94`](src/reward_scorer.rs#L94) and
[`src/reward_scorer.rs:128`](src/reward_scorer.rs#L128).

### Proposed Change

Invalidate derived exercise, lesson, and course scores when an explicit timestamp override
changes, and introduce bounded freshness for ordinary long-lived instances. Use the same
effective evaluation time for exercise and reward scoring.

Preserve time-independent caches where possible. Avoid indiscriminately clearing everything on
every lookup. Choose and document a freshness tolerance; the cited literature does not determine
a cache lifetime.

Clock consistency does not mean recording a later answer with the earlier batch-generation
timestamp. Practice events should retain their actual event timestamps.

### Scientific Context

- [Cepeda et al. (2008)](https://pubmed.ncbi.nlm.nih.gov/19076480/) demonstrated that retention
  depends strongly on the relationship between practice spacing and the retention horizon.
- [Tatel and Ackerman (2025)](https://pubmed.ncbi.nlm.nih.gov/40455501/) synthesized 1,344 effect
  sizes from 457 reports on procedural skills involving motor components, finding time-related
  decay and important task-dependent moderators. Temporal accuracy matters beyond flashcards.
- [Lindsey et al. (2014)](https://journals.sagepub.com/doi/10.1177/0956797613504302) found 10%
  higher retention under personalized review than generic spaced review in a semester-long
  Spanish study with equal review-trial allocations. This is a relative improvement, not a
  percentage-point gain.

These studies support taking temporal state seriously. They do not establish a cache lifetime,
validate Trane's exact forgetting function, or predict a particular reduction in Trane reviews.

Fixing this may increase some immediate reviews while preventing avoidable forgetting and later
relearning. Its practical impact is greatest for long-lived clients and simulations; restarting
Trane already discards caches.

### Validation

- Advance time without writing new history and verify that exercise and aggregate scores reflect
  the existing scorer's new projections.
- Check urgency as well as mastery.
- Compare a long-running instance and a fresh instance at the same effective timestamp.
- Verify reward half-life and recent-performance protections under the injected clock.
- Measure the cost of the selected expiration policy and preserve same-time cache hits.

## 5. Smaller Performance Wins

### Check Filenames Before Filesystem Type

[`src/course_library.rs:441`](src/course_library.rs#L441) calls `entry.is_dir()` before checking
whether the name is `course_manifest.json`. The VFS walker already checks entry metadata, and
`is_dir()` adds existence/metadata checks.

Moving the filename rejection first avoids those additional probes for unrelated assets. This
is particularly attractive for asset-heavy physical libraries and requires almost no structural
change. Preserve the directory rejection for matching names.

Validate with VFS operation counts and identical discovered courses. This opportunity was not
benchmarked during the review.

### Remove Duplicate SQLite Indexes

These tables declare `unit_id` as `UNIQUE`, then create another ordinary index on the same column:

- [`src/practice_stats.rs:54`](src/practice_stats.rs#L54)
- [`src/practice_rewards.rs:111`](src/practice_rewards.rs#L111)
- [`src/blacklist.rs:51`](src/blacklist.rs#L51)
- [`src/review_list.rs:39`](src/review_list.rs#L39)

SQLite already supplies an index for the unique constraint. Append migrations dropping
`unit_ids` in stats/rewards and `unit_id_index` in blacklist/review-list storage, preserving the
constraints and history indexes. Do not rewrite historical migrations.

This saves one redundant B-tree per affected database and reduces new-ID write amplification.
It is primarily a space improvement, not a major steady-state scheduling speedup. Freed pages
become reusable; file sizes need not shrink immediately.

Validate query plans and uniqueness with the bundled SQLite version, and measure reclaimed
index pages separately from compacted file size. No live query-plan or database-size experiment
was performed during this review.

## 6. Experiment: Verify Selected Coverage

This is the one new scheduling-policy direction worth investigating, but it should not be
immediately adopted as a proven improvement.

The knocker computes coverage from the initial candidate pool, removes or demotes reviews, and
only afterward does the filter choose the actual batch:
[`src/scheduler.rs:1058`](src/scheduler.rs#L1058).

Therefore, a review can be excluded because encompassing exercises were available, even though
none of those exercises survives into the returned batch. Coverage-based weighting makes
selecting them more likely but does not guarantee it.

### Counterexample

A one-hop construction uses five candidates: target A, encompassing exercises B1/B2/B3, and
unrelated U. Each belongs to a distinct lesson and course. All scores are 4.5. Let A's lesson and
course be LA and CA. Use only these explicit encompassing edges, all with weight 1:

| Source | Its lesson encompasses | Its course encompasses |
| ------ | ---------------------- | ---------------------- |
| B1     | LA and CA              | LA and CA              |
| B2     | LA and CA              | LA and CA              |
| B3     | LA                     | CA                     |
| U      | Nothing                | Nothing                |

LA and CA have no outgoing edges. The current calculation gives LA weight 5 and CA weight 5,
so A receives weight 10 and is fully knocked out.

With `batch_size = 1`, these candidates supplied through a lesson filter, and no relearning,
the filter selects one surviving mastered candidate. U has positive selection weight, so the
returned batch can be `[U]`: neither A nor any covering exercise is present.

This is a source-derived counterexample, not an executed learner experiment. It establishes a
possible zero-selected-coverage case, not starvation or learning harm.

### Narrow Experiment

Inspect fully suppressed reviews for zero coverage from the selected batch. Test a bounded
repair, such as at most one same-window substitution, preserving batch size and protecting
existing covering selections.

Do not simply reapply the current thresholds of 10 and 5 to the final batch. They currently
measure candidate abundance; applying them to small returned batches would fundamentally weaken
the knocker.

Implementation constraints:

- Retain a bounded reserve of suppressed candidates and their original window information.
- Separate selected propagation sources from suppressed query targets. The current encompassing
  helper uses one candidate slice for both; passing their union would incorrectly seed coverage
  from unselected material.
- Include selected relearn exercises in the audit and protect intentional relearn selections.
- Bound traversal and repair work. An incomplete audit is not proof of zero coverage.
- Avoid iterative knockout/reselection until convergence. Replacing a provider can uncover
  another review, changing quotas, stochastic behavior, and runtime.

### Scientific Context And Limits

[Pan and Rickard's 2018 meta-analysis](https://pubmed.ncbi.nlm.nih.gov/29733621/) found that
retrieval benefits can transfer, but transfer varies substantially with task and response
characteristics. [Corral et al. (2023)](https://pubmed.ncbi.nlm.nih.gov/36941495/) further showed
that advantages on practiced problems need not translate into analogous problem-solving
advantages.

That supports caution about treating alternatives as interchangeable. It does not prove this
repair improves Trane. The studies do not directly measure encompassing practice versus direct
practice of its components.

There is a legitimate counterargument: abundant encompassing material may provide enough
practice across successive batches, without covering every suppressed review in each batch.
A repair could consequently add unnecessary direct reviews. Measure before adopting.

This differs from the earlier weighted-knocker proposals: the change concerns which providers
were actually selected, not new coverage weights, confidence penalties, or thresholds.

## Validation Cautions

Before using `days_to_mastery` to judge scheduling changes, account for two existing limitations.

### Runtime Option Propagation

Runtime option changes update only top-level scheduler data, while components retain cloned
options: [`src/scheduler.rs:1183`](src/scheduler.rs#L1183). The benchmark uses this setter, so
some requested parameter changes may not reach the component being evaluated.

### Simulated Learning Outcomes

Simulated grades depend on trial count and a fixed lapse probability, not elapsed forgetting or
transfer: [`src/benchmark.rs:210`](src/benchmark.rs#L210). Mastery is then checked through
Trane's own scores at [`src/benchmark.rs:239`](src/benchmark.rs#L239). Faster attainment of that
threshold is not independent evidence of better human learning.

These are interpretation constraints, not a recommendation to revive the previously proposed
simulation/tuning framework.

More generally, declarative-recall studies cannot directly calibrate procedural or complex-skill
mastery. Vocabulary correctness, strategy selection, motor accuracy, speed, and integrated
performance are different outcomes. No quantified learning benefit is claimed for these changes.

## Verification Performed

- `cargo test --offline --release --lib`: 297 passed, zero failures.
- A temporary Rust harness reproduced stale exercise/lesson/course caches, duplicate batches,
  and the JSON read reduction. All harness assertions passed.
- No full integration suite or human-learning comparison was run.
- No implementation files were modified during the review.

The temporary harness was created outside the repository at:

```text
/var/folders/2j/9639h1v158s1g967981cssch0000gn/T/opencode/trane-validation/
```

The harness was run with:

```sh
cargo run --offline --release --quiet \
  --manifest-path "/var/folders/2j/9639h1v158s1g967981cssch0000gn/T/opencode/trane-validation/Cargo.toml" \
  --target-dir "/Users/martinmr/workspace/trane/target"
```

This temporary path is recorded for local reproducibility, not as a durable repository artifact.
The reproduction setups and measured results are preserved above.
