//! Contains the logic to score an exercise based on the results and timestamps of previous trials.

use anyhow::{anyhow, Result};
use chrono::{TimeZone, Utc};

use crate::data::ExerciseTrial;

/// A trait exposing a function to score an exercise based on the results of previous trials.
pub trait ExerciseScorer {
    /// Returns a score (between 0.0 and 5.0) for the exercise based on the results and timestamps
    /// of previous trials. The trials are assumed to be sorted in descending order by timestamp.
    fn score(&self, previous_trials: &[ExerciseTrial]) -> Result<f32>;
}

/// The initial decay rate for the score of a trial. This is the rate at which the score decreases
/// with each passing day.
const INITIAL_DECAY_RATE: f32 = 0.2;

/// The factor at which the decay rate is adjusted for each additional trial. This simulates how
/// skill performance deteriorates more slowly after repeated practice.
const DECAY_RATE_ADJUSTMENT_FACTOR: f32 = 0.8;

/// The initial minimum score for an exercise after exponential decay is applied as a factor of the
/// original score.
const INITIAL_MIN_SCORE_FACTOR: f32 = 0.75;

/// The minimum score factor will be adjusted by this factor with additional trials. This is to
/// simulate how the performance floor of a skill increases with practice. The value must be greater
/// than one.
const MIN_SCORE_ADJUSTMENT_FACTOR: f32 = 1.05;

/// The maximum score for an exercise after exponential decay is applied as a factor of the original
/// score and the adjustment increases the minimum score. It always should be less than 1.0.
const MAX_MIN_SCORE_FACTOR: f32 = 0.95;

/// The initial weight for an individual trial.
const INITIAL_WEIGHT: f32 = 1.0;

/// The weight of a trial is adjusted based on the index of the trial in the list. The first trial
/// has the initial weight, and the weight decreases with each subsequent trial by this factor.
const WEIGHT_INDEX_FACTOR: f32 = 0.8;

/// The minimum weight of a score, also used when there's an issue calculating the number of days
/// since the trial (e.g., the score's timestamp is after the current timestamp).
const MIN_WEIGHT: f32 = 0.1;

/// A scorer that uses an exponential decay function to compute the score of an exercise. As more
/// trials are completed, the decay rate decreases and the minimum score increases to simulate how
/// skills are retained better with more practice.
pub struct ExponentialDecayScorer {}

impl ExponentialDecayScorer {
    /// Returns the number of days between trials.
    #[inline]
    fn day_diffs(previous_trials: &[ExerciseTrial]) -> Vec<f32> {
        let mut now_plus_trials = vec![ExerciseTrial {
            timestamp: Utc::now().timestamp(),
            score: 0.0,
        }];
        now_plus_trials.extend(previous_trials.iter().cloned());
        now_plus_trials
            .windows(2)
            .map(|w| {
                let t1 = Utc
                    .timestamp_opt(w[0].timestamp, 0)
                    .earliest()
                    .unwrap_or_default();
                let t2 = Utc
                    .timestamp_opt(w[1].timestamp, 0)
                    .earliest()
                    .unwrap_or_default();
                (t1 - t2).num_days() as f32
            })
            .collect()
    }

    /// Returns the decay rates for each score based on the number of trials.
    #[inline]
    fn decay_rates(num_trials: usize) -> Vec<f32> {
        (0..num_trials)
            .map(|i| (INITIAL_DECAY_RATE * DECAY_RATE_ADJUSTMENT_FACTOR.powf(i as f32)).abs())
            .rev()
            .collect()
    }

    /// Returns the minimum score factors for each trial based on the number of trials. The minimum
    /// score is the score of the trial times this factor.
    #[inline]
    fn min_score_factors(num_trials: usize) -> Vec<f32> {
        (0..num_trials)
            .map(|i| {
                (INITIAL_MIN_SCORE_FACTOR * MIN_SCORE_ADJUSTMENT_FACTOR.powf(i as f32))
                    .min(MAX_MIN_SCORE_FACTOR)
            })
            .rev()
            .collect()
    }

    /// Performs the exponential decay on the score based on the number of days since the trial with
    /// the given minimum score and decay rate.
    #[inline]
    fn exponential_decay(
        initial_score: f32,
        num_days: f32,
        min_score_factor: f32,
        decay_rate: f32,
    ) -> f32 {
        // If the number of days is negative, return the score as is.
        if num_days < 0.0 {
            return initial_score;
        }

        // Compute the exponential decay using the formula:
        // min_score = initial_score * min_score_factor
        // S(num_days) = min_score + (initial_score - min_score) * e^(-decay_rate * num_days)
        let min_score = initial_score * min_score_factor;
        (min_score + (initial_score - min_score) * (-decay_rate * num_days).exp()).clamp(0.0, 5.0)
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

    /// Returns the weighted average of the scores.
    #[inline]
    fn weighted_average(scores: &[f32], weights: &[f32]) -> f32 {
        // weighted average = (cross product of scores and their weights) / (sum of weights)
        let cross_product: f32 = scores.iter().zip(weights.iter()).map(|(s, w)| s * *w).sum();
        let weight_sum = weights.iter().sum::<f32>();
        cross_product / weight_sum
    }
}

impl ExerciseScorer for ExponentialDecayScorer {
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

        // Compute the scores by running exponential decay on each trial.
        let days = Self::day_diffs(previous_trials);
        let decay_rates = Self::decay_rates(previous_trials.len());
        let min_score_factors = Self::min_score_factors(previous_trials.len());
        let scores: Vec<f32> = previous_trials
            .iter()
            .zip(days.iter())
            .zip(decay_rates.iter())
            .zip(min_score_factors.iter())
            .map(|(((trial, num_days), decay_rate), factor)| {
                Self::exponential_decay(trial.score, num_days.abs(), *factor, *decay_rate)
            })
            .collect();

        // Run a weighted average on the scores to compute the final score.
        let weights = Self::score_weights(previous_trials.len());
        Ok(Self::weighted_average(&scores, &weights))
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use chrono::Utc;

    use crate::{data::ExerciseTrial, exercise_scorer::*};

    const SECONDS_IN_DAY: i64 = 60 * 60 * 24;
    const SCORER: ExponentialDecayScorer = ExponentialDecayScorer {};

    /// Generates a timestamp equal to the timestamp from `num_days` ago.
    fn generate_timestamp(num_days: i64) -> i64 {
        let now = Utc::now().timestamp();
        now - num_days * SECONDS_IN_DAY
    }

    /// Verifies the number of days between two timestamps is calculated correctly.
    #[test]
    fn day_diffs() {
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
        let days = ExponentialDecayScorer::day_diffs(&trials);
        assert_eq!(days, vec![5.0, 5.0, 10.0]);
    }

    /// Verifies the decay rates are calculated correctly.
    #[test]
    fn decay_rates() {
        let num_trials = 4;
        let decay_rates = ExponentialDecayScorer::decay_rates(num_trials);
        assert_eq!(
            decay_rates,
            vec![
                INITIAL_DECAY_RATE * DECAY_RATE_ADJUSTMENT_FACTOR.powf(3.0),
                INITIAL_DECAY_RATE * DECAY_RATE_ADJUSTMENT_FACTOR.powf(2.0),
                INITIAL_DECAY_RATE * DECAY_RATE_ADJUSTMENT_FACTOR,
                INITIAL_DECAY_RATE,
            ]
        );
    }

    /// Verifies the minimum score factors are calculated correctly.
    #[test]
    fn min_score_factors() {
        let num_trials = 4;
        let min_score_factors = ExponentialDecayScorer::min_score_factors(num_trials);
        assert_eq!(
            min_score_factors,
            vec![
                INITIAL_MIN_SCORE_FACTOR * MIN_SCORE_ADJUSTMENT_FACTOR.powf(3.0),
                INITIAL_MIN_SCORE_FACTOR * MIN_SCORE_ADJUSTMENT_FACTOR.powf(2.0),
                INITIAL_MIN_SCORE_FACTOR * MIN_SCORE_ADJUSTMENT_FACTOR,
                INITIAL_MIN_SCORE_FACTOR,
            ]
        );

        // Assert that each value is greater than the next.
        for i in 0..min_score_factors.len() - 1 {
            assert!(min_score_factors[i] > min_score_factors[i + 1]);
        }
    }

    /// Verifies exponential decay returns the original score when the number of days is zero.
    #[test]
    fn exponential_decay_zero_days() {
        let initial_score = 5.0;
        let num_days = 0.0;
        let min_score_factor = 0.5;
        let decay_rate = 0.2;

        let adjusted_score = ExponentialDecayScorer::exponential_decay(
            initial_score,
            num_days,
            min_score_factor,
            decay_rate,
        );
        assert_eq!(adjusted_score, initial_score);
    }

    /// Verifies exponential decay converges to the minimum score.
    #[test]
    fn exponential_decay_converges() {
        let initial_score = 5.0;
        let num_days = 1000.0;
        let min_score_factor = 0.5;
        let decay_rate = 0.1;

        let adjusted_score = ExponentialDecayScorer::exponential_decay(
            initial_score,
            num_days,
            min_score_factor,
            decay_rate,
        );
        assert_eq!(adjusted_score, initial_score * min_score_factor);
    }

    /// Verifies exponential decay returns the original score when the number of days is negative.
    #[test]
    fn exponential_decay_negative_days() {
        let initial_score = 5.0;
        let num_days = -1.0;
        let min_score_factor = 0.5;
        let decay_rate = 0.2;

        let adjusted_score = ExponentialDecayScorer::exponential_decay(
            initial_score,
            num_days,
            min_score_factor,
            decay_rate,
        );
        assert_eq!(adjusted_score, initial_score);
    }

    /// Verifies the score for an exercise with no previous trials is 0.0.
    #[test]
    fn no_previous_trials() {
        assert_eq!(0.0, SCORER.score(&[]).unwrap());
    }

    /// Verifies that the scorer fails if the trials are not sorted in descending order.
    #[test]
    fn trials_not_sorted() {
        let trials = vec![
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(100),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(2),
            },
        ];
        assert!(SCORER.score(&trials).is_err());
    }

    /// Verifies the assignment of weights to trials based on their index.
    #[test]
    fn score_weights() {
        let num_trials = 4;
        let weights = ExponentialDecayScorer::score_weights(num_trials);
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
        let weights = ExponentialDecayScorer::score_weights(1000);
        assert_eq!(weights[weights.len() - 1], MIN_WEIGHT);
    }

    /// Verifies running the full scoring algorithm on a set of trials produces the expected score.
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
        assert!((score - 3.726246).abs() < f32::EPSILON);
    }

    /// Verifies scoring an exercise with an invalid timestamp still returns a sane score.
    #[test]
    fn invalid_timestamp() -> Result<()> {
        // The timestamp is before the Unix epoch.
        assert_eq!(
            SCORER.score(&[ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(1e10 as i64)
            },])?,
            3.75
        );
        Ok(())
    }
}
