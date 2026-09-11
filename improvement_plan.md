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
| 2          | Skip scoring queries whose results cannot matter (DONE)          | One scoring path              | Exact reduction in query count                   |
| 3          | Deduplicate exercise IDs before selection (DONE)                 | Candidate collection          | Reproduced duplicate output                      |
| 4          | Expire time-dependent score caches and align clocks (DONE)       | Small cross-module correction | Reproduced stale mastery                         |
| 5          | Reduce filesystem checks and remove redundant indexes (DONE)     | Independent small cleanups    | Source-backed, not benchmarked                   |
| Experiment | Check whether suppressed reviews have selected covering material | Bounded postselection check   | Demonstrable mismatch; learning benefit unproven |

Recommendation: implement the first four fixes before changing learning heuristics. Keep the
selected-coverage repair as a separate, explicitly experimental change.

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
