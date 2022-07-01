use anyhow::{Ok, Result};
use rusqlite::Connection;

use super::{PracticeStats, PracticeStatsDB};
use crate::data::{ExerciseTrial, MasteryScore};

fn new_tests_stats() -> Result<Box<dyn PracticeStats>> {
    let connection = Connection::open_in_memory()?;
    let practice_stats = PracticeStatsDB::new(connection)?;
    Ok(Box::new(practice_stats))
}

fn assert_scores(expected: Vec<f32>, actual: Vec<ExerciseTrial>) {
    let only_scores: Vec<f32> = actual.iter().map(|t| t.score).collect();
    assert_eq!(expected, only_scores);
    let all_sorted = actual
        .iter()
        .enumerate()
        .map(|(i, _)| {
            if i == 0 {
                return true;
            }
            actual[i - 1].score > actual[i].score
        })
        .all(|b| b);
    assert!(all_sorted);
}

#[test]
fn basic() -> Result<()> {
    let mut stats = new_tests_stats()?;
    stats.record_exercise_score("ex_123", MasteryScore::Five, 1)?;
    let scores = stats.get_scores("ex_123", 1)?;
    assert_scores(vec![5.0], scores);
    Ok(())
}

#[test]
fn multiple_records() -> Result<()> {
    let mut stats = new_tests_stats()?;
    stats.record_exercise_score("ex_123", MasteryScore::Three, 1)?;
    stats.record_exercise_score("ex_123", MasteryScore::Four, 2)?;
    stats.record_exercise_score("ex_123", MasteryScore::Five, 3)?;

    let one_score = stats.get_scores("ex_123", 1)?;
    assert_scores(vec![5.0], one_score);

    let three_scores = stats.get_scores("ex_123", 3)?;
    assert_scores(vec![5.0, 4.0, 3.0], three_scores);

    let more_scores = stats.get_scores("ex_123", 10)?;
    assert_scores(vec![5.0, 4.0, 3.0], more_scores);
    Ok(())
}

#[test]
fn no_records() -> Result<()> {
    let stats = new_tests_stats()?;
    let scores = stats.get_scores("ex_123", 10)?;
    assert_scores(vec![], scores);
    Ok(())
}
