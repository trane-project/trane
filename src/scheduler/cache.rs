//! Module defining a cache of previously computed exercises scores and utilities to compute the
//! scores of lessons and courses.
use anyhow::{anyhow, Result};
use std::{cell::RefCell, collections::HashMap};

use crate::{
    data::{SchedulerOptions, UnitType},
    scheduler::SchedulerData,
    scorer::{ExerciseScorer, SimpleScorer},
};

/// A cache of exercise scores with utility methods to compute the score of the exercises in a
/// lesson or course.
pub(super) struct ScoreCache {
    /// A mapping of exercise uid to cached score.
    cache: RefCell<HashMap<u64, f32>>,

    /// The data used schedule exercises.
    data: SchedulerData,

    /// The options used to schedule exercises.
    options: RefCell<SchedulerOptions>,

    /// The scorer used to score the exercises found during the search.
    scorer: Box<dyn ExerciseScorer>,
}

impl ScoreCache {
    /// Constructs a new score cache.
    pub(super) fn new(data: SchedulerData, options: SchedulerOptions) -> Self {
        Self {
            cache: RefCell::new(HashMap::new()),
            data,
            options: RefCell::new(options),
            scorer: Box::new(SimpleScorer {}),
        }
    }

    /// Replaces the options with the given value.
    pub(super) fn set_options(&self, options: SchedulerOptions) {
        self.options.replace(options);
    }

    /// Removes the cached score for the given exercise.
    pub(super) fn invalidate_cached_score(&self, exercise_uid: u64) {
        self.cache.borrow_mut().remove(&exercise_uid);
    }

    /// Returns the score for the given exercise.
    pub(super) fn get_exercise_score(&self, exercise_uid: u64) -> Result<f32> {
        let cached_score = self.cache.borrow_mut().get(&exercise_uid).cloned();
        if let Some(score) = cached_score {
            return Ok(score);
        }

        let unit_type = self.data.unit_graph.borrow().get_unit_type(exercise_uid);
        match unit_type {
            Some(UnitType::Exercise) => (),
            _ => {
                return Err(anyhow!(
                    "invalid unit type for exercise with UID {}",
                    exercise_uid
                ))?
            }
        }

        let exercise_id = self
            .data
            .unit_graph
            .borrow()
            .get_id(exercise_uid)
            .ok_or_else(|| anyhow!("missing ID for exercise with UID {}", exercise_uid))?;

        let scores = self
            .data
            .practice_stats
            .borrow()
            .get_scores(&exercise_id, self.options.borrow().num_scores)?;

        let score = self.scorer.score(scores);
        match score {
            None => Ok(0.0),
            Some(score) => {
                self.cache.borrow_mut().insert(exercise_uid, score);
                Ok(score)
            }
        }
    }

    fn get_unit_id(&self, unit_uid: u64) -> Result<String> {
        self.data
            .unit_graph
            .borrow()
            .get_id(unit_uid)
            .ok_or_else(|| anyhow!("missing ID for unit with UID {}", unit_uid))
    }

    /// Returns the average score of all the exercises in the given lesson.
    fn get_lesson_score(&self, lesson_uid: u64) -> Result<Option<f32>> {
        let exercises = self
            .data
            .unit_graph
            .borrow()
            .get_lesson_exercises(lesson_uid);
        match exercises {
            None => Ok(None),
            Some(uids) => {
                let valid_uids = uids
                    .into_iter()
                    .filter(|exercise_uid| {
                        let exercise_id = self.get_unit_id(*exercise_uid);
                        if exercise_id.is_err() {
                            return false;
                        }

                        let blacklisted = self
                            .data
                            .blacklist
                            .borrow()
                            .blacklisted(&exercise_id.unwrap());
                        blacklisted.is_err() || !blacklisted.unwrap()
                    })
                    .collect::<Vec<u64>>();

                if valid_uids.is_empty() {
                    return Ok(None);
                }

                let avg_score: f32 = valid_uids
                    .iter()
                    .map(|uid| self.get_exercise_score(*uid))
                    .collect::<Result<Vec<f32>>>()?
                    .into_iter()
                    .sum::<f32>()
                    / valid_uids.len() as f32;
                Ok(Some(avg_score))
            }
        }
    }

    /// Returns the average score of all the exercises in the given course.
    fn get_course_score(&self, course_uid: u64) -> Result<Option<f32>> {
        let course_id = self.get_unit_id(course_uid)?;
        let blacklisted = self.data.blacklist.borrow().blacklisted(&course_id);
        if blacklisted.is_ok() && !blacklisted.unwrap() {
            return Ok(None);
        }

        let lessons = self.data.unit_graph.borrow().get_course_lessons(course_uid);
        match lessons {
            None => Ok(None),
            Some(uids) => {
                let valid_scores = uids
                    .into_iter()
                    .filter(|lesson_uid| {
                        let lesson_id = self.get_unit_id(*lesson_uid);
                        if lesson_id.is_err() {
                            return false;
                        }

                        let blacklisted = self
                            .data
                            .blacklist
                            .borrow()
                            .blacklisted(&lesson_id.unwrap());
                        blacklisted.is_err() || !blacklisted.unwrap()
                    })
                    .map(|uid| self.get_lesson_score(uid))
                    .filter(|score| {
                        if score.is_err() || score.as_ref().unwrap().is_none() {
                            return false;
                        }
                        score.as_ref().unwrap().unwrap() > 0.0
                    })
                    .map(|score| score.unwrap().unwrap())
                    .collect::<Vec<f32>>();

                if valid_scores.is_empty() {
                    return Ok(None);
                }

                let avg_score: f32 = valid_scores.iter().sum::<f32>() / valid_scores.len() as f32;
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
            .borrow()
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
