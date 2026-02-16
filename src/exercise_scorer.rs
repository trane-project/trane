//! Contains the logic to score an exercise based on the results and timestamps of previous trials.

use anyhow::{anyhow, Result};
use chrono::Utc;

use crate::data::ExerciseTrial;

/// A trait exposing a function to score an exercise based on the results of previous trials.
pub trait ExerciseScorer {
    /// Returns a score (between 0.0 and 5.0) for the exercise based on the results and timestamps
    /// of previous trials. The trials are assumed to be sorted in descending order by timestamp.
    fn score(&self, previous_trials: &[ExerciseTrial]) -> Result<f32>;
}

/// The factor used in the power-law forgetting curve. This value ensures that the retrievability is
/// 90% when the time elapsed equals the stability. The value is taken from the FSRS-4.5
/// implementation.
const FORGETTING_CURVE_FACTOR: f32 = 19.0 / 81.0;

/// The decay exponent used in the power-law forgetting curve. The value is taken from the FSRS-4.5
/// implementation.
const FORGETTING_CURVE_DECAY: f32 = -0.5;

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

/// The maximum growth rate for stability per review (50% cap).
/// Limits stability increase to prevent unrealistic growth from perfect performance on hard exercises.
const GROWTH_RATE: f32 = 0.5;

/// A scorer that uses a power-law forgetting curve to compute the score of an exercise, using
/// simple interval-based estimation of stability and difficulty. This models memory retention more
/// accurately than exponential decay by accounting for the "fat tail" of long-term memory.
///
/// This implementation is inspired by FSRS (Free Spaced Repetition Scheduler) but simplified for
/// Trane's stateless architecture. Instead of maintaining separate state for each exercise, it
/// chains stability updates through the review history chronologically (oldest to newest).
/// Stability evolves with each review using: S' = S × (1 + GROWTH_RATE × P × E), where P is
/// performance factor and E is ease factor. Final score is retrievability at current time,
/// scaled to 0-5.
///
/// Algorithm:
/// 1. Estimate difficulty once from all trials (failure rate-based)
/// 2. Chain stability through reviews chronologically
/// 3. Compute retrievability from last review to now using power-law decay
/// 4. Adjust retrievability by difficulty for harder exercises
/// 5. Apply performance factor from last review (bad performance lowers score)
/// 6. Scale to final 0-5 score
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
        let difficulty = 1.0 + failure_rate * 9.0;

        // Boost difficulty if recent trials are failing.
        if previous_trials.len() >= 3 {
            let recent_failures = previous_trials
                .iter()
                .take(3)
                .filter(|t| t.score < PERFORMANCE_BASELINE_SCORE)
                .count() as f32;
            let recent_failure_rate = recent_failures / 3.0;
            return (difficulty * OVERALL_DIFFICULTY_WEIGHT
                + (1.0 + recent_failure_rate * 9.0) * RECENT_PERFORMANCE_WEIGHT)
                .clamp(MIN_DIFFICULTY, MAX_DIFFICULTY);
        }
        difficulty.clamp(MIN_DIFFICULTY, MAX_DIFFICULTY)
    }
    /// Starts with DEFAULT_STABILITY, evolves via S' = S × (1 + GROWTH_RATE × P × E) for each
    /// review. P = (grade-1)/4 - 0.5 (performance, -0.5 for fail to 0.5 for perfect), E =
    /// (11-difficulty)/5 (ease). Processed oldest to newest.
    #[inline]
    fn compute_stability(previous_trials: &[ExerciseTrial], difficulty: f32) -> f32 {
        let mut stability = DEFAULT_STABILITY;
        for trial in previous_trials.iter().rev() {
            let p = (trial.score - 1.0) / 4.0 - 0.5; // Performance: -0.5 (fail) to 0.5 (perfect)
            let e = (11.0 - difficulty) / 5.0; // Ease: 2.0 (easy) to 0.2 (hard)
            stability =
                (stability * (1.0 + GROWTH_RATE * p * e)).clamp(MIN_STABILITY, MAX_STABILITY);
        }
        stability
    }

    /// Computes retrievability using power-law forgetting: R = (1 + factor × t/S)^decay.
    /// factor=19/81, decay=-0.5 (FSRS constants). Returns 0-1 probability of recall.
    #[inline]
    fn compute_retrievability(days_since_last: f32, stability: f32) -> f32 {
        (1.0 + FORGETTING_CURVE_FACTOR * days_since_last / stability).powf(FORGETTING_CURVE_DECAY)
    }
}

impl ExerciseScorer for PowerLawScorer {
    fn score(&self, previous_trials: &[ExerciseTrial]) -> Result<f32> {
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
        let days_since_last =
            ((Utc::now().timestamp() - previous_trials[0].timestamp) as f32 / 86400.0).max(0.0);
        let retrievability = Self::compute_retrievability(days_since_last, stability);
        let difficulty_exponent = 1.0 + (difficulty - 1.0) / DIFFICULTY_FACTOR;
        let adjusted_retrievability = retrievability.powf(difficulty_exponent);

        // Adjust score by last performance: bad performance lowers the score to prevent advancement.
        let last_p = ((previous_trials[0].score - 1.0) / 4.0 - 0.5).clamp(-0.5, 0.5);
        let performance_factor = (last_p + 0.5).max(0.0);

        Ok((adjusted_retrievability * performance_factor * 5.0).clamp(0.0, 5.0))
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
        assert_eq!(PowerLawScorer::estimate_difficulty(&[]), 5.0);

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
                score: 1.0,
                timestamp: generate_timestamp(0),
            },
            ExerciseTrial {
                score: 2.0,
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
        assert_eq!(0.0, SCORER.score(&[]).unwrap());
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

        let score = SCORER.score(&trials).unwrap();
        assert!(score > 0.0 && score <= 5.0);
        assert!(score > 2.0); // Decent due to good recent performance
    }

    /// Verifies scoring an exercise with an invalid timestamp still returns a sane score.
    #[test]
    fn invalid_timestamp() -> Result<()> {
        let score = SCORER.score(&[ExerciseTrial {
            score: 5.0,
            timestamp: generate_timestamp(1e10 as i64),
        }])?;
        assert!(score >= 0.0 && score <= 5.0);
        assert!(score < 1.0); // Low due to long time elapsed
        Ok(())
    }

    /// Verifies stability computation evolves correctly through reviews.
    #[test]
    fn compute_stability() {
        let difficulty = 5.0;
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
        let stability = 1.0;
        // Recent review: high retrievability
        let recent = PowerLawScorer::compute_retrievability(0.01, stability);
        assert!(recent > 0.9);
        // Old review: moderate retrievability
        let old = PowerLawScorer::compute_retrievability(10.0, stability);
        assert!(old < 0.6 && old > 0.4);
        // Very old: low retrievability
        let very_old = PowerLawScorer::compute_retrievability(100.0, stability);
        assert!(very_old < 0.25);
    }

    /// Verifies score for perfect performance on recent exercise.
    #[test]
    fn score_perfect_recent() -> Result<()> {
        let trials = vec![
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(0),
            },
            ExerciseTrial {
                score: 5.0,
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
                score: 5.0,
                timestamp: generate_timestamp(4),
            },
        ];
        let score = SCORER.score(&trials)?;
        assert!((score - 5.0).abs() < 0.01); // Should be very close to 5.0
        Ok(())
    }

    /// Verifies score for bad performance on recent exercise.
    #[test]
    fn score_bad_recent() -> Result<()> {
        let trials = vec![
            ExerciseTrial {
                score: 1.0,
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
                score: 1.0,
                timestamp: generate_timestamp(3),
            },
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(4),
            },
        ];
        let score = SCORER.score(&trials)?;
        assert!((score - 0.0).abs() < 0.01); // Should be very close to 0.0
        Ok(())
    }

    /// Verifies score for mixed performance history.
    #[test]
    fn score_mixed_performance() -> Result<()> {
        let trials = vec![
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(0),
            }, // newest
            ExerciseTrial {
                score: 4.0,
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
                score: 3.0,
                timestamp: generate_timestamp(4),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(5),
            },
            ExerciseTrial {
                score: 2.0,
                timestamp: generate_timestamp(6),
            },
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(7),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(8),
            },
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(9),
            }, // oldest
        ];
        let score = SCORER.score(&trials)?;
        assert!(score > 2.0 && score < 3.0); // Based on output 2.5
        Ok(())
    }

    /// Verifies score for unsorted trials returns error.
    #[test]
    fn score_unsorted_trials() {
        let result = SCORER.score(&[
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(2), // Older first
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(1), // Newer
            },
        ]);
        assert!(result.is_err());
    }

    // TODO: Fix score_old_timestamp: Make realistic with old perfect trial, use println to determine tight assertion bounds.

    /// Verifies score for old timestamp gives low score.
    #[test]
    fn score_old_timestamp() -> Result<()> {
        let score = SCORER.score(&[ExerciseTrial {
            score: 5.0,
            timestamp: generate_timestamp(100), // Very old
        }])?;
        assert!(score > 1.0 && score < 1.5); // Based on output 1.23
        Ok(())
    }

    /// Verifies score for multiple good trials.
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
        let score = SCORER.score(&trials)?;
        assert!((score - 5.0).abs() < 0.01); // High score for good history
        Ok(())
    }

    /// Verifies score for multiple bad trials.
    #[test]
    fn score_multiple_bad() -> Result<()> {
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
                score: 1.0,
                timestamp: generate_timestamp(2),
            },
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(3),
            },
            ExerciseTrial {
                score: 2.0,
                timestamp: generate_timestamp(4),
            },
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(5),
            },
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(6),
            },
            ExerciseTrial {
                score: 2.0,
                timestamp: generate_timestamp(7),
            },
        ];
        let score = SCORER.score(&trials)?;
        assert!((score - 0.0).abs() < 0.01); // Low score for bad history
        Ok(())
    }
}
