# Improvement Plan

## 1. Residual Tracking

### Goal

Track the difference between the scorer's predicted mastery and the student's actual
submitted score. This serves two purposes: evaluating scorer accuracy and driving per-exercise
adaptation.

### Design

When an exercise is about to be presented, the scorer computes a score from the review
history. When the student submits their mastery score, the residual is:

```
residual = actual_score - predicted_score
```

- Positive residual: scorer underestimates the student (stability too low or difficulty too
  high).
- Negative residual: scorer overestimates the student (stability too high or difficulty too
  low).

### Storage

Store `(exercise_id, predicted_score, actual_score, timestamp)` tuples. Options:

- New SQLite table alongside practice_stats.db.
- Additional columns in the existing practice_stats schema.

A new table is cleaner since it avoids migrating existing data.

### Online Adaptation

Use an exponentially-weighted mean residual per exercise to compute a difficulty bias:

```
adjusted_difficulty = base_difficulty - residual_bias * RESIDUAL_SCALE
```

where `residual_bias` is the weighted mean of recent residuals for that exercise. This
feeds into the existing stability computation: lower difficulty means higher ease factor,
faster stability growth, and higher predicted scores next time.

### Offline Evaluation

The stored `(predicted, actual)` pairs enable computing:

- Mean absolute error and RMSE of predictions.
- Systematic bias (are predictions consistently too high or too low?).
- Per-exercise-type breakdowns (declarative vs. procedural).
- Whether the scorer improves over time as it sees more data.

### Implementation Steps

1. Add a new SQLite table for residual records.
2. At exercise presentation time, compute and cache the predicted score.
3. At score submission time, store the residual.
4. Compute per-exercise weighted mean residual.
5. Feed the residual bias into `estimate_difficulty` as a correction term.
6. Add evaluation metrics (MAE, RMSE, bias) that can be computed from the stored data.

## 2. Parameter Optimization

### Problem

The scorer uses fixed global constants (decay exponents, stability coefficient, spacing
weight, etc.). Finding optimal values requires running the benchmark and minimizing
days-to-mastery across student profiles. Each evaluation is expensive (a full simulation),
the function is not differentiable, and there are 5-10 parameters.

### Algorithms

#### Nelder-Mead

A derivative-free optimization algorithm that maintains a simplex (N+1 vertices in
N-dimensional space). At each step it reflects the worst vertex through the centroid of
the remaining vertices, then expands, contracts, or shrinks based on the result.

- Evaluations needed: 50-200 for 5 parameters, 150-500 for 10.
- Strengths: simple to implement (~50 lines of logic), deterministic "next point" logic,
  no gradients needed.
- Weaknesses: can converge to local minima, degrades above ~15 parameters, no native
  bound handling (use parameter transforms instead).

#### CMA-ES (Covariance Matrix Adaptation Evolution Strategy)

Maintains a multivariate normal distribution over parameter space. Each iteration samples
a population, evaluates them, and updates the distribution's mean, covariance matrix, and
step size based on the best candidates. The covariance matrix learns correlations between
parameters.

- Evaluations needed: 200-500 for 5 parameters, 500-2000 for 10.
- Strengths: handles non-convex and multimodal landscapes, learns parameter correlations,
  very robust.
- Weaknesses: higher evaluation cost than Nelder-Mead for simple landscapes, moderately
  complex to implement (covariance update, step-size control).

#### Bayesian Optimization

Maintains a Gaussian Process surrogate model fitted to all evaluations so far. An
acquisition function (e.g. Expected Improvement) balances exploration and exploitation
to pick the next point. Specifically designed for expensive evaluations.

- Evaluations needed: 30-100 for 5 parameters, 80-250 for 10.
- Strengths: most sample-efficient method, models uncertainty explicitly, ideal for
  expensive objective functions.
- Weaknesses: complex to implement (GP fitting, kernel hyperparameters, acquisition
  function optimization), practically requires a library.

#### Powell's Method

Performs sequential 1D line searches along a set of directions, updating the direction
set after each cycle to incorporate curvature information.

- Evaluations needed: similar to Nelder-Mead.
- Strengths: often faster convergence than Nelder-Mead for smooth functions.
- Weaknesses: requires implementing a line search subroutine, slightly more complex.

#### Differential Evolution

Population-based: creates new candidates by combining difference vectors from random
population members.

- Evaluations needed: 500-2000 for 5 parameters, 2000-10000 for 10.
- Strengths: simple to implement, robust for multimodal problems.
- Weaknesses: too many evaluations for tight budgets.

#### Random / Latin Hypercube Search

Random sampling, optionally with stratified coverage (Latin Hypercube). No learning
between evaluations.

- Useful as an initialization phase for directed methods, not as a standalone approach
  for 5+ parameters.

### Agent-Driven Optimization

These algorithms are well-suited for an AI agent to run autonomously because the agent
can implement the logic, run the benchmark, observe results, and decide next steps without
human intervention.

#### Recommended approach: phased strategy

**Phase 1 — Exploration (20-50 evaluations):** Latin Hypercube Sampling across parameter
bounds to get broad coverage. Identifies promising regions and which parameters matter most.

**Phase 2 — Directed optimization (remaining budget):** Nelder-Mead starting from the best
point found in Phase 1. If budget permits (>300 total), run 2-3 Nelder-Mead instances from
different starting points to mitigate local minima.

**Phase 3 — Local refinement (last 10-20% of budget):** Small perturbation study around the
best point to confirm it is a genuine minimum and assess parameter sensitivity.

#### Bound handling

Transform bounded parameters to unbounded space before optimizing:

- Parameters in (0, inf): log transform.
- Parameters in (0, 1): logit transform.
- Parameters in (a, b): logit of (x - a) / (b - a).

The agent optimizes in transformed space and maps back for evaluation. This is cleaner
than clamping or penalty functions.

### Evaluation Budget Summary

| Approach              | Budget (5 params) | Budget (10 params) | Implementability |
|-----------------------|--------------------|--------------------|------------------|
| Nelder-Mead           | 50-200             | 150-500            | Trivial          |
| Multi-start N-M       | 150-400            | 300-500+           | Trivial          |
| Powell's method       | 50-200             | 150-500            | Moderate         |
| CMA-ES                | 200-500            | 500-2000           | Moderate-Hard    |
| Bayesian Optimization | 30-100             | 80-250             | Hard (library)   |
| Differential Evolution| 500-2000           | 2000-10000         | Easy but costly  |
| Random / LHS          | 500+ (poor)        | 1000+ (poor)       | Trivial          |

### Parameters to Optimize

Candidates from `exercise_scorer.rs`:

- `DECLARATIVE_CURVE_DECAY` (-0.5)
- `PROCEDURAL_CURVE_DECAY` (-0.3)
- `STABILITY_COEFFICIENT` (2.1)
- `DIFFICULTY_GRADE_ADJUSTMENT_SCALE` (0.6)
- `DIFFICULTY_REVERSION_WEIGHT` (0.1)
- `PERFORMANCE_WEIGHT_DECAY` (0.98)
- `SPACING_EFFECT_WEIGHT` (0.7)

The benchmark's `days_to_mastery` aggregated across all student profiles is the objective
to minimize.
