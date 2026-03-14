# Trane Improvement Plan

## 1. Learning-Progress-Weighted Selection (ZPDES / Multi-Armed Bandit)

**Literature:** The ZPDES algorithm (Clement et al., 2013-2015) uses multi-armed bandits to select
activities within a student's Zone of Proximal Development. Students reached competence
significantly earlier (p < 0.05) than with expert-designed sequences. Oudeyer's "learning progress
hypothesis" shows that intrinsic motivation and optimal learning both correlate with the *rate of
learning*, not absolute difficulty. A Bayesian ZPD framework (Chounta et al., 2017) formalizes that
tasks should be selected based on **learning progression** (the derivative of model parameters), not
just difficulty.

**What Trane does now:** Selects exercises based on absolute score, depth, frequency, last-seen, and
num-trials. Two exercises both scoring 3.0 are treated essentially the same.

**What's different:** Track per-exercise *score velocity* — the moving average of recent score
changes. An exercise that went 2.0 → 3.0 recently is in the student's active learning zone and
should be favored over one stuck at 3.0 for many trials. This adds a temporal derivative dimension
to selection that currently doesn't exist.

**Implementation sketch:**
- In `Candidate`, add a `score_velocity: f32` field computed from the last N trials' score deltas.
- In `CandidateFilter::candidate_weight`, add a velocity-based weight component: positive velocity →
  selection boost, near-zero velocity → refer to weakness targeting, negative velocity → trigger
  relearn mechanisms.
- This is a pure addition to the existing weighting formula — no structural changes needed.

**Expected impact:** ~10-15% faster progression through material based on the ZPDES results.

---

## 2. Stagnation Detection and Weakness Prioritization

**Literature:** Ericsson's deliberate practice framework (1993) shows that targeting specific
weaknesses with focused repetition is the core mechanism of expert performance development. A 2024
study in *Frontiers in Virtual Reality* found that the Two-Up/One-Down Unlocked (2U1D-UL) algorithm,
which allows *independent subtask difficulty adjustment*, outperformed lockstep approaches (p =
0.005). The critical insight: different subskills progress at different rates, and systems that
detect and independently prioritize lagging subskills outperform uniform scheduling.

**What Trane does now:** Lower-scored exercises get higher weight in candidate selection via `(5.0 -
score)`. The relearn pile handles recent failures. But there's no mechanism to detect *stagnation* —
exercises where the student is stuck (multiple trials without improvement).

**What's different:** Stagnation is distinct from both low scores (which get weight naturally) and
recent failures (which go to the relearn pile). An exercise scored 2.5 for the last 8 trials is a
weakness that needs specific attention — but it won't trigger the relearn pile (only scores 1-2 do)
and its weight is identical to any other 2.5-scored exercise.

**Implementation sketch:**
- Compute a `stagnation_score` per exercise: number of recent trials with score change < epsilon.
- In `candidate_weight`, add a stagnation bonus that boosts selection of stagnant exercises.
- Optionally surface stagnation info to the user so they know which skills need focused attention.

**Expected impact:** ~10-15% based on the deliberate practice and adaptive training literature.

---

## 3. Forward-Testing Effect Scheduling

**Literature:** The forward testing effect (Pastotter & Bauml, 2014) shows that retrieval practice
of previously studied material enhances learning of *subsequently presented new material* — even
when the materials are unrelated. The effect is reliable and generalizes across domains. In motor
learning, retrieval is identified as a fundamental process alongside reasoning and refinement
(Krakauer et al., eLife 2024). Rawson & Dunlosky's research suggests "practice recalling to an
initial criterion of 3 correct recalls, then relearn at widely spaced intervals."

**What Trane does now:** The shuffler blocks low-scoring same-course exercises together and scatters
high-scoring exercises. The DFS traversal touches prerequisites before dependents. But there's no
explicit mechanism to *sequence prerequisite retrieval immediately before new dependent material*.

**What's different:** The forward testing effect says that practicing a partially-mastered
prerequisite *right before* first exposure to a dependent exercise actively enhances learning of the
new exercise. This is a specific ordering optimization within a batch, not just difficulty
balancing.

**Implementation sketch:**
- In the shuffler, when a batch contains both a new/unseen exercise and a partially-mastered
  exercise from a prerequisite lesson, position the prerequisite exercise immediately before the new
  one.
- This is a post-filter ordering optimization in `Shuffler::shuffle_candidates` that uses the
  dependency graph to inform sequencing.

**Expected impact:** Moderate but well-supported. The forward testing effect is one of the most
robust findings in memory research.

---

## 4. Contextual Interference via Related-Skill Interleaving

**Literature:** A 2024 meta-analysis in *Scientific Reports* found that high contextual interference
(random/interleaved practice) produces a medium effect on retention (SMD = 0.63, p < 0.001). The
forgetting-reconstruction hypothesis (Lee & Magill, 1983) explains why: switching between related
tasks forces the learner to forget and reconstruct the action plan, building stronger memory traces
through effortful reconstruction. Critically, this is about interleaving *related but different*
skills, not just mixing difficulty levels.

**What Trane does now:** Interleaves difficulty levels (low-scoring exercises blocked by course,
high-scoring scattered). But doesn't specifically interleave *related skills from different lessons
within the same course*.

**What's different:** The CI literature specifically says the benefit comes from switching between
*related* tasks that require different action plans. Two exercises from the same lesson test the
same skill — the CI benefit comes from interleaving exercises from *different* lessons that share a
parent course or dependency relationship.

**Implementation sketch:**
- In the shuffler, for high-scoring exercises, prefer orderings that alternate between different
  lessons within the same course (or lessons connected by dependency edges).
- For low-scoring exercises, the current blocking behavior is correct — CI research shows that
  blocking is better for initial acquisition of complex skills.

**Important caveat:** A sub-additive interaction finding from Bjork's lab shows that variation is
beneficial primarily at shorter spacing intervals. At longer intervals, variation may not add
benefit.

**Expected impact:** ~5-10% retention improvement for review exercises, minimal for new material.
