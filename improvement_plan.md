# Trane Improvement Plan

Four high-impact strategies missing from the current design, ordered by expected impact.

## 1. Session-Aware Scheduling (estimated ~15-25% retention improvement)

### Problem

Trane has zero awareness of when sessions happen, how long they last, or cognitive
fatigue within sessions. Every call to `get_exercise_batch` is treated identically
regardless of temporal context.

### Sub-strategies

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

## Implementation Order

Recommended order based on impact-to-effort ratio:

1. **Adaptive mastery windows (simpler approach)** — local to each batch, no
   cross-batch state needed.
2. **Session-aware scheduling** — most complex, touches multiple subsystems, but
   highest total impact.

Each strategy is independent and can be implemented and tested in isolation.
