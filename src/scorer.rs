//! Contains the logic to score an exercise based on the results and timestamps of previous trials.

use anyhow::{anyhow, Result};
use chrono::{TimeZone, Utc};

use crate::data::ExerciseTrial;

/// The weight of a score diminishes by the number of days multiplied by this factor.
const WEIGHT_DAY_FACTOR: f32 = 0.1;

/// The weight of a trial is adjusted based on the index of the trial in the list multiplied by this
/// factor. The most recent trial (with index zero) has the highest weight. This prevents scores
/// from the same day to be assigned the same weight.
const WEIGHT_INDEX_FACTOR: f32 = 0.5;

/// The initial weight for a score.
const INITIAL_WEIGHT: f32 = 10.0;

/// The minimum weight of a score. This weight is also assigned when there's an issue calculating
/// the number of days since the trial (e.g., the score's timestamp is after the current timestamp).
const MIN_WEIGHT: f32 = 1.0;

/// The score of a trial diminishes by the number of days multiplied by this factor.
const SCORE_ADJUSTMENT_FACTOR: f32 = 0.01;

/// A trait exposing a function to score an exercise based on the results of previous trials.
pub trait ExerciseScorer {
    /// Returns a score (between 0.0 and 5.0) for the exercise based on the results and timestamps
    /// of previous trials.
    fn score(&self, previous_trials: &[ExerciseTrial]) -> Result<f32>;
}

/// A simple scorer that computes a score based on the weighted average of previous scores.
///
/// The score is computed as a weighted average of the previous scores. The weight of each score is
/// based on the number of days since the trial and the index of the score in the list. The score is
/// adjusted based on the number of days to account for skills deteriorating over time.
pub struct SimpleScorer {}

impl ExerciseScorer for SimpleScorer {
    fn score(&self, previous_trials: &[ExerciseTrial]) -> Result<f32> {
        // An exercise with no previous trials is assigned a score of 0.0.
        if previous_trials.is_empty() {
            return Ok(0.0);
        }

        // Calculate the number of days since each trial.
        let now = Utc::now();
        let days = previous_trials
            .iter()
            .map(|t| -> Result<f32> {
                if let Some(utc_timestame) = Utc.timestamp_opt(t.timestamp, 0).earliest() {
                    Ok((now - utc_timestame).num_days() as f32)
                } else {
                    Err(anyhow!("Invalid timestamp for exercise trial"))
                }
            })
            .collect::<Result<Vec<f32>>>()?;

        // Calculate the weight of each score based on the number of days since the trial.
        let weights: Vec<f32> = previous_trials
            .iter()
            .zip(days.iter())
            .enumerate()
            .map(|(index, (_, num_days))| -> f32 {
                // If the difference is negative, there's been some error. Use the min weight for
                // this trial instead of ignoring it.
                if *num_days < 0.0 {
                    return MIN_WEIGHT;
                }

                // The weight decreases with the number of days.
                let mut weight = INITIAL_WEIGHT - WEIGHT_DAY_FACTOR * num_days;

                // Give the most recent scores a higher weight. Otherwise, scores from the same day
                // will be given the same weight, which might make initial progress more difficult.
                weight += ((previous_trials.len() - index) as f32) * WEIGHT_INDEX_FACTOR;

                // Make sure the weight is never less than the min weight.
                weight.max(MIN_WEIGHT)
            })
            .collect();

        // The score of the trial is adjusted based on the number of days since the trial. The score
        // decreases linearly with the number of days but is never less than half of the original.
        let scores: Vec<f32> = previous_trials
            .iter()
            .zip(days.iter())
            .map(|(t, num_days)| -> f32 {
                // If there's an issue with calculating the number of days since the trial, return
                // the score as is.
                if *num_days < 0.0 {
                    return t.score;
                }

                // The score decreases with the number of days but is never less than half of the
                // original score.
                (t.score - SCORE_ADJUSTMENT_FACTOR * num_days).max(t.score / 2.0)
            })
            .collect();

        // Calculate the weighted average.
        // weighted average = (cross product of scores and their weights) / (sum of weights)
        let cross_product: f32 = scores.iter().zip(weights.iter()).map(|(s, w)| s * *w).sum();
        let weight_sum = weights.iter().sum::<f32>();
        Ok(cross_product / weight_sum)
    }
}

/// An implementation of [Send] for [SimpleScorer]. This implementation is safe because
/// [SimpleScorer] stores no state.
unsafe impl Send for SimpleScorer {}

/// An implementation of [Sync] for [SimpleScorer]. This implementation is safe because
/// [SimpleScorer] stores no state.
unsafe impl Sync for SimpleScorer {}

#[cfg(test)]
mod test {
    use chrono::Utc;

    use crate::{
        data::ExerciseTrial,
        scorer::{
            ExerciseScorer, SimpleScorer, INITIAL_WEIGHT, MIN_WEIGHT, WEIGHT_DAY_FACTOR,
            WEIGHT_INDEX_FACTOR,
        },
    };

    use super::SCORE_ADJUSTMENT_FACTOR;

    const SECONDS_IN_DAY: i64 = 60 * 60 * 24;
    const SCORER: SimpleScorer = SimpleScorer {};

    /// Generates a timestamp equal to the timestamp from `num_days` ago.
    fn generate_timestamp(num_days: i64) -> i64 {
        let now = Utc::now().timestamp();
        now - num_days * SECONDS_IN_DAY
    }

    /// Verifies the score for an exercise with no previous trials is 0.0.
    #[test]
    fn no_previous_trials() {
        assert_eq!(0.0, SCORER.score(&vec![]).unwrap());
    }

    /// Verifies the expected score for an exercise with a single trial.
    #[test]
    fn single_trial() {
        let score1 = 4.0;
        let days1 = 1.0;
        let weight1 = INITIAL_WEIGHT - days1 * WEIGHT_DAY_FACTOR + WEIGHT_INDEX_FACTOR;
        let adjusted_score1 = score1 - days1 * SCORE_ADJUSTMENT_FACTOR;

        assert_eq!(
            adjusted_score1 * weight1 / weight1,
            SCORER
                .score(&vec![ExerciseTrial {
                    score: score1,
                    timestamp: generate_timestamp(days1 as i64)
                }])
                .unwrap()
        );
    }

    /// Verifies the expected score for an exercise is adjusted based on the number of days since
    /// the trial and the index of the trial in the list of previous trials.
    #[test]
    fn score_and_weight_adjusted_by_day_and_index() {
        // Both scores are from a few days ago. Calculate their weight and adjusted scores based on
        // the formula.
        let num_scores = 2.0;
        let score1 = 2.0;
        let days1 = 5.0;
        let weight1 =
            INITIAL_WEIGHT - days1 * WEIGHT_DAY_FACTOR + (num_scores) * WEIGHT_INDEX_FACTOR;
        let adjusted_score1 = score1 - days1 * SCORE_ADJUSTMENT_FACTOR;

        let score2 = 5.0;
        let days2 = 10.0;
        let weight2 =
            INITIAL_WEIGHT - days2 * WEIGHT_DAY_FACTOR + (num_scores - 1.0) * WEIGHT_INDEX_FACTOR;
        let adjusted_score2 = score2 - days2 * SCORE_ADJUSTMENT_FACTOR;

        assert_eq!(
            (weight1 * adjusted_score1 + weight2 * adjusted_score2) / (weight1 + weight2),
            SCORER
                .score(&vec![
                    ExerciseTrial {
                        score: score1,
                        timestamp: generate_timestamp(days1 as i64)
                    },
                    ExerciseTrial {
                        score: score2,
                        timestamp: generate_timestamp(days2 as i64)
                    },
                ])
                .unwrap()
        );
    }

    /// Verifies that the score for a trial in the future is not adjusted.
    #[test]
    fn score_after_now() {
        // The first score is from zero days ago. Its adjusted score is equal to the original score.
        let num_scores = 2.0;
        let score1 = 2.0;
        let days1 = 0.0;
        let weight1 = INITIAL_WEIGHT + (num_scores) * WEIGHT_INDEX_FACTOR;
        let adjusted_score1 = score1;

        // Give the second score a timestamp in the future. The weight will be set at the minimum
        // and the score will not be adjusted.
        let score2 = 5.0;
        let days2 = -2.0;
        let weight2 = MIN_WEIGHT;
        let adjusted_score2 = score2;

        assert_eq!(
            (weight1 * adjusted_score1 + weight2 * adjusted_score2) / (weight1 + weight2),
            SCORER
                .score(&vec![
                    ExerciseTrial {
                        score: score1,
                        timestamp: generate_timestamp(days1 as i64)
                    },
                    ExerciseTrial {
                        score: score2,
                        timestamp: generate_timestamp(days2 as i64)
                    },
                ])
                .unwrap()
        );
    }

    /// Verifies that the score and weight for a trial never go below the minimum.
    #[test]
    fn score_and_weight_never_less_than_minimum() {
        // The first score is from a few days ago. Its weight and adjusted score should not be
        // capped to a minimum.
        let num_scores = 2.0;
        let score1 = 2.0;
        let days1 = 4.0;
        let weight1 =
            INITIAL_WEIGHT - days1 * WEIGHT_DAY_FACTOR + (num_scores) * WEIGHT_INDEX_FACTOR;
        let adjusted_score1 = score1 - days1 * SCORE_ADJUSTMENT_FACTOR;

        // The second score is very old. Both its score and weight should be set to the minimum.
        let score2 = 5.0;
        let days2 = 1000.0;
        let weight2 = MIN_WEIGHT;
        let adjusted_score2 = score2 / 2.0;

        assert_eq!(
            (weight1 * adjusted_score1 + weight2 * adjusted_score2) / (weight1 + weight2),
            SCORER
                .score(&vec![
                    ExerciseTrial {
                        score: score1,
                        timestamp: generate_timestamp(days1 as i64)
                    },
                    ExerciseTrial {
                        score: score2,
                        timestamp: generate_timestamp(days2 as i64)
                    },
                ])
                .unwrap()
        );
    }

    /// Verify scoring an exercise with an invalid timestamp fails.
    #[test]
    fn invalid_timestamp() {
        // The timestamp is before the Unix epoch.
        assert!(SCORER
            .score(&vec![ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(1e10 as i64)
            },])
            .is_err());
    }
}
