//! Module defining the data structures used to score an exercise based on the user's previous
//! trials.
#[cfg(test)]
mod test;

use chrono::{TimeZone, Utc};

use crate::data::ExerciseTrial;

/// The weight of a score diminishes by the number of days multiplied by this factor.
const SIMPLE_SCORER_WEIGHT_FACTOR: f32 = 0.05;

/// The maximum weight for a score. The maximum weight is equal to the maximum score (5.0).
const SIMPLE_SCORER_MAX_WEIGHT: f32 = 5.0;

/// The minimum weight of a score assigned when there's an issue calculating the number of days
/// since the trial (e.g the score's timestamp is after the current timestamp).
const SIMPLE_SCORER_MIN_WEIGHT: f32 = 1.0;

/// The score of a score dimishes by the number of days multiplied by this factor.
const SIMPLE_SCORER_SCORE_FACTOR: f32 = 0.1;

/// A trait exposing functions to score an exercise based on the results of previous trials.
pub trait ExerciseScorer {
    /// Returns a score (between 0 and 5) for the exercise based on the results of previous trials.
    fn score(&self, previous_trials: Vec<ExerciseTrial>) -> f32;
}

/// A simple scorer that computes a score based on the weighted average of previous scores.
pub struct SimpleScorer {}

impl ExerciseScorer for SimpleScorer {
    fn score(&self, previous_trials: Vec<ExerciseTrial>) -> f32 {
        if previous_trials.is_empty() {
            return 0.0;
        }

        let now = Utc::now();
        let days: Vec<f32> = previous_trials
            .iter()
            .map(|t| -> f32 { (now - Utc.timestamp(t.timestamp, 0)).num_days() as f32 })
            .collect();

        // Calculate the weight of each score based on the number of days since the trial.
        let weights: Vec<f32> = previous_trials
            .iter()
            .zip(days.iter())
            .map(|(t, num_days)| -> f32 {
                if *num_days < 0.0 {
                    // If the difference is negative there's been some error. Use the min weight for
                    // this trial instead of ignoring it.
                    return SIMPLE_SCORER_MIN_WEIGHT;
                }

                // The weight decreses with the number of days but is never less than half of the
                // original score.
                (SIMPLE_SCORER_MAX_WEIGHT - SIMPLE_SCORER_WEIGHT_FACTOR * num_days)
                    .max(t.score / 2.0)
            })
            .collect();

        // Calculate the score of the trial based on the number of days since the trial.
        let scores: Vec<f32> = previous_trials
            .iter()
            .zip(days.iter())
            .map(|(t, num_days)| -> f32 {
                if *num_days < 0.0 {
                    // If there's an issue with calculating the number of days since the trial,
                    // return the score as is.
                    return t.score;
                }

                // The weight decreses with the number of days but is never less than half of the
                // original score.
                (t.score - SIMPLE_SCORER_SCORE_FACTOR * num_days).max(t.score / 2.0)
            })
            .collect();

        // Calculate the weighted average.
        // weighted average = (cross product of scores and their weights) / (sum of weights)
        let cross_product: f32 = scores.iter().zip(weights.iter()).map(|(s, w)| s * *w).sum();
        cross_product / weights.iter().sum::<f32>()
    }
}

unsafe impl Send for SimpleScorer {}
unsafe impl Sync for SimpleScorer {}
