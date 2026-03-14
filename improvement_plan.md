# Trane Improvement Plan

## 1. Score-Velocity-Weighted Selection

**Literature:** The ZPDES algorithm (Clement et al., 2013-2015) uses multi-armed bandits to select
activities within a student's Zone of Proximal Development. Students reached competence
significantly earlier (p < 0.05) than with expert-designed sequences. Oudeyer's "learning progress
hypothesis" shows that intrinsic motivation and optimal learning both correlate with the *rate of
learning*, not absolute difficulty. A Bayesian ZPD framework (Chounta et al., 2017) formalizes that
tasks should be selected based on **learning progression** (the derivative of model parameters), not
just difficulty. Ericsson's deliberate practice framework (1993) shows that targeting specific
weaknesses with focused repetition is the core mechanism of expert performance development. A 2024
study in *Frontiers in Virtual Reality* found that independently adjusting difficulty per subtask
outperformed lockstep approaches (p = 0.005).

**What Trane does now:** Selects exercises based on absolute score, depth, frequency, last-seen, and
num-trials. Two exercises both scoring 3.0 are treated essentially the same regardless of whether
one is improving and the other has been stuck for many trials.

**What's different:** Track per-exercise *score velocity* — the moving average of recent score
changes. Velocity combined with absolute score partitions exercises into distinct scheduling
regimes:

- **Positive velocity** (any score) → active learning zone, boost selection.
- **Near-zero velocity + low score** → stagnation/weakness, boost selection. This is distinct from
  both the relearn pile (which catches scores of 1-2) and the base `(5.0 - score)` weight (which
  treats all exercises at the same score equally). An exercise stuck at 2.5 for 8 trials needs
  targeted attention that neither mechanism provides.
- **Near-zero velocity + high score** → mastered, reduce selection.
- **Negative velocity** → regression, trigger relearn mechanisms.

**Implementation sketch:**
- In `Candidate`, add a `score_velocity: f32` field computed from the last N trials (e.g., N=5-8).
- Velocity is computed as the OLS linear regression slope of score vs trial index. Given trials
  sorted most-recent-first as `(x_i, s_i)` where `x_i = i`, the slope is:
  `slope = (n * Σ(i * score_i) - Σi * Σscore_i) / (n * Σi² - (Σi)²)`
  where `i` is the trial index (0 = most recent, 1 = second most recent, ...), `score_i` is the
  score for trial `i`, and `n` is the number of trials used.
  Since trials are in descending order, negate the result so that positive velocity means
  improvement. The denominator is a constant for a given N. If fewer than N trials exist, use
  whatever is available.
- In `CandidateFilter::candidate_weight`, add a velocity-based weight component that accounts for
  the velocity × score interaction described above.
- This is a pure addition to the existing weighting formula — no structural changes needed.
- Optionally surface stagnation info (low velocity + low score) to the user so they know which
  skills need focused attention.

**Expected impact:** ~10-15% faster progression through material based on the ZPDES and deliberate
practice results.

---

## 2. Forward-Testing Effect Scheduling

**Literature:** The forward testing effect (Pastotter & Bauml, 2014) shows that retrieval practice
of previously studied material enhances learning of *subsequently presented new material* — even
when the materials are unrelated. The effect is reliable and generalizes across domains. In motor
learning, retrieval is identified as a fundamental process alongside reasoning and refinement
(Krakauer et al., eLife 2024). Rawson & Dunlosky's research suggests "practice recalling to an
initial criterion of 3 correct recalls, then relearn at widely spaced intervals."

**What Trane does now:** The shuffler groups exercises into course-blocked chunks (low-scoring) or
individual groups (high-scoring), then shuffles the groups uniformly at random. New/unseen exercises
have no positional bias — they can appear anywhere in the batch.

**What's different:** By biasing new exercises toward the end of the batch, the learner naturally
does retrieval practice on familiar material first. This produces the forward testing effect without
needing explicit dependency graph lookups during shuffling.

**Implementation sketch:**
- Replace the uniform group shuffle in `Shuffler::shuffle_candidates` with a keyed sort.
- Assign each group a random `f64` sort key:
  - **Existing exercises** (`num_trials > 0`): key drawn uniformly from [0.0, 1.0].
  - **New exercises** (`num_trials == 0`): key drawn uniformly from [0.5, 1.0].
- Sort all groups by their key, then flatten into the final batch order.
- This biases new exercises toward the back half of the batch while preserving randomness. New
  exercises can still appear in the middle, just not as often at the very beginning.
- The existing course-blocking for low-scoring exercises and individual grouping for high-scoring
  exercises remain unchanged — this only affects the final ordering of groups.

**Expected impact:** Moderate but well-supported. The forward testing effect is one of the most
robust findings in memory research.
