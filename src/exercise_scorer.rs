//! Contains the logic to score an exercise based on the results and timestamps of previous trials.

use anyhow::{anyhow, Result};
use chrono::Utc;

use crate::data::{ExerciseTrial, ExerciseType};

/// A trait exposing a function to score an exercise based on the results of previous trials.
pub trait ExerciseScorer {
    /// Returns a score (between 0.0 and 5.0) for the exercise based on the results and timestamps
    /// of previous trials. The trials are assumed to be sorted in descending order by timestamp.
    fn score(&self, exercise_type: ExerciseType, previous_trials: &[ExerciseTrial]) -> Result<f32>;
}

/// The factor used in the power-law forgetting curve. With the declarative decay exponent, this
/// value yields roughly 90% retrievability when the time elapsed equals the stability. The value is
/// taken from the FSRS-4.5 implementation.
const FORGETTING_CURVE_FACTOR: f32 = 19.0 / 81.0;

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

/// The baseline score used to calculate the performance factor. Scores above this baseline improve
/// stability and difficulty estimates.
const PERFORMANCE_BASELINE_SCORE: f32 = 3.0;

/// The maximum growth rate for stability per review (50% cap). Limits stability increase to prevent
/// unrealistic growth from perfect performance on hard exercises.
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

/// A scorer that uses a power-law forgetting curve to compute the score of an exercise, using
/// review-history-based estimation of stability and difficulty. This models memory retention more
/// accurately than exponential decay by accounting for the "fat tail" of long-term memory.
///
/// This implementation is inspired by FSRS (Free Spaced Repetition Scheduler) but simplified for
/// Trane's stateless architecture. Instead of maintaining separate state for each exercise, it
/// chains stability updates through the review history chronologically (oldest to newest).
/// Stability evolves with each review using: S' = S × (1 + GROWTH_RATE × P × E), where P is
/// performance factor and E is ease factor. Final score is retrievability at current time, scaled
/// to 0-5.
///
/// Algorithm:
///
/// 1. Estimate difficulty once from all trials (failure rate-based)
/// 2. Chain stability through reviews chronologically
/// 3. Compute retrievability from last review to now using power-law decay
/// 4. Adjust retrievability by difficulty for harder exercises
/// 5. Apply performance factor from an exponentially weighted average of all reviews (recent
///    performance matters most)
/// 6. Scale to final 0-5 score.
///
/// Differences from FSRS:
/// - No additional state storage (stateless like original Trane design)
/// - Simplified formula without request/response vectors or complex parameters
/// - Single retrievability score instead of separate difficulty/stability outputs
/// - Chained computation instead of matrix-based state evolution
/// - Growth rate capped at 50% per review for stability
///
/// Why simplified: Trane uses a graph structure producing mixed batches, not optimal flat lists.
/// Optimizing ExerciseScorer is less critical than in dedicated SRS systems.
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

    /// Starts with DEFAULT_STABILITY, evolves via S' = S * (1 + GROWTH_RATE * P * E) for each
    /// review. P = (grade - GRADE_MIN) / GRADE_RANGE - 0.5 (performance, from -0.5 for fail to 0.5
    /// for perfect), E = (EASE_NUMERATOR_OFFSET - difficulty) / EASE_DENOMINATOR (ease). Processed
    /// oldest to newest.
    #[inline]
    fn compute_stability(previous_trials: &[ExerciseTrial], difficulty: f32) -> f32 {
        let mut stability = DEFAULT_STABILITY;
        for trial in previous_trials.iter().rev() {
            let p = (trial.score - GRADE_MIN) / GRADE_RANGE - 0.5;
            let e = (EASE_NUMERATOR_OFFSET - difficulty) / EASE_DENOMINATOR;
            stability =
                (stability * (1.0 + GROWTH_RATE * p * e)).clamp(MIN_STABILITY, MAX_STABILITY);
        }
        stability
    }

    /// Computes retrievability using power-law forgetting: R = (1 + factor × t/S)^decay. Returns
    /// 0-1 probability of recall. A different decay for declarative and procedural execises
    /// reflects the different forgetting patterns of these memory types.
    #[inline]
    fn compute_retrievability(
        exercise_type: &ExerciseType,
        days_since_last: f32,
        stability: f32,
    ) -> f32 {
        let decay = match exercise_type {
            ExerciseType::Declarative => DECLARATIVE_CURVE_DECAY,
            ExerciseType::Procedural => PROCEDURAL_CURVE_DECAY,
        };
        (1.0 + FORGETTING_CURVE_FACTOR * days_since_last / stability).powf(decay)
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

        let difficulty = Self::estimate_difficulty(previous_trials);
        let stability = Self::compute_stability(previous_trials, difficulty);
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
            MIN_DIFFICULTY + (difficulty - MIN_DIFFICULTY) / DIFFICULTY_FACTOR;
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

    const SECONDS_IN_DAY: i64 = 60 * 60 * 24;
    const SCORER: PowerLawScorer = PowerLawScorer {};

    /// Generates a timestamp equal to the timestamp from `num_days` ago.
    fn generate_timestamp(num_days: i64) -> i64 {
        let now = Utc::now().timestamp();
        now - num_days * SECONDS_IN_DAY
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
        let stability = PowerLawScorer::compute_stability(&trials, difficulty);
        assert!(stability > 0.0 && stability < 2.0); // Reasonable range
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
        assert!(recent_declarative < recent_procedural);

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

    /// Verifies that many old and well-spaced trials with good scores return a score that is stiill
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
