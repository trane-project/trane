# Trane Improvement Plan

## 1. Graph-Informed Cold Start Estimation

**The problem.** When an exercise has zero trials, its score is 0.0. It goes into the "new" mastery
window (score range 0.0-0.1) alongside every other untried exercise, regardless of context. The
system has no way to distinguish between:
- A genuinely unknown exercise in a brand-new domain
- An exercise in a lesson whose prerequisites the student has mastered at 5.0
- An exercise in a lesson where the student already scored 5.0 on 8 of 10 exercises

All three get score 0.0 and compete for the same "new" window slots. This is especially wasteful
when encompasses relationships exist: if exercise X is encompassed by exercise Y and Y is scored
5.0, the student almost certainly already knows X -- but the system won't discover this until X gets
a trial.

**What's different.** For exercises with zero (or very few) trials, compute a *prior score* from the
graph neighborhood: sibling exercises in the same lesson, prerequisite exercises, and encompassing
exercises. This prior represents "what we'd expect the student to score on this exercise given
everything else we know about them."

**Algorithm:**
- Define `compute_prior(exercise_id) -> Option<f32>`:
  1. **Sibling signal**: average score of other exercises in the same lesson, if any have trials.
     Weight: 0.5.
  2. **Prerequisite signal**: average score of exercises in dependency lessons of this lesson.
     Weight: 0.3.
  3. **Encompassing signal**: for each unit that encompasses this exercise's lesson, take the
     weighted average of its exercises' scores (weighted by the encompasses edge weight).
     Weight: 0.2.
  4. Return the weighted combination, or `None` if no signals are available.
- **Blending**: For exercises with few trials, blend the prior with the trial-based score:
  `effective_score = (trial_score * n + prior * k) / (n + k)`
  where `n` is num_trials and `k` is a smoothing constant (e.g., 3.0). When `n = 0`, the score is
  purely the prior. By `n = 6`, the prior's influence is halved. By `n = 15+`, it's negligible.
- Use the `effective_score` (instead of raw 0.0) for mastery window assignment in
  `CandidateFilter`.

**Concrete impact:**
- Student has mastered prerequisite lessons at 4.5+. A new lesson's exercises get a prior of ~3.5
  instead of 0.0. They enter the "current" window instead of "new", getting scheduled for targeted
  practice rather than exploratory exposure.
- Student has scored 5.0 on 9 of 10 exercises in a lesson. The 10th untried exercise gets a sibling
  prior of ~5.0. The system essentially knows this exercise is probably mastered and can deprioritize
  it. If it turns out to be hard, one failed trial will quickly override the prior.
- Exercise in a lesson with no related signals: prior is `None`, behavior unchanged from current
  system.

**Why it's big.** This eliminates the cold start problem that plagues all item-based spaced
repetition systems. Anki treats every new card as equally unknown. Trane has a dependency graph that
encodes rich structural information about how skills relate -- this improvement actually uses that
information for new exercises instead of waiting for trial data.

**Files to modify:**
- `src/scheduler/unit_scorer.rs` -- add
  `fn compute_prior(exercise_id, unit_graph, practice_stats) -> Option<f32>` and a blending
  function. Modify the exercise scoring path to blend prior with trial-based score when `num_trials`
  is low.
- `src/data.rs` -- add `Candidate::prior_score: Option<f32>` for debugging/introspection.
- `src/scheduler.rs` -- pass the prior computation into candidate generation.

---

## 2. Diagnostic Prerequisite Scheduling for Stagnation

**The problem.** Trane detects stagnation (velocity < 0.2 with score < 4.0) and boosts the stagnant
exercise's selection weight by +2000. But boosting selection of an exercise the student is stuck on
is like giving more attempts at a locked door. If the student is stuck at score 2.5 on exercise X
after 8 trials, the bottleneck is likely not exercise X itself but a weak prerequisite that X depends
on.

The current system has no mechanism to diagnose *why* a student is stuck and respond structurally.
Reward propagation sends negative signals upward through the encompassing graph, but this is a blunt
tool: it penalizes all encompassing ancestors equally and doesn't identify the specific weak link.

**What's different.** When stagnation is detected on an exercise, traverse the prerequisite graph to
find the weakest prerequisite (the one with the lowest score among dependencies). Then boost *that*
prerequisite's exercises rather than (or in addition to) the stagnant exercise itself. This mimics
what a good teacher does: "you're stuck on chord voicings -- let's go back and solidify your
interval recognition."

**Algorithm:**
- Add a `DiagnosticScheduler` module that maintains a
  `stagnation_diagnoses: UstrMap<Ustr>` mapping stagnant exercise -> diagnosed weak prerequisite
  lesson.
- **Detection** (in `score_exercise`): When an exercise is scored, check if it meets stagnation
  criteria (velocity < threshold, score < 4.0, num_trials >= 5). If so, trigger diagnosis.
- **Diagnosis**: For the stagnant exercise's lesson, walk the dependency graph backward
  (dependencies, not dependents). For each prerequisite lesson, check its score. The prerequisite
  lesson with the lowest score below passing threshold is the diagnosed weak link. If all
  prerequisites are above passing, check their prerequisites recursively (up to a configurable
  depth, e.g., 3 hops). If no weak link is found, the exercise is intrinsically hard -- keep the
  existing behavior.
- **Treatment**: Store the diagnosis. In `candidate_weight`, exercises from the diagnosed weak
  prerequisite lesson get a large bonus (similar to `DEAD_WEIGHT_FACTOR`). This funnels the
  student's practice toward the root cause.
- **Resolution**: When the diagnosed prerequisite lesson's score rises above passing, clear the
  diagnosis and let the original stagnant exercise be reattempted. The system then naturally
  schedules it because it's still low-scoring.

**Concrete example:**
- Student is stuck on "Jazz Chord Voicings" lesson (score 2.5, velocity 0.05, 8 trials).
- Diagnosis traverses dependencies: "Interval Recognition" (score 4.5, fine), "Triad Inversions"
  (score 2.8, below passing).
- Diagnosis: weak prerequisite = "Triad Inversions".
- Effect: exercises from "Triad Inversions" get a large weight bonus. Student practices inversions,
  gets them to 3.5+. Diagnosis clears. Chord Voicings exercises reappear, and now the student can
  make progress because the foundation is solid.

**Why it's big.** This adds a fundamentally new capability: causal reasoning about learning
difficulties. The current system treats every exercise independently and can only respond to
scores -- it can't reason about *why* a score is low. Diagnostic scheduling uses the graph structure
to identify and fix root causes, which is the most valuable thing a hierarchical dependency graph can
do that flat spaced repetition systems cannot.

**Files to modify:**
- New file: `src/scheduler/diagnostic.rs` -- diagnosis logic: traverse prerequisites, find weakest
  link, store diagnoses, clear on resolution.
- `src/scheduler/data.rs` -- add `diagnostics: Arc<RwLock<DiagnosticScheduler>>` to
  `SchedulerData`.
- `src/scheduler.rs` -- trigger diagnosis in `score_exercise` when stagnation criteria are met;
  populate a `diagnostic_boost: bool` field on relevant `Candidate`s.
- `src/scheduler/filter.rs` -- add weight bonus for `diagnostic_boost` candidates.
- `src/data.rs` -- add `Candidate::diagnostic_boost` field.
