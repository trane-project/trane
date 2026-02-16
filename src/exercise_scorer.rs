//! Scoring primitives for exercise mastery estimation.
//!
//! This module defines the scorer interface and implementations used to derive a normalized mastery
//! score from historical `ExerciseTrial` data. Implementations are deterministic and stateless:
//! scoring is a pure function of the supplied review history and exercise type.
//!
//! The returned score (0.0 to 5.0) is a compact signal used by the scheduler to select and filter
//! exercises rather than a direct review schedule.

use anyhow::{Result, anyhow};
use chrono::Utc;

use crate::data::{ExerciseTrial, ExerciseType};

/// A trait exposing a function to score an exercise based on the results of previous trials.
pub trait ExerciseScorer {
    /// Returns a score (between 0.0 and 5.0) for the exercise based on the results and timestamps
    /// of previous trials. The trials are assumed to be sorted in descending order by timestamp.
    fn score(&self, exercise_type: ExerciseType, previous_trials: &[ExerciseTrial]) -> Result<f32>;
}

/// The target retrievability at `t = stability` used to calibrate the forgetting-curve factor.
///
/// For each decay exponent `d`, the factor is derived so that:
/// `R(t = S, S) = TARGET_RETRIEVABILITY_AT_STABILITY`.
const TARGET_RETRIEVABILITY_AT_STABILITY: f32 = 0.9;

/// The decay exponent used in the power-law forgetting curve for declarative exercises (e.g. memory
/// recall). The value is taken from the FSRS-4.5 implementation.
const DECLARATIVE_CURVE_DECAY: f32 = -0.5;

/// The decay exponent used in the power-law forgetting curve for procedural exercises (e.g. playing
/// a piece of music). The value is higher than for declarative exercises, reflecting the slower
/// decay of procedural memory.
const PROCEDURAL_CURVE_DECAY: f32 = -0.3;

/// The minimum stability value in days. This prevents division by zero and ensures that exercises
/// with very few trials still have a reasonable stability estimate.
const MIN_STABILITY: f32 = 0.5;

/// The maximum stability value in days. This caps the stability estimate to prevent excessively
/// long intervals for well-learned material.
const MAX_STABILITY: f32 = 365.0;

/// The default stability for exercises with no review history.
const DEFAULT_STABILITY: f32 = 1.0;

/// The minimum difficulty value. This represents the easiest exercises.
const MIN_DIFFICULTY: f32 = 1.0;

/// The maximum difficulty value. This represents the hardest exercises.
const MAX_DIFFICULTY: f32 = 10.0;

/// The base difficulty value. This represents the default difficulty for exercises with no review
/// history.
const BASE_DIFFICULTY: f32 = 5.0;

/// The weight of the overall difficulty when calculating the final difficulty estimate.
const OVERALL_DIFFICULTY_WEIGHT: f32 = 0.7;

/// The weight of recent performance when calculating the final difficulty estimate.
const RECENT_PERFORMANCE_WEIGHT: f32 = 0.3;

/// The divisor used to scale the difficulty effect on the forgetting curve. A higher value means
/// difficulty has less impact on the decay rate. With a value of 60, the maximum difficulty (10.0)
/// increases the effective decay rate by approximately 15%.
const DIFFICULTY_FACTOR: f32 = 60.0;

/// The per-trial difficulty adjustment scale. Good grades reduce difficulty, poor grades increase
/// it.
const DIFFICULTY_GRADE_ADJUSTMENT_SCALE: f32 = 0.4;

/// How much the dynamic difficulty is pulled back toward the base estimate after each review.
const DIFFICULTY_REVERSION_WEIGHT: f32 = 0.2;

/// The baseline score used to calculate the performance factor. Scores above this baseline improve
/// stability and difficulty estimates.
const PERFORMANCE_BASELINE_SCORE: f32 = 3.0;

/// A scaling coefficient applied to the stability update term for each review. The per-review
/// multiplicative change is `1 + GROWTH_RATE * P * E * spacing_gain`. The actual growth for a
/// review depends on the performance factor `P`, the ease term `E`, and any spacing gain, and the
/// resulting stability is clamped to `MIN_STABILITY..MAX_STABILITY`.
const GROWTH_RATE: f32 = 0.5;

/// The minimum grade value used in performance calculations. Corresponds to complete failure. This
/// is the same as the minimum mastery score, just with a different name to shorten formulas.
const GRADE_MIN: f32 = 1.0;

/// The maximum grade value used in performance calculations. Corresponds to perfect performance.
/// This is also the same as the maximum mastery score.
const GRADE_MAX: f32 = 5.0;

/// The offset used in difficulty linear mapping. Represents the minimum difficulty.
const DIFFICULTY_OFFSET: f32 = 1.0;

/// The scale factor used in difficulty linear mapping. Determines the range of difficulty values.
const DIFFICULTY_SCALE: f32 = 9.0;

/// The number of seconds in a day, used for timestamp conversions.
const SECONDS_PER_DAY: f32 = 86400.0;

/// The number of recent trials considered when boosting difficulty estimates.
const RECENT_TRIALS_COUNT: usize = 3;

/// The decay factor for exponential weighting of performance. Latest score weight 1.0, then 0.8,
/// 0.64, etc.
const PERFORMANCE_WEIGHT_DECAY: f32 = 0.8;

/// The range of grade values used in performance calculations.
const GRADE_RANGE: f32 = GRADE_MAX - GRADE_MIN;

/// The numerator offset used in the ease factor calculation.
const EASE_NUMERATOR_OFFSET: f32 = 11.0;

/// The denominator used in the ease factor calculation.
const EASE_DENOMINATOR: f32 = 5.0;

/// The weight of the interval-aware spacing effect during successful reviews. Larger values
/// increase stability growth when pre-review retrievability is low.
const SPACING_EFFECT_WEIGHT: f32 = 0.5;

/// The exponent applied to stability when computing diminishing returns for repeated successful
/// reviews. Larger values increase saturation strength at high stability.
const STABILITY_DAMPING_EXP: f32 = 0.2;

/// The minimum stability-loss fraction for a lapse.
const MIN_LAPSE_DROP: f32 = 0.0;

/// The maximum stability-loss fraction for a lapse.
const MAX_LAPSE_DROP: f32 = 0.85;

/// The baseline stability-loss fraction for a lapse.
const LAPSE_BASE_DROP: f32 = 0.30;

/// The influence of difficulty on lapse penalties.
const LAPSE_DIFFICULTY_WEIGHT: f32 = 0.25;

/// The influence of how surprising the lapse was (higher pre-review retrievability means larger
/// penalties).
const LAPSE_RETRIEVABILITY_WEIGHT: f32 = 0.30;

/// A scorer that uses a power-law forgetting curve to compute the score of an exercise, using
/// review-history-based estimation of stability and difficulty. This models memory retention more
/// accurately than exponential decay by accounting for the "fat tail" of long-term memory.
///
/// This implementation is inspired by FSRS (Free Spaced Repetition Scheduler) but simplified for
/// Trane's stateless architecture. Instead of maintaining separate state for each exercise, it
/// chains stability updates through the review history chronologically (oldest to newest).
/// Stability evolves with each review using: S' = S × (1 + GROWTH_RATE × P × E × spacing_gain),
/// where P is performance factor, E is ease factor, and spacing_gain increases successful growth
/// after longer review intervals. Final score is retrievability at current time, scaled to 0-5.
///
/// Algorithm:
///
/// 1. Estimate a base difficulty from all trials (failure rate-based).
/// 2. Chain stability through reviews chronologically (oldest to newest), updating difficulty
///    after each review based on outcome.
/// 3. Apply interval-aware spacing during stability updates (successful recalls after longer
///    intervals boost stability more).
/// 4. Damp stability gains for already-stable memories to model explicit saturation.
/// 5. Compute retrievability from last review to now using power-law decay.
/// 6. Adjust retrievability by difficulty for harder exercises.
/// 7. Apply performance factor from an exponentially weighted average of all reviews (recent
///    performance matters most).
/// 8. Scale to final 0-5 score.
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
    #[inline]
    fn estimate_difficulty(previous_trials: &[ExerciseTrial]) -> f32 {
        // Assign the base probability to exercises with no history.
        if previous_trials.is_empty() {
            return BASE_DIFFICULTY;
        }

        // Count scores below the baseline as failures.
        let failures = previous_trials
            .iter()
            .filter(|t| t.score < PERFORMANCE_BASELINE_SCORE)
            .count() as f32;
        let failure_rate = failures / previous_trials.len() as f32;

        // Linearly map failure rate (0.0-1.0) to difficulty (1.0-10.0).
        // - 0% failures -> difficulty 1 (easy)
        // - 50% failures -> difficulty 5.5 (medium)
        // - 100% failures -> difficulty 10 (hard)
        let difficulty = DIFFICULTY_OFFSET + failure_rate * DIFFICULTY_SCALE;

        // Boost difficulty if recent trials are failing.
        if previous_trials.len() >= RECENT_TRIALS_COUNT {
            let recent_failures = previous_trials
                .iter()
                .take(RECENT_TRIALS_COUNT)
                .filter(|t| t.score < PERFORMANCE_BASELINE_SCORE)
                .count() as f32;
            let recent_failure_rate = recent_failures / RECENT_TRIALS_COUNT as f32;
            return (difficulty * OVERALL_DIFFICULTY_WEIGHT
                + (DIFFICULTY_OFFSET + recent_failure_rate * DIFFICULTY_SCALE)
                    * RECENT_PERFORMANCE_WEIGHT)
                .clamp(MIN_DIFFICULTY, MAX_DIFFICULTY);
        }
        difficulty.clamp(MIN_DIFFICULTY, MAX_DIFFICULTY)
    }

    /// Computes the exponentially weighted average performance from all trials.
    #[inline]
    fn compute_weighted_avg(previous_trials: &[ExerciseTrial]) -> f32 {
        if previous_trials.is_empty() {
            return 0.0;
        }
        let mut weight = 1.0;
        let mut sum_weighted = 0.0;
        let mut sum_weights = 0.0;
        for trial in previous_trials {
            sum_weighted += trial.score * weight;
            sum_weights += weight;
            weight *= PERFORMANCE_WEIGHT_DECAY;
        }
        sum_weighted / sum_weights
    }

    /// Returns the forgetting-curve decay exponent for the given exercise type.
    #[inline]
    fn get_curve_decay(exercise_type: &ExerciseType) -> f32 {
        match exercise_type {
            ExerciseType::Declarative => DECLARATIVE_CURVE_DECAY,
            ExerciseType::Procedural => PROCEDURAL_CURVE_DECAY,
        }
    }

    /// Returns the forgetting-curve factor derived from the decay exponent for this exercise type.
    ///
    /// Using `R(t, S) = (1 + factor * t / S)^(-decay_abs)`, this computes:
    /// `factor = TARGET_RETRIEVABILITY_AT_STABILITY^(-1 / decay_abs) - 1`.
    ///
    /// This calibration ensures `R(t = S, S) = TARGET_RETRIEVABILITY_AT_STABILITY` for each
    /// exercise type while still preserving shape differences through type-specific decay exponents.
    #[inline]
    fn get_curve_factor(exercise_type: &ExerciseType) -> f32 {
        let decay_abs = Self::get_curve_decay(exercise_type).abs().max(f32::EPSILON);
        TARGET_RETRIEVABILITY_AT_STABILITY.powf(-1.0 / decay_abs) - 1.0
    }

    /// Computes pre-review retrievability used by the interval-aware spacing effect. It uses the
    /// same decay exponent as final retrievability for the given exercise type.
    #[inline]
    fn compute_spacing_retrievability(
        exercise_type: &ExerciseType,
        days_since_previous_review: f32,
        stability: f32,
    ) -> f32 {
        let decay = Self::get_curve_decay(exercise_type);
        let factor = Self::get_curve_factor(exercise_type);
        (1.0 + factor * days_since_previous_review / stability)
            .powf(decay)
            .clamp(0.0, 1.0)
    }

    /// Computes the spacing gain multiplier for a review. Successful recalls (`performance_factor >
    /// 0`) receive additional growth after longer intervals. Non-successful reviews return a
    /// neutral multiplier so lapse handling is handled separately.
    #[inline]
    fn compute_spacing_gain(
        exercise_type: &ExerciseType,
        days_since_previous_review: f32,
        stability: f32,
        performance_factor: f32,
    ) -> f32 {
        if performance_factor <= 0.0 {
            return 1.0;
        }

        let pre_review_retrievability = Self::compute_spacing_retrievability(
            exercise_type,
            days_since_previous_review,
            stability,
        );
        (1.0 + SPACING_EFFECT_WEIGHT * (1.0 - pre_review_retrievability))
            .clamp(1.0, 1.0 + SPACING_EFFECT_WEIGHT)
    }

    /// Returns whether this trial is considered a lapse for state updates.
    #[inline]
    fn is_lapse(trial_score: f32) -> bool {
        trial_score < PERFORMANCE_BASELINE_SCORE
    }

    /// Updates difficulty after a review using dynamic trend and mean reversion.
    ///
    /// Good grades reduce difficulty, poor grades increase it. The result is then pulled back
    /// toward the base estimate to prevent drift.
    #[inline]
    fn update_difficulty(difficulty: f32, base_difficulty: f32, trial_score: f32) -> f32 {
        let grade_delta = (PERFORMANCE_BASELINE_SCORE - trial_score) / GRADE_RANGE
            * DIFFICULTY_GRADE_ADJUSTMENT_SCALE;
        let adjusted_difficulty = (difficulty + grade_delta).clamp(MIN_DIFFICULTY, MAX_DIFFICULTY);

        (DIFFICULTY_REVERSION_WEIGHT * base_difficulty
            + (1.0 - DIFFICULTY_REVERSION_WEIGHT) * adjusted_difficulty)
            .clamp(MIN_DIFFICULTY, MAX_DIFFICULTY)
    }

    /// Computes how much stability should be reduced on a lapse.
    ///
    /// Returns a fractional reduction in current stability. Higher difficulty and more surprising
    /// lapses produce larger reductions.
    #[inline]
    fn compute_lapse_drop(difficulty: f32, pre_review_retrievability: f32) -> f32 {
        let difficulty_adjust =
            ((difficulty - MIN_DIFFICULTY) / (MAX_DIFFICULTY - MIN_DIFFICULTY)).clamp(0.0, 1.0);
        let surprise_factor = pre_review_retrievability.clamp(0.0, 1.0);
        (LAPSE_BASE_DROP
            + LAPSE_DIFFICULTY_WEIGHT * difficulty_adjust
            + LAPSE_RETRIEVABILITY_WEIGHT * surprise_factor)
            .clamp(MIN_LAPSE_DROP, MAX_LAPSE_DROP)
    }

    /// Starts with DEFAULT_STABILITY and evolves through reviews from oldest to newest, while
    /// updating dynamic difficulty after each trial with mean reversion.
    ///
    /// For each review:
    /// - Compute elapsed days since the previous review in the chain.
    /// - Estimate pre-review retrievability from elapsed time and current stability.
    /// - Use the same type-specific forgetting curve decay as the final retrievability.
    /// - Apply an interval-aware spacing gain to successful reviews.
    /// - Apply a separate lapse reduction for recalls below the baseline threshold.
    /// - Update stability via the success branch: `S' = S × (1 + GROWTH_RATE × P × E × spacing_gain
    ///   × S^(-k))`.
    /// - Or the lapse branch: `S' = S * (1 - lapse_drop)`, where `lapse_drop` grows with difficulty
    ///   and surprise.
    /// - Update difficulty to the new post-review state and continue to the next trial.
    ///
    /// Here P = (grade - GRADE_MIN) / GRADE_RANGE - 0.5 (performance, from -0.5 for fail to 0.5 for
    /// perfect), and E = (EASE_NUMERATOR_OFFSET - difficulty) / EASE_DENOMINATOR (ease).
    #[inline]
    fn compute_stability_and_difficulty(
        exercise_type: &ExerciseType,
        previous_trials: &[ExerciseTrial],
        base_difficulty: f32,
    ) -> (f32, f32) {
        let mut stability = DEFAULT_STABILITY;
        let mut difficulty = base_difficulty;
        let mut previous_timestamp = None;
        for trial in previous_trials.iter().rev() {
            let days_since_previous_review = previous_timestamp.map_or(0.0, |timestamp| {
                ((trial.timestamp - timestamp) as f32 / SECONDS_PER_DAY).max(0.0)
            });
            let p = (trial.score - GRADE_MIN) / GRADE_RANGE - 0.5;
            let e = (EASE_NUMERATOR_OFFSET - difficulty) / EASE_DENOMINATOR;
            let pre_review_retrievability = Self::compute_spacing_retrievability(
                exercise_type,
                days_since_previous_review,
                stability,
            );

            if Self::is_lapse(trial.score) {
                let lapse_drop = Self::compute_lapse_drop(difficulty, pre_review_retrievability);
                stability = (stability * (1.0 - lapse_drop)).clamp(MIN_STABILITY, MAX_STABILITY);
            } else {
                let spacing_gain = Self::compute_spacing_gain(
                    exercise_type,
                    days_since_previous_review,
                    stability,
                    p,
                );
                let stability_damping = Self::compute_stability_damping(stability);
                let growth_term = GROWTH_RATE * p * e * spacing_gain * stability_damping;
                stability = (stability * (1.0 + growth_term)).clamp(MIN_STABILITY, MAX_STABILITY);
            }
            difficulty = Self::update_difficulty(difficulty, base_difficulty, trial.score);
            previous_timestamp = Some(trial.timestamp);
        }
        (stability, difficulty)
    }

    /// Returns a damping factor for stability growth.
    ///
    /// As stability grows, gains should saturate, so this term decreases with S.
    #[inline]
    fn compute_stability_damping(stability: f32) -> f32 {
        stability.max(MIN_STABILITY).powf(-STABILITY_DAMPING_EXP)
    }

    /// Computes retrievability using power-law forgetting: `R = (1 + factor × t/S)^decay`.
    /// Returns a 0-1 probability of recall.
    ///
    /// The factor is derived from each type's decay exponent so that `R(t = S, S) = 0.9` for both
    /// declarative and procedural exercises. This keeps stability interpretation aligned across
    /// exercise types while retaining type-specific curve shapes.
    #[inline]
    fn compute_retrievability(
        exercise_type: &ExerciseType,
        days_since_last: f32,
        stability: f32,
    ) -> f32 {
        let decay = Self::get_curve_decay(exercise_type);
        let factor = Self::get_curve_factor(exercise_type);
        (1.0 + factor * days_since_last / stability).powf(decay)
    }
}

impl ExerciseScorer for PowerLawScorer {
    fn score(&self, exercise_type: ExerciseType, previous_trials: &[ExerciseTrial]) -> Result<f32> {
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

        let base_difficulty = Self::estimate_difficulty(previous_trials);
        let (stability, final_difficulty) = Self::compute_stability_and_difficulty(
            &exercise_type,
            previous_trials,
            base_difficulty,
        );
        let days_since_last = ((Utc::now().timestamp() - previous_trials[0].timestamp) as f32
            / SECONDS_PER_DAY)
            .max(0.0);
        let retrievability =
            Self::compute_retrievability(&exercise_type, days_since_last, stability);

        // The difficulty exponent adjusts retrievability based on exercise hardness. Harder
        // exercises (higher difficulty) have lower retrievability for the same stability due to
        // increased decay. The formula is exponent = MIN_DIFFICULTY + (difficulty - MIN_DIFFICULTY)
        // / DIFFICULTY_FACTOR.
        let difficulty_exponent =
            MIN_DIFFICULTY + (final_difficulty - MIN_DIFFICULTY) / DIFFICULTY_FACTOR;
        let adjusted_retrievability = retrievability.powf(difficulty_exponent);

        // Compute the weighted average of all the trials and return the final score.
        let weighted_score = Self::compute_weighted_avg(previous_trials);
        Ok((adjusted_retrievability * weighted_score).clamp(0.0, 5.0))
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

        // Recent failures should increase difficulty estimate. Trials are sorted newest first, so
        // put failures at the beginning.
        let recent_failures = vec![
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
        let recent_difficulty = PowerLawScorer::estimate_difficulty(&recent_failures);
        // Should be elevated due to recent failures despite earlier successes.
        assert!(recent_difficulty > 5.0);
    }

    /// Verifies the score for an exercise with no previous trials is 0.0.
    #[test]
    fn no_previous_trials() {
        assert_eq!(0.0, SCORER.score(ExerciseType::Declarative, &[]).unwrap());
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

        let score = SCORER.score(ExerciseType::Declarative, &trials).unwrap();
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
        )?;
        assert!(score >= 0.0 && score <= 5.0);
        assert!(score < 1.0); // Low due to long time elapsed
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
        let stability = PowerLawScorer::compute_stability_and_difficulty(
            &ExerciseType::Declarative,
            &trials,
            difficulty,
        )
        .0;
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

        let short_spacing_stability = PowerLawScorer::compute_stability_and_difficulty(
            &ExerciseType::Declarative,
            &short_spacing_trials,
            difficulty,
        )
        .0;
        let long_spacing_stability = PowerLawScorer::compute_stability_and_difficulty(
            &ExerciseType::Declarative,
            &long_spacing_trials,
            difficulty,
        )
        .0;
        assert!(long_spacing_stability > short_spacing_stability);
    }

    /// Verifies lapses reduce stability more than a baseline success.
    #[test]
    fn stability_lapse_reduces_more_than_hard_success() {
        let difficulty = BASE_DIFFICULTY;
        let success_trials = vec![ExerciseTrial {
            score: 3.0,
            timestamp: generate_timestamp(1),
        }];
        let lapse_trials = vec![ExerciseTrial {
            score: 1.0,
            timestamp: generate_timestamp(1),
        }];

        let success_stability = PowerLawScorer::compute_stability_and_difficulty(
            &ExerciseType::Declarative,
            &success_trials,
            difficulty,
        )
        .0;
        let lapse_stability = PowerLawScorer::compute_stability_and_difficulty(
            &ExerciseType::Declarative,
            &lapse_trials,
            difficulty,
        )
        .0;

        assert!(success_stability > MIN_STABILITY);
        assert!(lapse_stability < success_stability);
        assert!(lapse_stability >= MIN_STABILITY);
    }

    /// Verifies that stronger damping reduces growth at high stability for the same review quality.
    #[test]
    fn stability_growth_saturates_at_high_s() {
        // Compare the same successful-review profile at low and high starting stability.
        let difficulty = BASE_DIFFICULTY;
        let exercise_type = ExerciseType::Declarative;
        let p = (5.0 - GRADE_MIN) / GRADE_RANGE - 0.5;
        let e = (EASE_NUMERATOR_OFFSET - difficulty) / EASE_DENOMINATOR;
        let spacing_gain =
            PowerLawScorer::compute_spacing_gain(&exercise_type, 0.0, MIN_STABILITY, p);
        let base_growth_term = GROWTH_RATE * p * e * spacing_gain;

        let low_stability = MIN_STABILITY;
        let high_stability = 50.0;
        let low_stability_damping = PowerLawScorer::compute_stability_damping(low_stability);
        let high_stability_damping = PowerLawScorer::compute_stability_damping(high_stability);

        let low_effective_growth = base_growth_term * low_stability_damping;
        let high_effective_growth = base_growth_term * high_stability_damping;
        assert!(low_effective_growth > high_effective_growth);
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

        let stability = PowerLawScorer::compute_stability_and_difficulty(
            &ExerciseType::Declarative,
            &lapses,
            difficulty,
        )
        .0;
        assert!(stability >= MIN_STABILITY);
        assert!(stability < DEFAULT_STABILITY);
    }

    /// Verifies long-run saturated success updates do not explode beyond expected bounds.
    #[test]
    fn high_stability_does_not_explode() {
        // Use a fixed successful-review profile for all iterations.
        let exercise_type = ExerciseType::Declarative;
        let difficulty = BASE_DIFFICULTY;
        let p = (5.0 - GRADE_MIN) / GRADE_RANGE - 0.5;
        let e = (EASE_NUMERATOR_OFFSET - difficulty) / EASE_DENOMINATOR;
        let spacing_gain =
            PowerLawScorer::compute_spacing_gain(&exercise_type, 0.0, MIN_STABILITY, p);
        let base_growth_term = GROWTH_RATE * p * e * spacing_gain;

        // Repeatedly apply success updates and track shrinking relative gains.
        let mut stability = MIN_STABILITY;
        let mut previous_relative_gain = f32::INFINITY;
        for _ in 0..25 {
            let stability_damping = PowerLawScorer::compute_stability_damping(stability);
            let effective_growth = base_growth_term * stability_damping;
            let next_stability =
                (stability * (1.0 + effective_growth)).clamp(MIN_STABILITY, MAX_STABILITY);
            let relative_gain = (next_stability - stability) / stability;

            assert!(relative_gain < previous_relative_gain);
            previous_relative_gain = relative_gain;
            stability = next_stability;
        }

        assert!(stability < MAX_STABILITY);
        assert!(stability >= MIN_STABILITY);
    }

    /// Verifies consistent successful trials reduce difficulty by pulling it toward easier recall.
    #[test]
    fn difficulty_trend_improves_with_successes() {
        // Start from a hard baseline and apply repeated good recall.
        let base_difficulty = MAX_DIFFICULTY;
        let trials = vec![
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(2),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(0),
            },
        ];

        let (_stability, adjusted_difficulty) = PowerLawScorer::compute_stability_and_difficulty(
            &ExerciseType::Declarative,
            &trials,
            base_difficulty,
        );
        assert!(adjusted_difficulty < base_difficulty);
        assert!(adjusted_difficulty >= MIN_DIFFICULTY);
    }

    /// Verifies repeated failed trials raise difficulty relative to a low baseline.
    #[test]
    fn difficulty_trend_worsens_with_failures() {
        // Start from an easy baseline and apply repeated misses.
        let base_difficulty = MIN_DIFFICULTY;
        let trials = vec![
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(2),
            },
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(0),
            },
        ];

        let (_stability, adjusted_difficulty) = PowerLawScorer::compute_stability_and_difficulty(
            &ExerciseType::Declarative,
            &trials,
            base_difficulty,
        );
        assert!(adjusted_difficulty > base_difficulty);
        assert!(adjusted_difficulty < base_difficulty + 1.0);
    }

    /// Verifies mean reversion keeps difficulty bounded during long repeated failures.
    #[test]
    fn difficulty_mean_reversion_prevents_runaway() {
        // Repeated failures should increase difficulty, but not grow without bound.
        let base_difficulty = 2.0;
        let failures = (0..20)
            .map(|days| ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(days),
            })
            .collect::<Vec<_>>();

        let (_stability, adjusted_difficulty) = PowerLawScorer::compute_stability_and_difficulty(
            &ExerciseType::Declarative,
            &failures,
            base_difficulty,
        );
        assert!(adjusted_difficulty > base_difficulty);
        assert!(adjusted_difficulty < base_difficulty + 1.0);
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

    /// Verifies pre-review spacing retrievability decreases as elapsed time grows.
    #[test]
    fn compute_spacing_retrievability() {
        let stability = DEFAULT_STABILITY;
        let recent_declarative = PowerLawScorer::compute_spacing_retrievability(
            &ExerciseType::Declarative,
            0.0,
            stability,
        );
        let old_declarative = PowerLawScorer::compute_spacing_retrievability(
            &ExerciseType::Declarative,
            30.0,
            stability,
        );
        let old_procedural = PowerLawScorer::compute_spacing_retrievability(
            &ExerciseType::Procedural,
            30.0,
            stability,
        );

        assert!((recent_declarative - 1.0).abs() < 1e-6);
        assert!(old_declarative >= 0.0 && old_declarative <= 1.0);
        assert!(old_procedural >= 0.0 && old_procedural <= 1.0);
        assert!(recent_declarative > old_declarative);
        assert!(old_procedural > old_declarative);
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

    /// Verifies that the weighted average is computed correctly.
    #[test]
    fn compute_weighted_avg() {
        // Empty trials should return 0.0.
        assert_eq!(PowerLawScorer::compute_weighted_avg(&[]), 0.0);

        // Single trial with score 5.0 returns 5.0.
        let single_trial = vec![ExerciseTrial {
            score: 5.0,
            timestamp: generate_timestamp(0),
        }];
        assert!((PowerLawScorer::compute_weighted_avg(&single_trial) - 5.0).abs() < 1e-6);

        // Multiple trials: [5.0, 4.0, 3.0] should be approx 4.147
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
        assert!((weighted - 4.147).abs() < 0.001);
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
        let score = SCORER.score(ExerciseType::Declarative, &trials)?;
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
        let score = SCORER.score(ExerciseType::Declarative, &trials)?;
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
        )?;
        assert!(score > 1.0 && score < 1.5);
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
        let score = SCORER.score(ExerciseType::Declarative, &trials)?;
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
        let score = SCORER.score(ExerciseType::Declarative, &trials)?;
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
                timestamp: generate_timestamp(40),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(60),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(67),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(99),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(140),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(167),
            },
        ];
        let score = SCORER.score(ExerciseType::Procedural, &trials)?;
        assert!(score > 3.0 && score < 5.0);
        Ok(())
    }
}
