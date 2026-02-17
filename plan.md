# Improvement Plan

- Lower passing score and select a fraction of the exercises from units with scores that are just
  above that passing score and all the exercises from units with scores that approach 5.0. The
  passing / non-passing decision stops being binary and becomes a spectrum.
- Use the days since the last trial as a weighting factor for the score of a unit, so that units
  that haven't been practiced in a while are more likely to be selected.
- Now that scoring and reward propagation are in a better state, make the rewards last longer. Also,
  some kind of reward auto cleanup.

## Final plan

- Keep `PassingScoreOptionsV2` as the new scheduler path for gradual passing behavior.
- Use the V2 option with linear interpolation up to score `4.5` and keep dependency traversal
  thresholds binary.
- Remove `PassingScoreOptions` only after a wider validation pass.

## Agent todo

- [x] Add `PassingScoreOptionsV2::verify` and wire it into `SchedulerOptions::verify`.
- [x] Implement `DepthFirstScheduler::select_candidates` with linear fraction interpolation and
  a floor + minimum-one guarantee.
- [x] Apply `select_candidates` when collecting candidates from lessons.
- [x] Add unit tests for `select_candidates` behavior (empty, below threshold, partial, and forced-one
  cases).
- [ ] Decide whether `min_score` should be exposed as a user-facing field in configuration after V2 rollout.
