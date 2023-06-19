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

/// The score of a trial diminishes at a faster rate during this number of days.
const SHORT_TERM_LENGTH: f32 = 10.0;

/// The score of a trial diminishes by this factor during the first [SHORT_TERM_LENGTH] days.
const SHORT_TERM_ADJUSTMENT_FACTOR: f32 = 0.025;

/// The score of a trial diminishes by this factor after [SHORT_TERM_LENGTH] days.
const LONG_TERM_ADJUSTMENT_FACTOR: f32 = 0.01;

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

impl SimpleScorer {
    /// Returns the weight of the score based on the number of days since the trial and the index of
    /// of the trial in the list.
    fn weight(num_trials: usize, trial_index: usize, num_days: f32) -> f32 {
        // If the difference is negative, there's been some error. Use the min weight for
        // this trial instead of ignoring it.
        if num_days < 0.0 {
            return MIN_WEIGHT;
        }

        // The weight decreases with the number of days.
        let mut weight = INITIAL_WEIGHT - WEIGHT_DAY_FACTOR * num_days;

        // Give the most recent scores a higher weight. Otherwise, scores from the same day
        // will be given the same weight, which might make initial progress more difficult.
        weight += (num_trials - trial_index) as f32 * WEIGHT_INDEX_FACTOR;

        // Make sure the weight is never less than the min weight.
        weight.max(MIN_WEIGHT)
    }

    /// Returns the adjusted score based on the number of days since the trial. The score decreases
    /// with each passing day to account for skills deteriorating over time.
    fn adjusted_score(score: f32, num_days: f32) -> f32 {
        // If there's an issue with calculating the number of days since the trial, return
        // the score as is.
        if num_days < 0.0 {
            return score;
        }

        // The score decreases with the number of days but is never less than half of the original
        // score. The score decreases faster during the first few days, but then decreases slower.
        // This is to simulate the fact that skills deteriorate faster during the first few days
        // after a trial but then settle into long-term memory.
        if num_days <= SHORT_TERM_LENGTH {
            (score - SHORT_TERM_ADJUSTMENT_FACTOR * num_days).max(score / 2.0)
        } else {
            let long_term_days = num_days - SHORT_TERM_LENGTH.max(0.0);
            let adjusted_score = score - SHORT_TERM_ADJUSTMENT_FACTOR * SHORT_TERM_LENGTH;
            (adjusted_score - LONG_TERM_ADJUSTMENT_FACTOR * long_term_days).max(score / 2.0)
        }
    }

    /// Returns the weighted average of the scores.
    fn weighted_average(scores: &[f32], weights: &[f32]) -> f32 {
        // weighted average = (cross product of scores and their weights) / (sum of weights)
        let cross_product: f32 = scores.iter().zip(weights.iter()).map(|(s, w)| s * *w).sum();
        let weight_sum = weights.iter().sum::<f32>();
        cross_product / weight_sum
    }
}

impl ExerciseScorer for SimpleScorer {
    fn score(&self, previous_trials: &[ExerciseTrial]) -> Result<f32> {
        // An exercise with no previous trials is assigned a score of 0.0.
        if previous_trials.is_empty() {
            return Ok(0.0);
        }

        // Calculate the number of days from each trial to the next, and from the last trial to now.
        // This assumes that the trials are sorted by timestamp in descending order.
        let days = previous_trials
            .iter()
            .enumerate()
            .map(|(i, t)| -> Result<f32> {
                let now = if i == 0 {
                    Utc::now()
                } else {
                    Utc.timestamp_opt(previous_trials[i - 1].timestamp, 0)
                        .earliest()
                        .unwrap_or_default()
                };

                if let Some(utc_timestamp) = Utc.timestamp_opt(t.timestamp, 0).earliest() {
                    Ok((now - utc_timestamp).num_days() as f32)
                } else {
                    Err(anyhow!("Invalid timestamp for exercise trial"))
                }
            })
            .collect::<Result<Vec<f32>>>()?;

        // Calculate the weight of each score based on the number of days since each trial to the
        // next.
        let weights: Vec<f32> = previous_trials
            .iter()
            .zip(days.iter())
            .enumerate()
            .map(|(index, (_, num_days))| -> f32 {
                Self::weight(previous_trials.len(), index, *num_days)
            })
            .collect();

        // The score of the trial is adjusted based on the number of days since the trial. The score
        // decreases linearly with the number of days but is never less than half of the original.
        let scores: Vec<f32> = previous_trials
            .iter()
            .zip(days.iter())
            .map(|(t, num_days)| -> f32 { Self::adjusted_score(t.score, *num_days) })
            .collect();

        // Return the weighted average of the scores.
        Ok(Self::weighted_average(&scores, &weights))
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

    use crate::{data::ExerciseTrial, scorer::*};

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

    /// Verifies that the score is not changed if the number of days since the trial is negative.
    #[test]
    fn negative_days() {
        let score = 4.0;
        assert_eq!(score, SimpleScorer::adjusted_score(score, -1.0));
    }

    /// Verifies that recent scores decrease faster.
    #[test]
    fn recent_scores_decrease_faster() {
        let score = 4.0;
        let days = 2.0;
        let adjusted_score = SimpleScorer::adjusted_score(score, days);
        assert_eq!(adjusted_score, score - days * SHORT_TERM_ADJUSTMENT_FACTOR);
    }

    /// Verifies that older scores decrease slower.
    #[test]
    fn older_scores_decrease_slower() {
        let score = 4.0;
        let days = 20.0;
        let adjusted_score = SimpleScorer::adjusted_score(score, days);
        assert_eq!(
            adjusted_score,
            score
                - SHORT_TERM_ADJUSTMENT_FACTOR * SHORT_TERM_LENGTH
                - (days - SHORT_TERM_LENGTH) * LONG_TERM_ADJUSTMENT_FACTOR
        );
    }

    /// Verifies that the adjusted score is never less than half of the original.
    #[test]
    fn score_capped_at_half() {
        let score = 4.0;
        let days = 1000.0;
        let adjusted_score = SimpleScorer::adjusted_score(score, days);
        assert_eq!(adjusted_score, score / 2.0);
    }

    /// Verifies that the minimum weight is returned if the number of days is negative.
    #[test]
    fn negative_days_weight() {
        let num_scores = 2;
        let index = 0;
        let days = -1.0;
        assert_eq!(SimpleScorer::weight(num_scores, index, days), MIN_WEIGHT,);
    }

    /// Verifiest that the weight is adjusted based on the index of the score and the number of days
    /// since the trial.
    #[test]
    fn weight_adjusted_by_day_and_index() {
        let num_scores = 2;
        let index = 2;
        let num_trials = 2;
        let days = 4.0;
        assert_eq!(
            SimpleScorer::weight(num_scores, index, days),
            INITIAL_WEIGHT - days * WEIGHT_DAY_FACTOR
                + (num_trials - index) as f32 * WEIGHT_INDEX_FACTOR
        );
    }

    /// Verifies that the weight is never less than the minimum weight.
    #[test]
    fn weight_capped_at_min() {
        let num_scores = 2;
        let index = 0;
        let days = 1000.0;
        assert_eq!(SimpleScorer::weight(num_scores, index, days), MIN_WEIGHT,);
    }

    /// Verifies the expected score for an exercise with a single trial.
    #[test]
    fn single_trial() {
        let score = 4.0;
        let days = 1.0;
        let adjusted_score = SimpleScorer::adjusted_score(score, days);

        assert_eq!(
            adjusted_score,
            SCORER
                .score(&vec![ExerciseTrial {
                    score: score,
                    timestamp: generate_timestamp(days as i64)
                }])
                .unwrap()
        );
    }

    /// Verifies the expected score for an exercise with multiple trials.
    #[test]
    fn multiple_trials() {
        // Both scores are from a few days ago. Calculate their weight and adjusted scores based on
        // the formula.
        let num_scores = 2;
        let score1 = 2.0;
        let days1 = 5.0;
        let weight1 = SimpleScorer::weight(num_scores, 0, days1);
        let adjusted_score1 = SimpleScorer::adjusted_score(score1, days1);

        let score2 = 5.0;
        let days2 = 10.0;
        let weight2 = SimpleScorer::weight(num_scores, 1, days2 - days1);
        let adjusted_score2 = SimpleScorer::adjusted_score(score2, days2 - days1);

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
