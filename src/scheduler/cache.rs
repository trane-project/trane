//! Module defining a cache of previously computed exercises scores and utilities to compute the
//! scores of lessons and courses.
use anyhow::{anyhow, Result};
use parking_lot::RwLock;
use std::collections::HashMap;

use crate::{
    blacklist::Blacklist,
    data::{SchedulerOptions, UnitType},
    graph::UnitGraph,
    practice_stats::PracticeStats,
    scheduler::SchedulerData,
    scorer::{ExerciseScorer, SimpleScorer},
};

/// A cache of exercise scores with utility methods to compute the score of the exercises in a
/// lesson or course.
pub(super) struct ScoreCache {
    /// A mapping of exercise uid to cached score.
    cache: RwLock<HashMap<u64, f32>>,

    /// The data used schedule exercises.
    data: SchedulerData,

    /// The options used to schedule exercises.
    options: RwLock<SchedulerOptions>,

    /// The scorer used to score the exercises found during the search.
    scorer: Box<dyn ExerciseScorer + Send + Sync>,
}

impl ScoreCache {
    /// Constructs a new score cache.
    pub(super) fn new(data: SchedulerData, options: SchedulerOptions) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            data,
            options: RwLock::new(options),
            scorer: Box::new(SimpleScorer {}),
        }
    }

    /// Removes the cached score for the given exercise.
    pub(super) fn invalidate_cached_score(&self, exercise_uid: u64) {
        self.cache.write().remove(&exercise_uid);
    }

    /// Returns the score for the given exercise.
    pub(super) fn get_exercise_score(&self, exercise_uid: u64) -> Result<f32> {
        let cached_score = self.cache.read().get(&exercise_uid).cloned();
        if let Some(score) = cached_score {
            return Ok(score);
        }

        let unit_type = self.data.unit_graph.read().get_unit_type(exercise_uid);
        match unit_type {
            Some(UnitType::Exercise) => (),
            _ => {
                return Err(anyhow!(
                    "invalid unit type for exercise with UID {}",
                    exercise_uid
                ))?
            }
        }

        let exercise_id = self.data.get_id(exercise_uid)?;
        let scores = self
            .data
            .practice_stats
            .read()
            .get_scores(&exercise_id, self.options.read().num_scores)?;
        let score = self.scorer.score(scores);
        self.cache.write().insert(exercise_uid, score);
        Ok(score)
    }

    /// Returns the average score of all the exercises in the given lesson.
    fn get_lesson_score(&self, lesson_uid: u64) -> Result<Option<f32>> {
        let lesson_id = self.data.get_id(lesson_uid)?;
        let blacklisted = self.data.blacklist.read().blacklisted(&lesson_id);
        if blacklisted.unwrap_or(false) {
            return Ok(None);
        }

        let exercises = self.data.unit_graph.read().get_lesson_exercises(lesson_uid);
        match exercises {
            None => Ok(None),
            Some(uids) => {
                let valid_exercises = uids
                    .into_iter()
                    .filter(|exercise_uid| {
                        let exercise_id = self.data.get_id(*exercise_uid).unwrap_or_default();
                        let blacklisted = self.data.blacklist.read().blacklisted(&exercise_id);
                        !blacklisted.unwrap_or(false)
                    })
                    .collect::<Vec<u64>>();
                if valid_exercises.is_empty() {
                    return Ok(None);
                }

                let avg_score: f32 = valid_exercises
                    .iter()
                    .map(|uid| self.get_exercise_score(*uid))
                    .collect::<Result<Vec<f32>>>()?
                    .into_iter()
                    .sum::<f32>()
                    / valid_exercises.len() as f32;
                Ok(Some(avg_score))
            }
        }
    }

    /// Returns the average score of all the exercises in the given course.
    fn get_course_score(&self, course_uid: u64) -> Result<Option<f32>> {
        let course_id = self.data.get_id(course_uid)?;
        let blacklisted = self.data.blacklist.read().blacklisted(&course_id);
        if blacklisted.unwrap_or(false) {
            return Ok(None);
        }

        let lessons = self.data.unit_graph.read().get_course_lessons(course_uid);
        match lessons {
            None => Ok(None),
            Some(uids) => {
                let valid_lesson_scores = uids
                    .into_iter()
                    .map(|uid| self.get_lesson_score(uid))
                    .filter(|score| {
                        if score.as_ref().unwrap_or(&None).is_none() {
                            return false;
                        }
                        true
                    })
                    .map(|score| score.unwrap_or(Some(0.0)).unwrap())
                    .collect::<Vec<f32>>();
                if valid_lesson_scores.is_empty() {
                    return Ok(None);
                }

                let avg_score: f32 =
                    valid_lesson_scores.iter().sum::<f32>() / valid_lesson_scores.len() as f32;
                Ok(Some(avg_score))
            }
        }
    }

    /// Returns the score of the unit based on its unit type along with the number of scores taken
    /// into acount. This second value is used to indicate the scheduler that the search should
    /// continue if all exercises in a lesson or lessons in a course have been blacklisted.
    pub(super) fn get_unit_score(&self, unit_uid: u64) -> Result<Option<f32>> {
        let unit_type = self
            .data
            .unit_graph
            .read()
            .get_unit_type(unit_uid)
            .ok_or_else(|| anyhow!("missing unit type for unit with UID {}", unit_uid))?;
        match unit_type {
            UnitType::Course => self.get_course_score(unit_uid),
            UnitType::Lesson => self.get_lesson_score(unit_uid),
            UnitType::Exercise => match self.get_exercise_score(unit_uid) {
                Err(e) => Err(e),
                Ok(score) => Ok(Some(score)),
            },
        }
    }
}
