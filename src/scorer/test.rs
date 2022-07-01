use super::{ExerciseScorer, SimpleScorer};
use crate::data::ExerciseTrial;
use chrono::Utc;

const SECONDS_IN_DAY: i64 = 60 * 60 * 24;
const SCORER: SimpleScorer = SimpleScorer {};

/// Generates a timestamp equal to the timestamp from num_days ago.
fn generate_timestamp(num_days: i64) -> i64 {
    let now = Utc::now().timestamp();
    now - num_days * SECONDS_IN_DAY
}

#[test]
fn no_previous_trials() {
    assert_eq!(None, SCORER.score(vec![]))
}

#[test]
fn single_trial() {
    assert_eq!(
        4.0 - 0.1,
        SCORER
            .score(vec![ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(1)
            }])
            .unwrap()
    )
}

#[test]
fn score_and_weight_decrease_by_day() {
    assert_eq!(
        ((2.0 - 0.1) * 4.95 + (5.0 - 0.1 * 20.0) * 4.0) / (4.95 + 4.0),
        SCORER
            .score(vec![
                ExerciseTrial {
                    score: 2.0,
                    timestamp: generate_timestamp(1)
                },
                ExerciseTrial {
                    score: 5.0,
                    timestamp: generate_timestamp(20)
                },
            ])
            .unwrap()
    )
}

#[test]
fn score_after_now() {
    assert_eq!(
        (2.0 * 5.0 + 5.0 * 1.0) / (5.0 + 1.0),
        SCORER
            .score(vec![
                ExerciseTrial {
                    score: 2.0,
                    timestamp: generate_timestamp(0)
                },
                ExerciseTrial {
                    score: 5.0,
                    timestamp: generate_timestamp(-2)
                },
            ])
            .unwrap()
    )
}

#[test]
fn score_and_weight_never_less_than_half_score() {
    assert_eq!(
        (2.0 * 5.0 + 2.5 * 2.5) / (5.0 + 2.5),
        SCORER
            .score(vec![
                ExerciseTrial {
                    score: 2.0,
                    timestamp: generate_timestamp(0)
                },
                ExerciseTrial {
                    score: 5.0,
                    timestamp: generate_timestamp(1000)
                },
            ])
            .unwrap()
    )
}
