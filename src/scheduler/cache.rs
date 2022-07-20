//! Module defining a cache of previously computed exercises scores and utilities to compute the
//! scores of lessons and courses.
use anyhow::{anyhow, Result};
use parking_lot::RwLock;
use ustr::{Ustr, UstrMap};

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
    /// A mapping of exercise ID to cached score.
    cache: RwLock<UstrMap<f32>>,

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
            cache: RwLock::new(UstrMap::default()),
            data,
            options: RwLock::new(options),
            scorer: Box::new(SimpleScorer {}),
        }
    }

    /// Removes the cached score for the given exercise.
    pub(super) fn invalidate_cached_score(&self, exercise_id: &Ustr) {
        self.cache.write().remove(exercise_id);
    }

    /// Returns the score for the given exercise.
    pub(super) fn get_exercise_score(&self, exercise_id: &Ustr) -> Result<f32> {
        let cached_score = self.cache.read().get(exercise_id).cloned();
        if let Some(score) = cached_score {
            return Ok(score);
        }

        let unit_type = self.data.unit_graph.read().get_unit_type(exercise_id);
        match unit_type {
            Some(UnitType::Exercise) => (),
            _ => {
                return Err(anyhow!(
                    "invalid unit type for exercise with ID {}",
                    exercise_id
                ))?
            }
        }

        let scores = self
            .data
            .practice_stats
            .read()
            .get_scores(exercise_id, self.options.read().num_scores)?;
        let score = self.scorer.score(scores);
        self.cache.write().insert(*exercise_id, score);
        Ok(score)
    }

    /// Returns the average score of all the exercises in the given lesson.
    fn get_lesson_score(&self, lesson_id: &Ustr) -> Result<Option<f32>> {
        let blacklisted = self.data.blacklist.read().blacklisted(lesson_id);
        if blacklisted.unwrap_or(false) {
            return Ok(None);
        }

        let exercises = self.data.unit_graph.read().get_lesson_exercises(lesson_id);
        match exercises {
            None => Ok(None),
            Some(exercise_ids) => {
                let valid_exercises = exercise_ids
                    .into_iter()
                    .filter(|exercise_id| {
                        let blacklisted = self.data.blacklist.read().blacklisted(exercise_id);
                        !blacklisted.unwrap_or(false)
                    })
                    .collect::<Vec<Ustr>>();
                if valid_exercises.is_empty() {
                    return Ok(None);
                }

                let avg_score: f32 = valid_exercises
                    .iter()
                    .map(|id| self.get_exercise_score(id))
                    .collect::<Result<Vec<f32>>>()?
                    .into_iter()
                    .sum::<f32>()
                    / valid_exercises.len() as f32;
                Ok(Some(avg_score))
            }
        }
    }

    /// Returns the average score of all the exercises in the given course.
    fn get_course_score(&self, course_id: &Ustr) -> Result<Option<f32>> {
        let blacklisted = self.data.blacklist.read().blacklisted(course_id);
        if blacklisted.unwrap_or(false) {
            return Ok(None);
        }

        let lessons = self.data.unit_graph.read().get_course_lessons(course_id);
        match lessons {
            None => Ok(None),
            Some(lesson_ids) => {
                let valid_lesson_scores = lesson_ids
                    .into_iter()
                    .map(|lesson_id| self.get_lesson_score(&lesson_id))
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
    pub(super) fn get_unit_score(&self, unit_id: &Ustr) -> Result<Option<f32>> {
        let unit_type = self
            .data
            .unit_graph
            .read()
            .get_unit_type(unit_id)
            .ok_or_else(|| anyhow!("missing unit type for unit with ID {}", unit_id))?;
        match unit_type {
            UnitType::Course => self.get_course_score(unit_id),
            UnitType::Lesson => self.get_lesson_score(unit_id),
            UnitType::Exercise => match self.get_exercise_score(unit_id) {
                Err(e) => Err(e),
                Ok(score) => Ok(Some(score)),
            },
        }
    }
}
