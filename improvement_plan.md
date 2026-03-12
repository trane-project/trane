# Trane Improvement Plan

Four high-impact strategies missing from the current design, ordered by expected impact.

## 1. Session-Aware Scheduling (estimated ~15-25% retention improvement)

### Problem

Trane has zero awareness of when sessions happen, how long they last, or cognitive
fatigue within sessions. Every call to `get_exercise_batch` is treated identically
regardless of temporal context.

### Sub-strategies

#### 1a. Consolidation-Aware Spacing

Enforce a minimum gap (e.g., 12 hours) before re-presenting the same exercise.
Research shows a 12-hour spacing interval produces ~24% less forgetting after one week
compared to shorter gaps.

`last_seen` is already tracked in `Candidate` and used as a filtering weight, but
there is no hard exclusion. Exercises seen very recently can still appear.

**Implementation sketch:**
- Add a configurable `min_retest_hours` field to `SchedulerOptions` (default: 12).
- In `get_candidates_from_lesson_helper`, filter out exercises where `last_seen` is
  below the threshold (converting hours to days for comparison with the existing
  `last_seen` field).
- This is a lightweight change: a single filter predicate in the candidate generation
  path.

#### 1b. Within-Session Fatigue Detection

Track score trends within a session. If scores are declining over the last N exercises,
the user is fatiguing and the batch should shift toward easier review or the session
should end.

**Implementation sketch:**
- Add a `session_scores: Vec<(Ustr, f32, i64)>` field to `SchedulerData` (or a
  separate `SessionTracker` struct) that accumulates `(exercise_id, score, timestamp)`
  entries during a session.
- Define a session boundary as a gap of >30 minutes between consecutive
  `score_exercise` calls (configurable).
- After each `score_exercise`, compute a rolling average of the last N scores. If it
  drops below a threshold relative to the session's initial average, set a
  `fatigued: bool` flag on the scheduler.
- When `fatigued` is true, `CandidateFilter::filter_candidates` shifts the mastery
  window distribution toward easier exercises (increase the "easy" and "mastered"
  percentages, decrease "new" and "target").
- Expose a `fn is_fatigued(&self) -> bool` on `ExerciseScheduler` so the client can
  decide whether to suggest ending the session.

#### 1c. Time-of-Day Material Selection

Harder/newer material benefits most from overnight consolidation. If session time is
known, prioritize new material in evening sessions and review in morning sessions.

**Implementation sketch:**
- Add an optional `session_time_hint: Option<TimeOfDay>` enum (`Morning`, `Afternoon`,
  `Evening`) to the `get_exercise_batch` call or to `SchedulerOptions`.
- When set to `Evening`, increase the "new" and "target" mastery window percentages.
  When set to `Morning`, increase the "easy" and "mastered" percentages.
- This requires no new data storage, just the current time at batch generation. The
  client can pass this or the scheduler can derive it from `Utc::now()`.

### Files to modify

- `src/data.rs` — add `min_retest_hours` to `SchedulerOptions`, add `TimeOfDay` enum.
- `src/scheduler/data.rs` — add session tracking state.
- `src/scheduler.rs` — filter by `min_retest_hours` in candidate generation.
- `src/scheduler/filter.rs` — adjust mastery window percentages based on fatigue and
  time-of-day signals.

---

## 2. Successive Relearning (estimated ~10-20% retention improvement)

### Problem

When a user scores 1-2 on an exercise, it returns to the general candidate pool. The
next time it appears could be the next session or much later. Research on successive
relearning shows that "retrieve until correct, then space" produces substantially
stronger memories than single retrieval attempts. The critical window is within the same
session.

### Current behavior

The `dead_end` flag in `get_candidates_from_graph` marks entire lessons as dead-ends
when the average score is below passing. This is a lesson-level mechanism. There is no
exercise-level mechanism for prioritizing recently-failed individual exercises.

The `frequency` field in `Candidate` tracks how often an exercise has been scheduled and
is used to decrease selection probability. A "needs-relearning" flag would do the
opposite.

### Implementation sketch

- Add a `relearn_queue: Arc<RwLock<VecDeque<Ustr>>>` to `SchedulerData` that holds
  exercise IDs needing immediate re-presentation.
- In `score_exercise`, if the score is below a configurable threshold (e.g., 2.0), push
  the exercise ID onto the relearn queue.
- In `score_exercise`, if the score is at or above the threshold and the exercise is in
  the relearn queue, remove it (the learner succeeded on retry).
- In `get_exercise_batch`, before filtering candidates through mastery windows, inject
  exercises from the relearn queue into the final batch (up to a configurable fraction
  of `batch_size`, e.g., 20%).
- These injected exercises bypass the normal mastery window distribution since their
  purpose is immediate re-testing, not spaced review.
- Add a `relearn_threshold` field to `SchedulerOptions` (default: 2.0) and a
  `relearn_fraction` field (default: 0.2).

### Files to modify

- `src/data.rs` — add `relearn_threshold` and `relearn_fraction` to
  `SchedulerOptions`.
- `src/scheduler/data.rs` — add `relearn_queue` field to `SchedulerData`.
- `src/scheduler.rs` — inject relearn exercises in `get_exercise_batch`, manage queue
  in `score_exercise`.

---

## 3. Adaptive Mastery Windows (estimated ~10-15% faster progression)

### Problem

Trane uses fixed mastery window percentages (new: 20%, target: 20%, current: 30%,
easy: 20%, mastered: 10%) defined in `CandidateFilter`. These do not adapt to the
individual learner's current performance level.

Research (Wilson et al., 2019, Nature Communications,
https://www.nature.com/articles/s41467-019-12552-4) shows optimal learning occurs at ~85% success
rate. A user breezing through everything gets too many easy exercises. A struggling user gets too
many hard ones.

### Current behavior

The mastery windows are defined in `SchedulerOptions::mastery_windows` and applied
statically in `CandidateFilter::filter_candidates`. The percentages are fixed for the
lifetime of the scheduler unless the client manually calls `set_scheduler_options`.

### Implementation sketch

- After each batch is scored (i.e., after all exercises in a batch have been scored via
  `score_exercise`), compute the batch's weighted success rate.
- Track a rolling average of recent batch success rates in `SchedulerData`.
- Before generating the next batch, adjust the mastery window percentages:
  - If rolling success rate > 90%: shift distribution toward harder windows (increase
    "new" and "target", decrease "easy" and "mastered").
  - If rolling success rate < 75%: shift distribution toward easier windows (increase
    "easy" and "mastered", decrease "new" and "target").
  - If between 75% and 90%: keep current distribution (in the optimal zone).
- The adjustment should be gradual (e.g., shift by 2-5 percentage points per batch) to
  avoid oscillation.
- Add a `target_success_rate` field to `SchedulerOptions` (default: 0.85) and an
  `adaptive_windows: bool` toggle (default: true).

### Alternative: simpler approach

Rather than tracking batch success rates explicitly, use the distribution of candidate
scores already computed during `get_candidates_from_graph`. If the majority of
candidates fall in the high-score windows, shift the batch composition toward more new
and target exercises. This avoids needing cross-batch state entirely — the adaptation
is local to each batch based on the candidate pool available.

### Files to modify

- `src/data.rs` — add `target_success_rate` and `adaptive_windows` to
  `SchedulerOptions`.
- `src/scheduler/data.rs` — add rolling success rate tracking.
- `src/scheduler/filter.rs` — adjust percentages before applying window selection.
- `src/scheduler.rs` — update rolling average in `score_exercise`.

---

## 4. Uncertainty-Aware Scoring (estimated ~10-15% scheduling precision)

### Problem

`PowerLawScorer` returns a point estimate (single float). An exercise scored 5.0 once
is indistinguishable from one scored 5.0 twenty times. The scheduler cannot distinguish
"probably mastered" from "definitely mastered."

Without uncertainty, the system over-trusts sparse data. One high score on a new
exercise can cause it to be scheduled as "mastered" and rarely seen again, even though
the evidence is extremely weak.

### Current behavior

`num_trials` is tracked in `Candidate` and used as one factor in the
`CandidateFilter` weighting, but it does not directly modulate the score itself. The
`PowerLawScorer` computes a weighted average and retrievability without expressing
confidence in that estimate.

### Implementation sketch

#### Option A: Score variance penalty

- In `PowerLawScorer::score`, compute the variance of recent trial scores alongside
  the weighted average.
- Apply a confidence penalty: `effective_score = score - k * variance / sqrt(n)` where
  `k` is a configurable constant and `n` is the number of trials. This pulls
  high-variance, low-trial-count scores downward.
- This is a conservative estimator: it assumes the true score is at the lower end of
  the confidence interval. The effect diminishes as evidence accumulates.

#### Option B: Separate confidence signal

- Add a `confidence: f32` field (0.0-1.0) to the score output, computed as
  `1.0 - 1.0 / (1.0 + alpha * num_trials)` where `alpha` controls how quickly
  confidence saturates.
- In `CandidateFilter`, use confidence to modulate window assignment: low-confidence
  high-score exercises are placed in the "current" window rather than "mastered",
  ensuring they get re-tested sooner.
- This keeps the score and confidence separate, which is cleaner but requires threading
  the confidence value through more of the pipeline.

#### Recommended: Option A

Option A is simpler (modifies only the scorer) and achieves the main goal of preventing
premature promotion of under-tested exercises. Option B is cleaner architecturally but
requires changes across the scorer, candidate, and filter layers.

### Files to modify

- `src/exercise_scorer.rs` — add variance computation and confidence penalty to
  `PowerLawScorer::score`.
- `src/data.rs` — add `confidence_penalty_k` to `SchedulerOptions` (default: 0.5).

---

## Implementation Order

Recommended order based on impact-to-effort ratio:

1. **Successive relearning** — smallest change, high impact, isolated to queue
   management and batch injection.
2. **Uncertainty-aware scoring (Option A)** — single file change in the scorer, no
   pipeline changes needed.
3. **Adaptive mastery windows (simpler approach)** — local to each batch, no
   cross-batch state needed.
4. **Session-aware scheduling** — most complex, touches multiple subsystems, but
   highest total impact.

Each strategy is independent and can be implemented and tested in isolation.
