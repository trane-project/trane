# Review Knocker Improvement Plan

## Core Change: Composite Knocker Score

Replace the current two-dimensional (frequency threshold, exercise score threshold) check with
a single composite knocker score computed from multiple factors:

### Factors for the composite score

1. **Encompassed frequency (weighted):** How much this exercise is covered by others in the
   batch. Use fractional encompassing weights instead of raw counts, so the frequency becomes
   an `f32` weighted sum rather than a `u32` count.

2. **Encompassing frequency (weighted):** How much this exercise covers other due exercises in
   the batch. This is the missing "supply side" — exercises that provide the most implicit
   coverage of other due items should be prioritized (lower knocker score).

3. **Exercise score:** Higher scores make an exercise more likely to be knocked out.

4. **num_trials as stability proxy:** Exercises with high score but few trials have uncertain
   mastery and should be harder to knock out. High score + high num_trials = confident mastery
   = safer to knock out. No need to expose PowerLawScorer internals.

### Three tiers from two thresholds

- **Above high threshold:** Completely knocked out (removed from candidates).
- **Above low threshold:** Put in the highly encompassed pile (merged into mastered window in
  filter).
- **Below low threshold:** Keep in normal candidate pool, but pass the knocker score through to
  `filter.rs` as an additional weight component in `candidate_weight`.

### What this replaces

Currently `review_knocker.rs` uses:
- `VERY_HIGHLY_SCORE` (4.5) + `VERY_HIGHLY_FREQUENCY` (10) for full knockout
- `HIGHLY_SCORE` (3.75) + `HIGHLY_FREQUENCY` (5) for the highly encompassed pile

The composite score subsumes all of these and makes it easy to add new factors later without
changing the tier logic.

## Additional Ideas (lower priority)

- **Adaptive thresholds:** Scale knockout thresholds relative to batch size rather than using
  fixed values.
- **Per-exercise-type retention targets:** Procedural vs declarative exercises could use
  different knocker score weights.
- **Leech detection:** Exercises with many trials but oscillating/stagnant low scores waste
  review time and could be flagged.
