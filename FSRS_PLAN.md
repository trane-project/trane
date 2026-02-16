# FSRS Plan for `PowerLawScorer`

This document proposes incremental, low-risk improvements to `src/exercise_scorer.rs` so it keeps the current Trane-friendly design (stateless, simple, cheap) while capturing more of modern FSRS behavior.

## Goals

- Preserve Trane constraints:
  - no persisted per-exercise latent state
  - deterministic scoring from review history only
  - simple, auditable formulas
  - output remains a single score in `[0.0, 5.0]`
- Improve realism of memory dynamics:
  - spacing effect should matter
  - lapses should behave differently from successful recalls
  - stability growth should show diminishing returns
- Keep rollout safe and incremental:
  - each step can ship independently
  - each step has tests and migration notes

## Current model recap

Current `PowerLawScorer` already includes:

- power-law forgetting curve
- a difficulty estimate from failure rates
- chronological stability chaining
- recency-weighted performance multiplier

Main gaps relative to newer FSRS ideas:

- stability updates do not use inter-review intervals
- success and forgetting share one update rule
- forgetting curve uses fixed factor/decay pairs rather than deriving factor from decay
- stability growth has limited explicit diminishing-returns control
- difficulty is estimated from aggregate history rather than dynamically updated per review

## Implementation roadmap

### Step 1: Add interval-aware spacing effect (highest value)

#### Why

In FSRS, a review done at lower pre-review retrievability (`R`) typically yields larger stability gains (spacing effect). The current implementation ignores elapsed time inside stability updates.

#### Proposed change

- During stability chaining, for each review event:
  - compute elapsed days since previous review event (or since first observed point)
  - compute pre-review retrievability using current `S`
  - scale growth by a monotonic spacing term, e.g. `spacing_gain = f(1 - R)`
- Keep function stateless by reconstructing everything from history.

#### Suggested formula shape

- Let `R_prev = retrievability(delta_t, S)` before processing current review.
- For successful review:
  - `S' = S * (1 + base_growth * spacing_gain)`
  - where `spacing_gain` increases as `R_prev` decreases.
- Initial safe default:
  - `spacing_gain = 1.0 + SPACING_WEIGHT * (1.0 - R_prev)`
  - with `SPACING_WEIGHT` small (e.g. `0.3..1.0`).

#### Acceptance criteria

- Two histories with identical grades but longer spacing produce lower immediate `R` before review and higher post-review `S` after successful recall.
- No panic or instability for zero/negative intervals (clamp to `0.0`).

#### Tests to add

- `stability_spacing_effect_success`: same grades, different intervals, verify larger gain when interval is longer.
- `stability_zero_interval_is_stable`: same-day or duplicate timestamps do not break computations.

---

### Step 2: Split successful-recall vs lapse updates

#### Why

FSRS uses separate updates for recall and forgetting. Failures should not simply be "negative growth" in the same branch.

#### Proposed change

- Define a lapse threshold consistent with current scoring scale (`score < PERFORMANCE_BASELINE_SCORE`).
- Branch update in stability chaining:
  - **success branch**: multiplicative growth
  - **lapse branch**: controlled reduction/reset toward lower stability

#### Suggested formula shape

- Success:
  - `S' = S * (1 + growth_term)`
- Lapse:
  - `S' = max(MIN_STABILITY, S * (1 - lapse_drop))`
  - `lapse_drop` increases with difficulty and may increase when pre-review `R` was high (unexpected lapse).

Alternative lapse form (still simple):

- `S' = (LAPSE_BASE * difficulty_adjust * retrievability_adjust).clamp(MIN_STABILITY, S)`

#### Acceptance criteria

- A lapse causes stronger stability reduction than a mediocre successful review.
- Repeated lapses quickly depress score but remain bounded and stable.

#### Tests to add

- `stability_lapse_reduces_more_than_hard_success`
- `multiple_lapses_bounded_by_min_stability`

---

### Step 3: Derive forgetting factor from decay (FSRS-6 style invariant)

#### Why

Recent FSRS variants parameterize forgetting primarily by decay and derive factor to preserve `R(S) = 0.9`. This gives cleaner semantics and easier tuning.

#### Proposed change

- Replace hardcoded `FORGETTING_CURVE_FACTOR` for all types with:
  - one decay per exercise type (or globally)
  - `factor = 0.9f32.powf(-1.0 / decay_abs) - 1.0`
- Keep retrievability formula as power-law:
  - `R(t, S) = (1 + factor * t / S)^(-decay_abs)`

#### Notes

- Use positive `decay_abs` internally to avoid sign confusion.
- This keeps behavior interpretable when changing declarative/procedural decay rates.

#### Acceptance criteria

- Numeric check: for both exercise types, `R(t=S)` is approximately `0.9`.
- Existing trend assertions (recent > old, procedural decays slower than declarative) still hold if intended.

#### Tests to add

- `retrievability_at_stability_is_ninety_percent`
- `procedural_decay_slower_than_declarative` (if kept as product requirement)

---

### Step 4: Add explicit diminishing returns for high stability

#### Why

FSRS-6 introduces stronger saturation behavior so already-stable memories gain less from additional successful reviews.

#### Proposed change

- Add a damping term to success growth based on current `S`:
  - `damping = S.powf(-STABILITY_DAMPING_EXP)`
  - `growth_term = raw_growth * damping`
- Small exponent first (e.g. `0.1..0.3`) to avoid over-damping.

#### Acceptance criteria

- For equal review quality, `delta_S / S` is smaller when starting from high `S`.
- Long-run behavior converges smoothly instead of racing toward `MAX_STABILITY`.

#### Tests to add

- `stability_growth_saturates_at_high_s`
- `high_stability_does_not_explode`

---

### Step 5: Move difficulty from aggregate estimate to dynamic mean-reverting update

#### Why

Aggregate failure-rate difficulty is stable but blunt. FSRS-style dynamic difficulty updates capture trend while avoiding "ease hell" through mean reversion.

#### Proposed change

- Keep `difficulty` in chaining loop (reconstructed each run, still stateless externally).
- On each review:
  - apply grade-based delta
  - apply mean reversion toward base difficulty target
  - clamp to `[MIN_DIFFICULTY, MAX_DIFFICULTY]`

#### Suggested formula shape

- `D_tmp = D + grade_delta`
- `D' = mean_reversion_weight * D_target + (1 - mean_reversion_weight) * D_tmp`
- Where `grade_delta` decreases difficulty for good grades and increases it for poor grades.

#### Acceptance criteria

- Difficulty reacts to recent trend but does not drift uncontrollably.
- Sequences of strong grades generally lower effective difficulty over time; repeated failures increase it.

#### Tests to add

- `difficulty_trend_improves_with_successes`
- `difficulty_trend_worsens_with_failures`
- `difficulty_mean_reversion_prevents_runaway`

---

## Recommended rollout order

1. Step 1 (interval-aware spacing)
2. Step 2 (success/lapse split)
3. Step 3 (derived factor from decay)
4. Step 4 (diminishing returns)
5. Step 5 (dynamic difficulty)

This order gives maximum behavior gain early with minimal conceptual churn.

## Guardrails and compatibility

- Keep public trait unchanged:
  - `fn score(&self, exercise_type: ExerciseType, previous_trials: &[ExerciseTrial]) -> Result<f32>`
- Keep score range and sorting precondition unchanged.
- Use conservative defaults for new constants and tune with tests before larger shifts.
- Preserve deterministic behavior (no randomness, no external state).

## Validation checklist per step

- `cargo fmt`
- `cargo test exercise_scorer`
- `cargo test --lib`
- check monotonic sanity:
  - longer time since last review should not increase retrievability (all else equal)
  - better recent performance should not decrease final score (all else equal)
  - score remains in `[0.0, 5.0]`

## Optional future extension (not in initial scope)

- Add configurable scorer parameters in `scheduler_options`/preferences so deployments can tune constants without code changes.
- If later desired, expose intermediate diagnostics (`difficulty`, `stability`, `retrievability`) for evaluation tooling while keeping scheduler output as a scalar.
