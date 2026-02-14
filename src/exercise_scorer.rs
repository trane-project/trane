//! Contains the logic to score an exercise based on the results and timestamps of previous trials.

use anyhow::{Result, anyhow};
use chrono::Utc;

use crate::{data::ExerciseTrial, utils};

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

/// The minimum performance factor applied to stability. A value of 0.5 means poor performance can
/// reduce the stability estimate by up to 50%.
const MIN_PERFORMANCE_FACTOR: f32 = 0.5;

/// The maximum performance factor applied to stability. A value of 1.5 means excellent performance
/// can increase the stability estimate by up to 50%.
const MAX_PERFORMANCE_FACTOR: f32 = 1.5;

/// The initial weight for an individual trial.
const INITIAL_WEIGHT: f32 = 1.0;

/// The weight of a trial is adjusted based on the index of the trial in the list. The first trial
/// has the initial weight, and the weight decreases with each subsequent trial by this factor.
const WEIGHT_INDEX_FACTOR: f32 = 0.8;

/// The minimum weight of a score, also used when there's an issue calculating the number of days
/// since the trial (e.g., the score's timestamp is after the current timestamp).
const MIN_WEIGHT: f32 = 0.1;

/// A scorer that uses a power-law forgetting curve to compute the score of an exercise, using
/// simple interval-based estimation of stability and difficulty. This models memory retention more
/// accurately than exponential decay by accounting for the "fat tail" of long-term memory. 
/// 
/// The estimated stability and difficulty are simpler than they are in something like FSRS to avoid
/// having to store additional state about exercises. Optimizing the ExerciseScorer is not as
/// important in Trane because it uses a graph structure and produces batches of mixed exercises to
/// review instead of trying to compute the optimal review for a flat list of exercises.
pub struct PowerLawScorer {}

impl PowerLawScorer {
    /// Returns the number of days since each trial to the current time.
    #[inline]
    fn days_since_now(previous_trials: &[ExerciseTrial]) -> Vec<f32> {
        let now = Utc::now().timestamp();
        previous_trials
            .iter()
            .map(|trial| ((now - trial.timestamp) as f32 / 86400.0).max(0.0))
            .collect()
    }

    /// Estimates the stability of the exercise based on the intervals between trials. Stability
    /// represents the number of days after which the probability of recall drops to 90%. Uses the
    /// median interval as a robust estimate, adjusted by recent performance.
    #[inline]
    fn estimate_stability(previous_trials: &[ExerciseTrial]) -> f32 {
        // Stability only makes sense for exercises with at least 2 trials.
        if previous_trials.len() < 2 {
            return DEFAULT_STABILITY;
        }

        // Calculate all intervals between consecutive reviews (in days).
        let intervals: Vec<f32> = previous_trials
            .windows(2)
            .map(|w| ((w[0].timestamp - w[1].timestamp) as f32 / 86400.0).abs())
            .filter(|&d| d > 0.0) // Ignore same-day reviews
            .collect();
        if intervals.is_empty() {
            return DEFAULT_STABILITY;
        }

        // Use median for a most robust estimate of the typical interval.
        let mut sorted_intervals = intervals.clone();
        sorted_intervals.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = sorted_intervals[sorted_intervals.len() / 2];

        // Adjust by recent performance: good performance increases effective stability.
        let avg_score: f32 =
            previous_trials.iter().map(|t| t.score).sum::<f32>() / previous_trials.len() as f32;
        let performance_factor = (avg_score / PERFORMANCE_BASELINE_SCORE)
            .clamp(MIN_PERFORMANCE_FACTOR, MAX_PERFORMANCE_FACTOR);
        (median * performance_factor).clamp(MIN_STABILITY, MAX_STABILITY)
    }

    /// Estimates the difficulty of the exercise based on failure rates. Difficulty ranges from 1.0
    /// (easiest) to 10.0 (hardest).
    #[inline]
    fn estimate_difficulty(previous_trials: &[ExerciseTrial]) -> f32 {
        // Assing the base probability to exercises with no history.
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

    /// Performs the power-law decay on the score based on the number of days since the trial, the
    /// estimated stability, and the estimated difficulty.
    ///
    /// The power-law forgetting curve is: R(t) = (1 + factor * t/S)^decay where R is
    /// retrievability, t is time, S is stability, and factor/decay are constants.
    ///
    /// Difficulty is applied as a post-hoc adjustment: harder exercises appear to decay faster.
    #[inline]
    fn power_law_decay(initial_score: f32, num_days: f32, stability: f32, difficulty: f32) -> f32 {
        // If the number of days is negative, return the score as is.
        if num_days < 0.0 {
            return initial_score;
        }

        // Calculate retrievability using the power-law forgetting curve: R(t) = (1 + factor *
        // t/S)^decay
        let retrievability =
            (1.0 + FORGETTING_CURVE_FACTOR * num_days / stability).powf(FORGETTING_CURVE_DECAY);

        // Apply difficulty adjustment: harder exercises appear to decay faster. D=1 (easy): no
        // penalty, D=10 (hard): ~15% faster effective decay. We apply this as a power to
        // retrievability.
        let difficulty_exponent = 1.0 + (difficulty - 1.0) / DIFFICULTY_FACTOR;
        let adjusted_retrievability = retrievability.powf(difficulty_exponent);

        // Scale by initial score and clamp to valid range.
        (adjusted_retrievability * initial_score).clamp(0.0, 5.0)
    }

    /// Returns the weights to used to compute the weighted average of the scores.
    #[inline]
    fn score_weights(num_trials: usize) -> Vec<f32> {
        (0..num_trials)
            .map(|i| {
                let weight = INITIAL_WEIGHT * WEIGHT_INDEX_FACTOR.powf(i as f32);
                weight.max(MIN_WEIGHT)
            })
            .collect()
    }
}

impl ExerciseScorer for PowerLawScorer {
    fn score(&self, previous_trials: &[ExerciseTrial]) -> Result<f32> {
        // An exercise with no previous trials is assigned a score of 0.0.
        if previous_trials.is_empty() {
            return Ok(0.0);
        }

        // Check the sorting of the trials is in descending order by timestamp.
        if previous_trials
            .windows(2)
            .any(|w| w[0].timestamp < w[1].timestamp)
        {
            return Err(anyhow!(
                "Exercise trials are not sorted in descending order by timestamp"
            ));
        }

        // Estimate stability and difficulty from the trial history.
        let stability = Self::estimate_stability(&previous_trials);
        let difficulty = Self::estimate_difficulty(&previous_trials);

        // Compute the scores by running power-law decay on each trial.
        let days = Self::days_since_now(&previous_trials);
        let scores: Vec<f32> = previous_trials
            .iter()
            .zip(days.iter())
            .map(|(trial, &num_days)| {
                Self::power_law_decay(trial.score, num_days, stability, difficulty)
            })
            .collect();

        // Run a weighted average on the scores to compute the final score.
        let weights = Self::score_weights(previous_trials.len());
        Ok(utils::weighted_average(&scores, &weights))
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

    /// Verifies the number of days since each trial is calculated correctly.
    #[test]
    fn days_since_now() {
        let trials = vec![
            ExerciseTrial {
                score: 0.0,
                timestamp: generate_timestamp(5),
            },
            ExerciseTrial {
                score: 0.0,
                timestamp: generate_timestamp(10),
            },
            ExerciseTrial {
                score: 0.0,
                timestamp: generate_timestamp(20),
            },
        ];
        let days = PowerLawScorer::days_since_now(&trials);
        // Should return days since each trial: 5, 10, 20
        assert!((days[0] - 5.0).abs() < 0.1);
        assert!((days[1] - 10.0).abs() < 0.1);
        assert!((days[2] - 20.0).abs() < 0.1);
    }

    /// Verifies that stability is estimated correctly from trial intervals.
    #[test]
    fn estimate_stability() {
        // Single trial should return default stability.
        let single_trial = vec![ExerciseTrial {
            score: 5.0,
            timestamp: generate_timestamp(0),
        }];
        assert_eq!(
            PowerLawScorer::estimate_stability(&single_trial),
            DEFAULT_STABILITY
        );

        // Multiple trials with regular intervals.
        let regular_trials = vec![
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(0),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(5),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(10),
            },
        ];
        let stability = PowerLawScorer::estimate_stability(&regular_trials);
        assert!((stability - 7.5).abs() < 0.1); // 5 days * (5.0/3.0) performance factor

        // Trials with high scores should have higher stability (performance bonus).
        let high_score_trials = vec![
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(0),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(10),
            },
        ];
        let high_stability = PowerLawScorer::estimate_stability(&high_score_trials);
        assert!(high_stability > 10.0); // Should be boosted by performance

        // Multiple trials with bad results should have low stability.
        let low_score_trials = vec![
            ExerciseTrial {
                score: 2.0,
                timestamp: generate_timestamp(0),
            },
            ExerciseTrial {
                score: 1.0,
                timestamp: generate_timestamp(10),
            },
        ];
        let low_stability = PowerLawScorer::estimate_stability(&low_score_trials);
        assert!(low_stability < 10.0); // Should be reduced by poor performance
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

    /// Verifies power-law decay returns the original score when the number of days is zero.
    #[test]
    fn power_law_decay_zero_days() {
        let initial_score = 5.0;
        let num_days = 0.0;
        let stability = 5.0;
        let difficulty = 5.0;

        let adjusted_score =
            PowerLawScorer::power_law_decay(initial_score, num_days, stability, difficulty);
        assert_eq!(adjusted_score, initial_score);
    }

    /// Verifies power-law decay converges to zero over long periods.
    #[test]
    fn power_law_decay_converges() {
        let initial_score = 5.0;
        let num_days = 1000.0;
        let stability = 5.0;
        let difficulty = 5.0;

        let adjusted_score =
            PowerLawScorer::power_law_decay(initial_score, num_days, stability, difficulty);
        // After 1000 days with stability of 5, score should be very low but not zero.
        assert!(adjusted_score > 0.0);
        assert!(adjusted_score < 1.0);
    }

    /// Verifies power-law decay returns the original score when the number of days is negative.
    #[test]
    fn power_law_decay_negative_days() {
        let initial_score = 5.0;
        let num_days = -1.0;
        let stability = 5.0;
        let difficulty = 5.0;

        let adjusted_score =
            PowerLawScorer::power_law_decay(initial_score, num_days, stability, difficulty);
        assert_eq!(adjusted_score, initial_score);
    }

    /// Verifies that difficulty affects the decay rate.
    #[test]
    fn power_law_difficulty_effect() {
        let initial_score = 5.0;
        let num_days = 10.0;
        let stability = 5.0;

        let easy_score = PowerLawScorer::power_law_decay(initial_score, num_days, stability, 1.0);
        let hard_score = PowerLawScorer::power_law_decay(initial_score, num_days, stability, 10.0);

        // Harder exercises should have lower scores after the same time period.
        assert!(hard_score < easy_score);
    }

    /// Verifies the score for an exercise with no previous trials is 0.0.
    #[test]
    fn no_previous_trials() {
        assert_eq!(0.0, SCORER.score(&[]).unwrap());
    }

    /// Verifies the assignment of weights to trials based on their index.
    #[test]
    fn score_weights() {
        let num_trials = 4;
        let weights = PowerLawScorer::score_weights(num_trials);
        assert_eq!(
            weights,
            vec![
                INITIAL_WEIGHT,
                INITIAL_WEIGHT * WEIGHT_INDEX_FACTOR,
                INITIAL_WEIGHT * WEIGHT_INDEX_FACTOR.powf(2.0),
                INITIAL_WEIGHT * WEIGHT_INDEX_FACTOR.powf(3.0),
            ]
        );
    }

    /// Verifies that the score weight is never less than the minimum weight.
    #[test]
    fn score_weight_capped_at_min() {
        let weights = PowerLawScorer::score_weights(1000);
        assert_eq!(weights[weights.len() - 1], MIN_WEIGHT);
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

        // Score should be between 0 and 5. Recent high scores should dominate, so score should be
        // closer to 4-5 range.
        let score = SCORER.score(&trials).unwrap();
        assert!(score > 0.0 && score < 5.0);
        assert!(score > 3.0);
    }

    /// Verifies scoring an exercise with an invalid timestamp still returns a sane score.
    #[test]
    fn invalid_timestamp() -> Result<()> {
        // The timestamp is before the Unix epoch (very old). Very old trials should have decayed
        // significantly but not to zero.
        let score = SCORER.score(&[ExerciseTrial {
            score: 5.0,
            timestamp: generate_timestamp(1e10 as i64),
        }])?;
        assert!(score > 0.0);
        assert!(score < 1.0);
        Ok(())
    }
}
