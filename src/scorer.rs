//! Contains the logic to score an exercise based on the results and timestamps of previous trials.

use chrono::{TimeZone, Utc};

use crate::data::ExerciseTrial;

/// The weight of a score diminishes by the number of days multiplied by this factor.
const SIMPLE_SCORER_WEIGHT_FACTOR: f32 = 0.05;

/// The maximum weight for a score. The maximum weight is equal to the maximum score (5.0).
const SIMPLE_SCORER_MAX_WEIGHT: f32 = 5.0;

/// The minimum weight of a score assigned when there's an issue calculating the number of days
/// since the trial (e.g., the score's timestamp is after the current timestamp).
const SIMPLE_SCORER_MIN_WEIGHT: f32 = 1.0;

/// The score of a score diminishes by the number of days multiplied by this factor.
const SIMPLE_SCORER_SCORE_FACTOR: f32 = 0.1;

/// A trait exposing a function to score an exercise based on the results of previous trials.
pub trait ExerciseScorer {
    /// Returns a score (between 0.0 and 5.0) for the exercise based on the results and timestamps
    /// of previous trials.
    fn score(&self, previous_trials: Vec<ExerciseTrial>) -> f32;
}

/// A simple scorer that computes a score based on the weighted average of previous scores.
///
/// The score is computed as a weighted average of the previous scores. The weight of each score is
/// based on the number of days since the trial. The score is also adjusted based on the number of
/// days to represent how skills deteriorate over time.
pub struct SimpleScorer {}

impl ExerciseScorer for SimpleScorer {
    fn score(&self, previous_trials: Vec<ExerciseTrial>) -> f32 {
        // An exercise with no previous trials is assigned a score of 0.0.
        if previous_trials.is_empty() {
            return 0.0;
        }

        // Calculate the number of days since each trial.
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
                // If the difference is negative, there's been some error. Use the min weight for
                // this trial instead of ignoring it.
                if *num_days < 0.0 {
                    return SIMPLE_SCORER_MIN_WEIGHT;
                }

                // The weight decreases with the number of days but is never less than half of the
                // original score.
                (SIMPLE_SCORER_MAX_WEIGHT - SIMPLE_SCORER_WEIGHT_FACTOR * num_days)
                    .max(t.score / 2.0)
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

                // The weight decreases with the number of days but is never less than half of the
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
        scorer::{ExerciseScorer, SimpleScorer},
    };

    const SECONDS_IN_DAY: i64 = 60 * 60 * 24;
    const SCORER: SimpleScorer = SimpleScorer {};

    /// Generates a timestamp equal to the timestamp from num_days ago.
    fn generate_timestamp(num_days: i64) -> i64 {
        let now = Utc::now().timestamp();
        now - num_days * SECONDS_IN_DAY
    }

    #[test]
    fn no_previous_trials() {
        assert_eq!(0.0, SCORER.score(vec![]))
    }

    #[test]
    fn single_trial() {
        assert_eq!(
            4.0 - 0.1,
            SCORER.score(vec![ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(1)
            }])
        )
    }

    #[test]
    fn score_and_weight_decrease_by_day() {
        assert_eq!(
            ((2.0 - 0.1) * 4.95 + (5.0 - 0.1 * 20.0) * 4.0) / (4.95 + 4.0),
            SCORER.score(vec![
                ExerciseTrial {
                    score: 2.0,
                    timestamp: generate_timestamp(1)
                },
                ExerciseTrial {
                    score: 5.0,
                    timestamp: generate_timestamp(20)
                },
            ])
        )
    }

    #[test]
    fn score_after_now() {
        assert_eq!(
            (2.0 * 5.0 + 5.0 * 1.0) / (5.0 + 1.0),
            SCORER.score(vec![
                ExerciseTrial {
                    score: 2.0,
                    timestamp: generate_timestamp(0)
                },
                ExerciseTrial {
                    score: 5.0,
                    timestamp: generate_timestamp(-2)
                },
            ])
        )
    }

    #[test]
    fn score_and_weight_never_less_than_half_score() {
        assert_eq!(
            (2.0 * 5.0 + 2.5 * 2.5) / (5.0 + 2.5),
            SCORER.score(vec![
                ExerciseTrial {
                    score: 2.0,
                    timestamp: generate_timestamp(0)
                },
                ExerciseTrial {
                    score: 5.0,
                    timestamp: generate_timestamp(1000)
                },
            ])
        )
    }
}
