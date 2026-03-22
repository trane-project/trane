//! Scoring primitives for exercise mastery estimation.
//!
//! This module defines the scorer interface and implementations used to derive a normalized mastery
//! score from historical `ExerciseTrial` data. Implementations are deterministic and stateless:
//! scoring is a pure function of the supplied review history and exercise type.
//!
//! The returned score (0.0 to 5.0) is a compact signal used by the scheduler to select and filter
//! exercises rather than a direct review schedule.

use anyhow::{Result, anyhow};

use crate::data::{ExerciseTrial, ExerciseType};

/// A trait exposing a function to score an exercise based on the results of previous trials.
pub trait ExerciseScorer {
    /// Returns a score (between 0.0 and 5.0) for the exercise based on the results and timestamps
    /// of previous trials. The trials are assumed to be sorted in descending order by timestamp.
    fn score(
        &self,
        exercise_type: ExerciseType,
        previous_trials: &[ExerciseTrial],
        now: i64,
    ) -> Result<f32>;

    /// Returns the velocity of learning for exercise with the given trials. The velocity is a
    /// measure of how quickly the score is improving or worsening over trials. A value of None
    /// indicates that there are too few trials to compute a reliable velocity.
    fn velocity(&self, previous_trials: &[ExerciseTrial]) -> Option<f32>;
}

// Adjustable constants: these can be tuned to calibrate the scorer.

/// The decay exponent used in the power-law forgetting curve for declarative exercises (e.g. memory
/// recall). The value is taken from the FSRS-4.5 implementation.
const DECLARATIVE_CURVE_DECAY: f32 = -0.5;

/// The decay exponent used in the power-law forgetting curve for procedural exercises (e.g. playing
/// a piece of music). The value is higher than for declarative exercises, reflecting the slower
/// decay of procedural memory.
const PROCEDURAL_CURVE_DECAY: f32 = -0.3;

/// A scaling coefficient applied to the stability update term for each review. The per-review
/// multiplicative change is `1 + STABILITY_COEFFICIENT * P * E * spacing_gain`. The resulting
/// stability is clamped to `MIN_STABILITY..MAX_STABILITY`.
const STABILITY_COEFFICIENT: f32 = 2.1;

/// The per-trial difficulty adjustment scale. Good grades reduce difficulty, poor grades increase
/// it.
const DIFFICULTY_GRADE_ADJUSTMENT_SCALE: f32 = 0.6;

/// How much the dynamic difficulty is pulled back toward the base estimate after each review.
const DIFFICULTY_REVERSION_WEIGHT: f32 = 0.1;

/// The per-day decay factor for exponential weighting of performance. Latest score weight 1.0,
/// scores one day old are multiplied by it, two days old by its square and so on.
const PERFORMANCE_WEIGHT_DECAY: f32 = 0.98;

/// The weight of the interval-aware spacing effect during successful reviews. Larger values
/// increase stability growth when pre-review retrievability is low.
const SPACING_EFFECT_WEIGHT: f32 = 0.7;

/// The minimum weighted score required to apply the old-good retrievability floor. This floor is
/// applied to exercises with strong historical performance to prevent them from dropping too low
/// after long gaps in practice. In such cases, it is better to allow students to see old exercises
/// and have them fail than to have them stuck with review of very old exercises.
const OLD_GOOD_MIN_SCORE: f32 = 4.0;

/// The minimum number of scores required to apply the old-good retrievability floor.
const OLD_GOOD_MIN_SCORES: usize = 2;

/// The minimum number of elapsed days required to apply the old-good retrievability floor.
const OLD_GOOD_MIN_AGE: f32 = 50.0;

/// The minimum retrievability used for old exercises with strong historical performance.
const OLD_GOOD_FLOOR: f32 = 0.75;

// Basic constants: these should not be tuned as they represent basic properties of the scoring
// model that are not subject to change.

/// The target retrievability at `t = stability` used to calibrate the forgetting-curve factor for
/// procedural and declarative exercises.
const TARGET_RETRIEVABILITY_AT_STABILITY: f32 = 0.9;

/// The minimum stability value in days. This prevents division by zero and ensures that exercises
/// with very few trials still have a reasonable stability estimate.
const MIN_STABILITY: f32 = 0.5;

/// The maximum stability value in days. Trane is designed for the long-life learning of acquiring
/// mastery, so a high stability ceiling of two years allows it to model this case.
const MAX_STABILITY: f32 = 730.0;

/// The default stability for exercises with no review history.
const DEFAULT_STABILITY: f32 = 1.0;

/// The minimum difficulty value. This represents the easiest exercises.
const MIN_DIFFICULTY: f32 = 1.0;

/// The maximum difficulty value. This represents the hardest exercises.
const MAX_DIFFICULTY: f32 = 10.0;

/// The base difficulty value. This represents the default difficulty for exercises with no review
/// history.
const BASE_DIFFICULTY: f32 = 5.0;

/// The numerator offset used in the ease factor calculation. It and the denominator are derived to
/// make them work with the minimum and maximum difficulty values.
const EASE_NUMERATOR_OFFSET: f32 = 11.0;

/// The denominator used in the ease factor calculation.
const EASE_DENOMINATOR: f32 = 5.0;

/// The baseline score used to calculate the performance factor. Scores above this baseline improve
/// stability and difficulty estimates.
const PERFORMANCE_BASELINE_SCORE: f32 = 3.0;

/// The minimum per-trial performance weight, ensuring very old trials never disappear entirely.
const PERFORMANCE_WEIGHT_MIN: f32 = 0.1;

/// The minimum grade value used in performance calculations. Corresponds to complete failure. This
/// is the same as the minimum mastery score, just with a different name to shorten formulas.
const GRADE_MIN: f32 = 1.0;

/// The maximum grade value used in performance calculations. Corresponds to perfect performance.
/// This is also the same as the maximum mastery score.
const GRADE_MAX: f32 = 5.0;

/// The range of grade values used in performance calculations.
const GRADE_RANGE: f32 = GRADE_MAX - GRADE_MIN;

/// The number of seconds in a day, used for timestamp conversions.
const SECONDS_PER_DAY: f32 = 86400.0;

/// A scorer that uses a power-law forgetting curve to compute the score of an exercise, using
/// review-history-based estimation of stability and difficulty. This models memory retention more
/// accurately than exponential decay by accounting for the "fat tail" of long-term memory.
///
/// This implementation is inspired by FSRS (Free Spaced Repetition Scheduler) but simplified for
/// Trane's stateless architecture. Instead of maintaining separate state for each exercise, it
/// chains stability updates through the review history chronologically (oldest to newest).
/// Stability evolves with each review using:
///
/// S' = S × (1 + STABILITY_COEFFICIENT × P × E × spacing_gain),
///
/// where P is performance factor, E is ease factor, and spacing_gain increases successful growth
/// after longer review intervals. Final score multiplies retrievability by the recency-weighted
/// performance score, then clamps the result to 0-5.
///
/// Algorithm:
///
/// 1. Estimate a base difficulty from review failure rates.
/// 2. Compute the stability of the exercise by replaying the review history.
/// 3. Compute retrievability from last review to now using power-law decay.
/// 4. Apply a retrievability floor for old exercises with strong historical performance.
/// 5. Multiply retrievability by the recency-weighted average score and clamp to 0-5.
///
/// A simplified implementation without additional stored parameters is preferred for Trane because:
///
/// - Spaced repetition is just one of many strategies used by the expert system.
/// - The main concept behind trane is to map content to a graph of dependencies, not to have a flat
///   list of exercises. Trane has more information than just previous trials.
/// - The score of an exercise is a score meant to reflect mastery of an exercise, not just memory.
/// - The final output is an optimized batch of exercises, not just a list of exercises due for
///   review.
pub struct PowerLawScorer {}

impl PowerLawScorer {
    /// Estimates the difficulty of the exercise based on failure rates. Difficulty ranges from 1.0
    /// (easiest) to 10.0 (hardest).
    fn estimate_difficulty(previous_trials: &[ExerciseTrial]) -> f32 {
        // Assign the base difficulty to exercises with no history.
        if previous_trials.is_empty() {
            return BASE_DIFFICULTY;
        }

        // Count scores below the baseline as failures.
        let failures = previous_trials
            .iter()
            .filter(|t| t.score < PERFORMANCE_BASELINE_SCORE)
            .count() as f32;
        let failure_rate = failures / previous_trials.len() as f32;

        // Linearly map aggregate failure rate (0.0-1.0) to the difficulty range (1.0-10.0).
        let difficulty = 1.0 + failure_rate * 9.0;
        difficulty.clamp(MIN_DIFFICULTY, MAX_DIFFICULTY)
    }

    /// Computes the time-decayed weighted average performance from all trials.
    ///
    /// Weights decay by elapsed days from the most recent trial so irregular practice cadence is
    /// modeled more accurately.
    fn compute_weighted_avg(previous_trials: &[ExerciseTrial]) -> f32 {
        if previous_trials.is_empty() {
            return 0.0;
        }

        // Start from the latest timestamp and compute the weights based on the number of days
        // from it.
        let newest_timestamp = previous_trials[0].timestamp;
        let mut sum_weighted = 0.0;
        let mut sum_weights = 0.0;
        for trial in previous_trials {
            let elapsed_days = ((newest_timestamp.saturating_sub(trial.timestamp)) as f32
                / SECONDS_PER_DAY)
                .max(0.0);
            let weight = PERFORMANCE_WEIGHT_DECAY
                .powf(elapsed_days)
                .max(PERFORMANCE_WEIGHT_MIN);
            sum_weighted += weight * trial.score;
            sum_weights += weight;
        }

        sum_weighted / sum_weights
    }

    /// Returns the forgetting-curve decay exponent for the given exercise type.
    fn get_curve_decay(exercise_type: &ExerciseType) -> f32 {
        match exercise_type {
            ExerciseType::Declarative => DECLARATIVE_CURVE_DECAY,
            ExerciseType::Procedural => PROCEDURAL_CURVE_DECAY,
        }
    }

    /// Returns the forgetting-curve factor for this exercise type.
    ///
    /// The factor is chosen so that retrievability always drops to the target retrievability when
    /// the elapsed time since the last review equals the exercise's stability, regardless of
    /// exercise type.
    fn get_curve_factor(exercise_type: &ExerciseType) -> f32 {
        let decay_abs = Self::get_curve_decay(exercise_type).abs().max(f32::EPSILON);
        TARGET_RETRIEVABILITY_AT_STABILITY.powf(-1.0 / decay_abs) - 1.0
    }

    /// Computes retrievability using power-law forgetting: `R = (1 + factor × t/S)^decay`. Returns
    /// a 0-1 probability of recall.
    ///
    /// The factor is derived from each type's decay exponent so that `R(t = S, S) = 0.9` for both
    /// declarative and procedural exercises. This keeps stability interpretation aligned across
    /// exercise types while retaining type-specific curve shapes.
    fn compute_retrievability(
        exercise_type: &ExerciseType,
        days_since_last: f32,
        stability: f32,
    ) -> f32 {
        let decay = Self::get_curve_decay(exercise_type);
        let factor = Self::get_curve_factor(exercise_type);
        (1.0 + factor * days_since_last / stability)
            .powf(decay)
            .clamp(0.0, 1.0)
    }

    /// Computes the spacing gain multiplier for a review. Successful recalls (`performance_factor >
    /// 0`) receive additional growth after longer intervals. Non-successful reviews receive no
    /// spacing bonus; the negative growth term from the performance factor reduces stability.
    fn compute_spacing_gain(
        exercise_type: &ExerciseType,
        days_since_previous_review: f32,
        stability: f32,
        performance_factor: f32,
    ) -> f32 {
        if performance_factor <= 0.0 {
            return 1.0;
        }

        let pre_review_retrievability =
            Self::compute_retrievability(exercise_type, days_since_previous_review, stability);
        (1.0 + SPACING_EFFECT_WEIGHT * (1.0 - pre_review_retrievability))
            .clamp(1.0, 1.0 + SPACING_EFFECT_WEIGHT)
    }

    /// Updates difficulty after a review using dynamic trend and mean reversion.
    ///
    /// Good grades reduce difficulty, poor grades increase it. The result is then pulled back
    /// toward the base estimate to prevent drift.
    fn update_difficulty(difficulty: f32, base_difficulty: f32, trial_score: f32) -> f32 {
        let grade_delta = (PERFORMANCE_BASELINE_SCORE - trial_score) / GRADE_RANGE
            * DIFFICULTY_GRADE_ADJUSTMENT_SCALE;
        let adjusted_difficulty = (difficulty + grade_delta).clamp(MIN_DIFFICULTY, MAX_DIFFICULTY);

        (DIFFICULTY_REVERSION_WEIGHT * base_difficulty
            + (1.0 - DIFFICULTY_REVERSION_WEIGHT) * adjusted_difficulty)
            .clamp(MIN_DIFFICULTY, MAX_DIFFICULTY)
    }

    /// Applies a single review result to the current stability estimate.
    fn apply_stability_transition(
        exercise_type: &ExerciseType,
        stability: f32,
        difficulty: f32,
        score: f32,
        days_since_previous_review: f32,
    ) -> f32 {
        let p = (score - GRADE_MIN) / GRADE_RANGE - 0.5;
        let e = (EASE_NUMERATOR_OFFSET - difficulty) / EASE_DENOMINATOR;
        let spacing_gain =
            Self::compute_spacing_gain(exercise_type, days_since_previous_review, stability, p);
        let intra_day_damping = days_since_previous_review.min(1.0);
        let growth_term = STABILITY_COEFFICIENT * p * e * spacing_gain * intra_day_damping;
        (stability * (1.0 + growth_term)).clamp(MIN_STABILITY, MAX_STABILITY)
    }

    /// Replays the full review history chronologically to compute the current stability. Difficulty
    /// is updated after each review with mean reversion toward the base estimate.
    fn compute_stability(
        exercise_type: &ExerciseType,
        previous_trials: &[ExerciseTrial],
        base_difficulty: f32,
    ) -> f32 {
        // Seed state for chain replay from the oldest known review.
        let mut stability = DEFAULT_STABILITY;
        let mut difficulty = base_difficulty;
        let mut previous_timestamp = None;

        // Replay each review chronologically so each result sees the updated state.
        for trial in previous_trials.iter().rev() {
            // Skip the first review.
            if previous_timestamp.is_none() {
                previous_timestamp = Some(trial.timestamp);
                continue;
            }

            // Compute interval and review-derived signals from the current state.
            let days_since_previous_review = previous_timestamp.map_or(0.0, |timestamp| {
                ((trial.timestamp.saturating_sub(timestamp)) as f32 / SECONDS_PER_DAY).max(0.0)
            });
            stability = Self::apply_stability_transition(
                exercise_type,
                stability,
                difficulty,
                trial.score,
                days_since_previous_review,
            );

            // Update the difficulty state for the next review in the chain.
            difficulty = Self::update_difficulty(difficulty, BASE_DIFFICULTY, trial.score);
            previous_timestamp = Some(trial.timestamp);
        }
        stability
    }

    /// Applies a retrievability floor for old exercises with strong weighted performance. It is
    /// preferable to show old exercises and have the student fail than to have students stuck with
    /// review of very old exercises after long gaps in their practice.
    fn apply_old_good_retrievability_floor(
        retrievability: f32,
        weighted_score: f32,
        days_since_last: f32,
        num_scores: usize,
    ) -> f32 {
        if num_scores >= OLD_GOOD_MIN_SCORES
            && weighted_score >= OLD_GOOD_MIN_SCORE
            && days_since_last >= OLD_GOOD_MIN_AGE
        {
            retrievability.max(OLD_GOOD_FLOOR)
        } else {
            retrievability
        }
    }
}

impl ExerciseScorer for PowerLawScorer {
    fn score(
        &self,
        exercise_type: ExerciseType,
        previous_trials: &[ExerciseTrial],
        now: i64,
    ) -> Result<f32> {
        // Guard input ordering and missing-history edge cases.
        if previous_trials.is_empty() {
            return Ok(0.0);
        }
        if previous_trials
            .windows(2)
            .any(|w| w[0].timestamp < w[1].timestamp)
        {
            return Err(anyhow!(
                "Exercise trials not sorted in descending order by timestamp"
            ));
        }

        // Compute the stability of the exercise to project the retrivability from the last review
        // to now.
        let base_difficulty = Self::estimate_difficulty(previous_trials);
        let stability = Self::compute_stability(&exercise_type, previous_trials, base_difficulty);
        let days_since_last =
            ((now.saturating_sub(previous_trials[0].timestamp)) as f32 / SECONDS_PER_DAY).max(0.0);
        let retrievability =
            Self::compute_retrievability(&exercise_type, days_since_last, stability);

        // Compute the weighted score and apply the old-good retrievability floor.
        let weighted_score = Self::compute_weighted_avg(previous_trials);
        let effective_retrievability = Self::apply_old_good_retrievability_floor(
            retrievability,
            weighted_score,
            days_since_last,
            previous_trials.len(),
        );
        Ok((effective_retrievability * weighted_score).clamp(0.0, 5.0))
    }

    fn velocity(&self, previous_trials: &[ExerciseTrial]) -> Option<f32> {
        // Need at least 2 trials for a meaningful slope.
        if previous_trials.len() < 2 {
            return None;
        }

        // Compute the velocity using the ordinary least squares regression method. The oldest trial
        // is used as the reference point and other trials are converted to days from it.
        let oldest_timestamp = previous_trials.last().unwrap().timestamp;
        let n = previous_trials.len() as f32;
        let mut sum_t = 0.0_f32;
        let mut sum_scores = 0.0_f32;
        let mut sum_t_scores = 0.0_f32;
        let mut sum_t_sq = 0.0_f32;
        for trial in previous_trials {
            let t = (trial.timestamp.saturating_sub(oldest_timestamp)) as f32 / SECONDS_PER_DAY;
            sum_t += t;
            sum_scores += trial.score;
            sum_t_scores += t * trial.score;
            sum_t_sq += t * t;
        }
        let denominator = n * sum_t_sq - sum_t * sum_t;
        if denominator.abs() < f32::EPSILON {
            return Some(0.0);
        }
        let slope = (n * sum_t_scores - sum_t * sum_scores) / denominator;
        Some(slope)
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use chrono::Utc;

    use crate::{data::ExerciseTrial, exercise_scorer::*};

    const SCORER: PowerLawScorer = PowerLawScorer {};

    /// Generates a timestamp equal to the timestamp from `num_days` ago.
    fn generate_timestamp(num_days: i64) -> i64 {
        let now = Utc::now().timestamp();
        now - num_days * SECONDS_PER_DAY as i64
    }

    /// Verifies that difficulty is estimated correctly from failure rates.
    #[test]
    fn estimate_difficulty() {
        // Empty trials should return neutral difficulty.
        assert_eq!(PowerLawScorer::estimate_difficulty(&[]), BASE_DIFFICULTY);

        // All successes should yield low difficulty.
        let easy_trials = vec![
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(0),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(2),
            },
        ];
        let easy_difficulty = PowerLawScorer::estimate_difficulty(&easy_trials);
        assert!(easy_difficulty < 3.0);

        // All failures should yield high difficulty.
        let hard_trials = vec![
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(0),
            },
            ExerciseTrial {
                score: 2.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(2),
            },
        ];
        let hard_difficulty = PowerLawScorer::estimate_difficulty(&hard_trials);
        assert!(hard_difficulty > 8.0);

        // Mixed results should yield medium difficulty.
        let medium_trials = vec![
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(0),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 2.0,
                timestamp: generate_timestamp(2),
            },
        ];
        let medium_difficulty = PowerLawScorer::estimate_difficulty(&medium_trials);
        assert!(medium_difficulty >= 4.0 && medium_difficulty < 7.0);

        // A mixed history should yield an intermediate difficulty from aggregate failures.
        let mixed_trials = vec![
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(0),
            },
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 2.0,
                timestamp: generate_timestamp(2),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(3),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(4),
            },
        ];
        let mixed_difficulty = PowerLawScorer::estimate_difficulty(&mixed_trials);
        assert!(mixed_difficulty > 4.0 && mixed_difficulty < 6.0);
    }

    /// Verifies the score for an exercise with no previous trials is 0.0.
    #[test]
    fn no_previous_trials() {
        assert_eq!(
            0.0,
            SCORER
                .score(ExerciseType::Declarative, &[], Utc::now().timestamp())
                .unwrap()
        );
    }

    /// Verifies running the full scoring algorithm on a set of trials produces a reasonable score.
    #[test]
    fn score_trials() {
        let trials = vec![
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(2),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(3),
            },
        ];

        let score = SCORER
            .score(ExerciseType::Declarative, &trials, Utc::now().timestamp())
            .unwrap();
        assert!(score > 0.0 && score <= 5.0);
        assert!(score > 2.0); // Decent due to good recent performance
    }

    /// Verifies scoring an exercise with an invalid timestamp still returns a sane score.
    #[test]
    fn invalid_timestamp() -> Result<()> {
        let score = SCORER.score(
            ExerciseType::Declarative,
            &[ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(1e10 as i64),
            }],
            Utc::now().timestamp(),
        )?;
        assert!(score >= 0.0 && score <= 5.0);
        assert!(score < 1.0); // Low due to long time elapsed
        Ok(())
    }

    /// Verifies extreme timestamp gaps do not overflow elapsed-time calculations.
    #[test]
    fn extreme_timestamp_gap_does_not_overflow() -> Result<()> {
        let score = SCORER.score(
            ExerciseType::Declarative,
            &[
                ExerciseTrial {
                    score: 5.0,
                    timestamp: i64::MAX,
                },
                ExerciseTrial {
                    score: 1.0,
                    timestamp: i64::MIN,
                },
            ],
            Utc::now().timestamp(),
        )?;
        assert!(score >= 0.0 && score <= 5.0);
        Ok(())
    }

    /// Verifies stability computation evolves correctly through reviews.
    #[test]
    fn compute_stability() {
        let difficulty = BASE_DIFFICULTY;
        let trials = vec![
            ExerciseTrial {
                score: 1.0, // Bad: P = -0.5, stability decreases
                timestamp: generate_timestamp(3),
            },
            ExerciseTrial {
                score: 5.0, // Good: P = 0.5, stability increases
                timestamp: generate_timestamp(2),
            },
            ExerciseTrial {
                score: 3.0, // Medium: P = 0.0, stability unchanged
                timestamp: generate_timestamp(1),
            },
        ];
        let stability =
            PowerLawScorer::compute_stability(&ExerciseType::Declarative, &trials, difficulty);
        assert!(stability > 0.0 && stability < 2.0); // Reasonable range
    }

    /// Verifies longer spacing between successful reviews yields higher stability.
    #[test]
    fn compute_stability_spacing_effect() {
        let difficulty = BASE_DIFFICULTY;
        let short_spacing_trials = vec![
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(2),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(3),
            },
        ];
        let long_spacing_trials = vec![
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(10),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(30),
            },
        ];

        let short_spacing_stability = PowerLawScorer::compute_stability(
            &ExerciseType::Declarative,
            &short_spacing_trials,
            difficulty,
        );
        let long_spacing_stability = PowerLawScorer::compute_stability(
            &ExerciseType::Declarative,
            &long_spacing_trials,
            difficulty,
        );
        assert!(long_spacing_stability > short_spacing_stability);
    }

    /// Verifies bad scores reduce stability more than neutral scores.
    #[test]
    fn bad_score_reduces_stability() {
        let difficulty = BASE_DIFFICULTY;
        let success_trials = vec![
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(2),
            },
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(3),
            },
        ];
        let lapse_trials = vec![
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(2),
            },
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(3),
            },
        ];

        let success_stability = PowerLawScorer::compute_stability(
            &ExerciseType::Declarative,
            &success_trials,
            difficulty,
        );
        let lapse_stability = PowerLawScorer::compute_stability(
            &ExerciseType::Declarative,
            &lapse_trials,
            difficulty,
        );
        assert!(success_stability > MIN_STABILITY);
        assert!(lapse_stability < success_stability);
        assert!(lapse_stability >= MIN_STABILITY);
    }

    /// Verifies repeated lapses remain bounded by minimum stability.
    #[test]
    fn multiple_lapses_bounded() {
        let difficulty = BASE_DIFFICULTY;
        let lapses = vec![
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(3),
            },
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(2),
            },
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(1),
            },
        ];

        let stability =
            PowerLawScorer::compute_stability(&ExerciseType::Declarative, &lapses, difficulty);
        assert!(stability >= MIN_STABILITY);
        assert!(stability <= DEFAULT_STABILITY);
    }

    /// Verifies long-run success updates do not explode beyond MAX_STABILITY.
    #[test]
    fn high_stability_does_not_explode() {
        // Use a fixed successful-review profile for all iterations.
        let exercise_type = ExerciseType::Declarative;
        let difficulty = BASE_DIFFICULTY;
        let p = (5.0 - GRADE_MIN) / GRADE_RANGE - 0.5;
        let e = (EASE_NUMERATOR_OFFSET - difficulty) / EASE_DENOMINATOR;
        let spacing_gain =
            PowerLawScorer::compute_spacing_gain(&exercise_type, 0.0, MIN_STABILITY, p);
        let growth_term = STABILITY_COEFFICIENT * p * e * spacing_gain;

        // Repeatedly apply success updates and ensure the clamp keeps stability bounded.
        let mut stability = MIN_STABILITY;
        for _ in 0..25 {
            let next_stability =
                (stability * (1.0 + growth_term)).clamp(MIN_STABILITY, MAX_STABILITY);
            let relative_gain = (next_stability - stability) / stability;

            assert!(relative_gain >= 0.0);
            assert!(relative_gain <= growth_term + f32::EPSILON);
            stability = next_stability;
        }

        assert!(stability <= MAX_STABILITY);
        assert!(stability >= MIN_STABILITY);
    }

    /// Verifies retrievability computation using power-law decay.
    #[test]
    fn compute_retrievability() {
        // Recent review: high retrievability
        let stability = DEFAULT_STABILITY;
        let recent_declarative =
            PowerLawScorer::compute_retrievability(&ExerciseType::Declarative, 0.01, stability);
        let recent_procedural =
            PowerLawScorer::compute_retrievability(&ExerciseType::Procedural, 0.01, stability);
        assert!(recent_declarative > 0.9);
        assert!(recent_declarative > recent_procedural);

        // Old review: moderate retrievability
        let old_declarative =
            PowerLawScorer::compute_retrievability(&ExerciseType::Declarative, 10.0, stability);
        let old_procedural =
            PowerLawScorer::compute_retrievability(&ExerciseType::Procedural, 10.0, stability);
        assert!(old_declarative < 0.6 && old_declarative > 0.4);
        assert!(old_declarative < old_procedural);

        // Very old: low retrievability
        let very_old_declarative =
            PowerLawScorer::compute_retrievability(&ExerciseType::Declarative, 100.0, stability);
        let very_old_procedural =
            PowerLawScorer::compute_retrievability(&ExerciseType::Procedural, 100.0, stability);
        assert!(very_old_declarative < 0.25);
        assert!(very_old_declarative < very_old_procedural);
    }

    /// Verifies that retrievability is calibrated to 90% when elapsed time equals stability.
    #[test]
    fn retrievability_at_stability_is_ninety_percent() {
        let declarative = PowerLawScorer::compute_retrievability(
            &ExerciseType::Declarative,
            DEFAULT_STABILITY,
            DEFAULT_STABILITY,
        );
        let procedural = PowerLawScorer::compute_retrievability(
            &ExerciseType::Procedural,
            DEFAULT_STABILITY,
            DEFAULT_STABILITY,
        );

        assert!((declarative - TARGET_RETRIEVABILITY_AT_STABILITY).abs() < 1e-6);
        assert!((procedural - TARGET_RETRIEVABILITY_AT_STABILITY).abs() < 1e-6);
    }

    /// Verifies spacing gain grows with interval for successful reviews and stays neutral
    /// otherwise.
    #[test]
    fn compute_spacing_gain() {
        let stability = DEFAULT_STABILITY;
        let short_interval_gain =
            PowerLawScorer::compute_spacing_gain(&ExerciseType::Declarative, 0.0, stability, 0.25);
        let long_interval_gain =
            PowerLawScorer::compute_spacing_gain(&ExerciseType::Declarative, 10.0, stability, 0.25);
        let neutral_gain =
            PowerLawScorer::compute_spacing_gain(&ExerciseType::Declarative, 10.0, stability, 0.0);
        let failure_gain =
            PowerLawScorer::compute_spacing_gain(&ExerciseType::Declarative, 10.0, stability, -0.5);

        assert!(short_interval_gain >= 1.0 && short_interval_gain <= 1.0 + SPACING_EFFECT_WEIGHT);
        assert!(long_interval_gain > short_interval_gain);
        assert_eq!(neutral_gain, 1.0);
        assert_eq!(failure_gain, 1.0);
    }

    /// Verifies that intra-day reviews produce less stability change than inter-day reviews.
    #[test]
    fn intra_day_damping() {
        let difficulty = BASE_DIFFICULTY;
        let score = 5.0;
        let half_day = PowerLawScorer::apply_stability_transition(
            &ExerciseType::Declarative,
            DEFAULT_STABILITY,
            difficulty,
            score,
            0.5,
        );
        let one_day = PowerLawScorer::apply_stability_transition(
            &ExerciseType::Declarative,
            DEFAULT_STABILITY,
            difficulty,
            score,
            1.0,
        );
        let two_days = PowerLawScorer::apply_stability_transition(
            &ExerciseType::Declarative,
            DEFAULT_STABILITY,
            difficulty,
            score,
            2.0,
        );

        // All successful reviews should grow stability. Intra-day review should grow stability less
        // than a one-day review.
        assert!(half_day > DEFAULT_STABILITY);
        assert!(one_day > DEFAULT_STABILITY);
        assert!(two_days > DEFAULT_STABILITY);
        assert!(half_day < one_day);
    }

    /// Verifies that the weighted average is computed correctly.
    #[test]
    fn compute_weighted_avg() {
        // Empty trials should return 0.0.
        assert_eq!(PowerLawScorer::compute_weighted_avg(&[]), 0.0);

        // Single trial with score 5.0 returns mean 5.0.
        let single_trial = vec![ExerciseTrial {
            score: 5.0,
            timestamp: generate_timestamp(0),
        }];
        let mean = PowerLawScorer::compute_weighted_avg(&single_trial);
        assert!((mean - 5.0).abs() < 1e-6);

        // Multiple trials: [5.0, 4.0, 3.0] should be approx 4.03 at this decay rate.
        let multi_trials = vec![
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(0),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(2),
            },
        ];
        let weighted = PowerLawScorer::compute_weighted_avg(&multi_trials);
        assert!((weighted - 4.013).abs() < 0.001);

        // Irregular spacing should down-weight distant failures more than dense spacing.
        let dense_low_tail = vec![
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(0),
            },
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(2),
            },
        ];
        let sparse_low_tail = vec![
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(0),
            },
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(30),
            },
        ];
        let dense_weighted = PowerLawScorer::compute_weighted_avg(&dense_low_tail);
        let sparse_weighted = PowerLawScorer::compute_weighted_avg(&sparse_low_tail);
        assert!(sparse_weighted > dense_weighted);

        // Very old history contributes a floor weight and remains somewhat influential.
        let compact = vec![
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(0),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(1),
            },
        ];
        let with_ancient = vec![
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(0),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(365),
            },
        ];
        let compact_weighted = PowerLawScorer::compute_weighted_avg(&compact);
        let ancient_weighted = PowerLawScorer::compute_weighted_avg(&with_ancient);
        assert!(ancient_weighted < compact_weighted);
        assert!(ancient_weighted > 4.0);
    }

    /// Verifies that the old-good retrievability floor is applied only when thresholds are met.
    #[test]
    fn apply_old_good_retrievability_floor() {
        assert_eq!(
            PowerLawScorer::apply_old_good_retrievability_floor(0.2, 4.0, 80.0, 3),
            OLD_GOOD_FLOOR
        );
        assert_eq!(
            PowerLawScorer::apply_old_good_retrievability_floor(0.95, 4.0, 80.0, 3),
            0.95
        );
        assert_eq!(
            PowerLawScorer::apply_old_good_retrievability_floor(0.2, 3.4, 80.0, 3),
            0.2
        );
        assert_eq!(
            PowerLawScorer::apply_old_good_retrievability_floor(0.2, 4.0, 49.0, 3),
            0.2
        );
        assert_eq!(
            PowerLawScorer::apply_old_good_retrievability_floor(0.2, 4.0, 80.0, 1),
            0.2
        );
    }

    /// Verifies that a recent bad performance results in a very low score.
    #[test]
    fn score_bad_recent() -> Result<()> {
        let trials = vec![
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(3),
            },
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(7),
            },
            ExerciseTrial {
                score: 2.0,
                timestamp: generate_timestamp(10),
            },
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(13),
            },
        ];
        let score = SCORER.score(ExerciseType::Declarative, &trials, Utc::now().timestamp())?;
        assert!(score < 2.0);
        Ok(())
    }

    /// Verifies score for mixed performance history.
    #[test]
    fn score_mixed_performance() -> Result<()> {
        let trials = vec![
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(4),
            },
            ExerciseTrial {
                score: 2.0,
                timestamp: generate_timestamp(5),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(6),
            },
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(7),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(10),
            },
            ExerciseTrial {
                score: 2.0,
                timestamp: generate_timestamp(14),
            },
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(18),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(21),
            },
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(25),
            },
        ];
        let score = SCORER.score(ExerciseType::Declarative, &trials, Utc::now().timestamp())?;
        assert!(score > 1.0 && score < 4.0);
        Ok(())
    }

    /// Verifies that trials not sorted in descending order by timestamp return an error.
    #[test]
    fn score_unsorted_trials() {
        let result = SCORER.score(
            ExerciseType::Declarative,
            &[
                ExerciseTrial {
                    score: 3.0,
                    timestamp: generate_timestamp(2),
                },
                ExerciseTrial {
                    score: 4.0,
                    timestamp: generate_timestamp(1),
                },
            ],
            Utc::now().timestamp(),
        );
        assert!(result.is_err());
    }

    /// Verifies that trials with old timestamp result in a low score.
    #[test]
    fn score_old_timestamp() -> Result<()> {
        let score = SCORER.score(
            ExerciseType::Declarative,
            &[ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(100),
            }],
            Utc::now().timestamp(),
        )?;
        assert!(score < 3.0);
        Ok(())
    }

    /// Verifies that the score for multiple good trials is very close to the maximum score.
    #[test]
    fn score_multiple_good() -> Result<()> {
        let trials = vec![
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(0),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(2),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(3),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(4),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(5),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(6),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(7),
            },
        ];
        let score = SCORER.score(ExerciseType::Declarative, &trials, Utc::now().timestamp())?;
        assert!(score > 4.0);
        Ok(())
    }

    /// Verifies that multiple bad trials result in a very low score.
    #[test]
    fn score_multiple_bad() -> Result<()> {
        let trials = vec![
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(0),
            },
            ExerciseTrial {
                score: 2.0,
                timestamp: generate_timestamp(2),
            },
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(4),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(6),
            },
            ExerciseTrial {
                score: 2.0,
                timestamp: generate_timestamp(9),
            },
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(15),
            },
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(16),
            },
            ExerciseTrial {
                score: 2.0,
                timestamp: generate_timestamp(27),
            },
        ];
        let score = SCORER.score(ExerciseType::Declarative, &trials, Utc::now().timestamp())?;
        assert!(score < 2.0);
        Ok(())
    }

    /// Verifies that many old and well-spaced trials with good scores return a score that is still
    /// good due to high stability.
    #[test]
    fn score_old_good_trials() -> Result<()> {
        let trials = vec![
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(200),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(210),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(213),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(248),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(256),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(270),
            },
        ];
        let score = SCORER.score(ExerciseType::Procedural, &trials, Utc::now().timestamp())?;
        assert!(score >= 3.5);
        let score = SCORER.score(ExerciseType::Declarative, &trials, Utc::now().timestamp())?;
        assert!(score >= 3.5);
        Ok(())
    }

    /// Verifies that very old trials of an exercise with good scores return a high score due to
    /// strong stability.
    #[test]
    fn score_very_good_old_trials() -> Result<()> {
        let trials = vec![
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(400),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(410),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(411),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(420),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(430),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(431),
            },
        ];
        let score = SCORER.score(ExerciseType::Procedural, &trials, Utc::now().timestamp())?;
        assert!(score >= 3.5);
        let score = SCORER.score(ExerciseType::Declarative, &trials, Utc::now().timestamp())?;
        assert!(score >= 3.5);
        Ok(())
    }

    /// Verifies that velocity returns None for 0 or 1 trials.
    #[test]
    fn velocity_empty_trials() {
        assert_eq!(SCORER.velocity(&[]), None);

        let trials = vec![ExerciseTrial {
            score: 3.0,
            timestamp: generate_timestamp(0),
        }];
        assert_eq!(SCORER.velocity(&trials), None);
    }

    /// Verifies that improving scores (most recent is highest) produce positive velocity.
    #[test]
    fn velocity_improving_scores() {
        // Most-recent-first: [5.0, 4.0, 3.0, 2.0, 1.0] — scores are getting better over time.
        let trials = vec![
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(0),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(2),
            },
            ExerciseTrial {
                score: 2.0,
                timestamp: generate_timestamp(3),
            },
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(4),
            },
        ];
        let velocity = SCORER.velocity(&trials).unwrap();
        assert!(velocity > 0.0);
    }

    /// Verifies that worsening scores (most recent is lowest) produce negative velocity.
    #[test]
    fn velocity_worsening_scores() {
        // Most-recent-first: [1.0, 2.0, 3.0, 4.0, 5.0] — scores are getting worse over time.
        let trials = vec![
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(0),
            },
            ExerciseTrial {
                score: 2.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(2),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(3),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(4),
            },
        ];
        let velocity = SCORER.velocity(&trials).unwrap();
        assert!(velocity < 0.0);
    }

    /// Verifies that constant scores produce near-zero velocity.
    #[test]
    fn velocity_constant_scores() {
        let trials = vec![
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(0),
            },
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(2),
            },
        ];
        let velocity = SCORER.velocity(&trials).unwrap();
        assert!(velocity.abs() < 1e-6);
    }
}
