 1) Adaptive mastery-window percentages
- Why: Fixed windows in `src/data.rs` and static filtering in `src/scheduler/filter.rs` cannot react to students who are temporarily struggling or cruising.
- Change: Add dynamic window adjustment per batch using recent performance (e.g., rolling average of recent scored exercises in current run).
- Behavior: If recent performance is low, shift allocation from `new/target` to `current/easy`; if high, shift some from `easy/mastered` to `target/new`; always renormalize to sum to 1.0 and enforce floor/ceiling bounds.
- Touchpoints: `src/scheduler/filter.rs`, `src/scheduler.rs`, `src/data.rs` (new scheduler options for adaptation strength, bounds, and target band).
- Done when: End-to-end simulation shows fewer “stuck” runs on low-scoring personas while preserving progression for high-scoring personas.

 2) Due-ness/retrievability in candidate weighting
- Why: Candidate weighting in `src/scheduler/filter.rs` uses score/frequency/depth but not explicit due-ness, so overdue items can be under-prioritized.
- Change: Add candidate fields for recency (`last_trial_timestamp`) and/or retrievability estimate, then add a due-ness term to `candidate_weight`.
- Behavior: For similar score candidates, older/less-retrievable items get modestly higher selection odds; keep cap so due-ness does not overwhelm mastery-window balance.
- Touchpoints: `src/scheduler.rs` (populate candidate metadata), `src/scheduler/unit_scorer.rs` (expose recency with cached score data), `src/scheduler/filter.rs` (new weight component).
- Done when: Probabilistic tests show older equivalent candidates are selected more often, without collapsing diversity.

3) Progressive unlock with early exposure (preview lane)
- Why: Strict evidence gates can delay new material and increase frustration, even when a learner is ready for partial exposure.
- Change: Add fractional unlock behavior to passing score policy. Instead of a binary lock/unlock, compute an `exercise_fraction` in [0, 1] for near-unlock dependents and cap total preview share per batch.
- Behavior:
  - Keep full unlock logic at `exercise_fraction = 1.0`.
  - Use a ramp (for example, 3.5 to 3.8) where `exercise_fraction` increases smoothly from 0.0 to 1.0.
  - Sample exercises from partially unlocked units proportional to `exercise_fraction`, with a global preview cap.
  - Do not tighten superseding logic in this change.
- Touchpoints:
  - `src/data.rs`: extend `PassingScoreOptions` with fractional unlock options (for example, unlock ramp / `exercise_fraction` policy) and preview cap options.
  - `src/scheduler/unit_scorer.rs`: add helpers to compute per-unit `exercise_fraction` from score and policy.
  - `src/scheduler.rs`: dependency traversal and candidate selection use `exercise_fraction` while enforcing preview cap.
- Done when: One-off high scores can surface some blocked material early, sustained strong performance reaches `exercise_fraction = 1.0` quickly, and superseding behavior remains unchanged.

 4) Path-aggregated reward propagation
- Why: Reward propagation currently de-duplicates by first visit, making outcomes path-order sensitive (`src/scheduler/reward_propagator.rs`).
- Change: Aggregate contributions from multiple valid paths (net sum or controlled combine), instead of first-hit wins.
- Behavior: Traversal remains bounded by existing stop conditions, but repeated independent evidence reinforces or cancels signals deterministically.
- Touchpoints: `src/scheduler/reward_propagator.rs`, `src/practice_rewards.rs` (if storing aggregate metadata is helpful), unit tests for invariance under traversal order.
- Done when: Reordered edge traversal yields identical propagated rewards for same graph/score input.

 5) Exponential reward decay + persistent anti-repeat frequency
- Why: Reward decay is linear (`src/reward_scorer.rs`), and anti-repeat scheduling frequency is in-memory only (`src/scheduler/data.rs`), so restart resets spacing pressure.
- Change: Use half-life-based exponential decay for rewards and persist schedule frequency counters across sessions.
- Behavior: Recent rewards matter most in a smooth way; anti-repeat behavior survives process restarts but can also decay over time to avoid permanent penalties.
- Touchpoints: `src/reward_scorer.rs`, `src/scheduler/data.rs`, plus a small persistent store/migration (new table/module or extension in existing storage layer).
- Done when: Restarting Trane preserves near-term anti-repeat behavior and reward effects feel less abrupt over time.

 6) Simulation-based parameter tuning harness
- Why: Many constants are hand-tuned across scorer/filter/propagation; tuning quality can drift as algorithm pieces evolve.
- Change: Add an offline harness that runs deterministic simulations over generated and hand-crafted libraries, then reports key metrics and candidate constant sets.
- Metrics: progression speed, retention proxy, review diversity, repetition rate, and stall frequency.
- Touchpoints: `src/test_utils.rs`, `tests/large_tests.rs` (or dedicated ignored benchmark-style tests), optional output artifact for comparison.
- Done when: A reproducible run can recommend parameter updates and catch regressions before merge.
 7) Best-first/beam traversal (replace DFS default)
- Why: DFS with random stack order and early stop can overexplore whichever branch happens to be popped first (`src/scheduler.rs`).
- Change: Add a best-first or beam frontier using a utility score (readiness, due-ness, novelty/diversity, anti-repeat).
- Behavior: Explore more promising frontier nodes first while preserving coverage and respecting dependency constraints.
- Touchpoints: `src/scheduler.rs` (new traversal mode and frontier scoring), `src/data.rs` (strategy option/feature flag), integration tests for progression and batch diversity.
- Done when: In simulations, branch starvation decreases and progression consistency improves versus DFS baseline.

 Suggested PR Sequence
- PR1: Adaptive mastery windows.
- PR2: Due-ness/retrievability weighting.
- PR3: Progressive unlock with preview lane.
- PR4: Path-aggregated reward propagation.
- PR5: Exponential decay + persistent anti-repeat frequency.
- PR6: Tuning harness.
- PR7: Best-first/beam traversal experiment behind option flag.
