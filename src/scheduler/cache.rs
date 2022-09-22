//! Defines a cache that is used to retrieve unit scores and stores previously computed exercise and
//! lesson scores
//!
//! During performance testing, it was found that caching exercise and lesson scores significantly
//! improved the performance of exercise scheduling. Caching course scores had a negligible impact,
//! so they are not cached, although they are still computed through this cache for consistency.

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

/// A cache of unit scores used to improve the performance of computing exercise and lesson scores
/// during scheduling. Course scores are also accessed through this struct but are not cached
/// because the performance improvement is negligible.
pub(super) struct ScoreCache {
    /// A mapping of exercise ID to cached score.
    exercise_cache: RwLock<UstrMap<f32>>,

    /// A mapping of lesson ID to cached score.
    lesson_cache: RwLock<UstrMap<Option<f32>>>,

    /// The data used by the scheduler.
    data: SchedulerData,

    /// The options used to schedule exercises.
    options: RwLock<SchedulerOptions>,

    /// The scorer used to compute the score of an exercise based on its previous trials.
    scorer: Box<dyn ExerciseScorer + Send + Sync>,
}

impl ScoreCache {
    /// Constructs a new score cache.
    pub(super) fn new(data: SchedulerData, options: SchedulerOptions) -> Self {
        Self {
            exercise_cache: RwLock::new(UstrMap::default()),
            lesson_cache: RwLock::new(UstrMap::default()),
            data,
            options: RwLock::new(options),
            scorer: Box::new(SimpleScorer {}),
        }
    }

    /// Removes the cached score for the given unit.
    pub(super) fn invalidate_cached_score(&self, unit_id: &Ustr) {
        self.exercise_cache.write().remove(unit_id);
        self.lesson_cache.write().remove(unit_id);

        if let Some(lesson_id) = self.data.unit_graph.read().get_exercise_lesson(unit_id) {
            self.lesson_cache.write().remove(&lesson_id);
        }
    }

    /// Returns the score for the given exercise.
    pub(super) fn get_exercise_score(&self, exercise_id: &Ustr) -> Result<f32> {
        let cached_score = self.exercise_cache.read().get(exercise_id).cloned();
        if let Some(score) = cached_score {
            return Ok(score);
        }

        let scores = self
            .data
            .practice_stats
            .read()
            .get_scores(exercise_id, self.options.read().num_scores)?;
        let score = self.scorer.score(scores);
        self.exercise_cache.write().insert(*exercise_id, score);
        Ok(score)
    }

    /// Returns the average score of all the exercises in the given lesson.
    fn get_lesson_score(&self, lesson_id: &Ustr) -> Result<Option<f32>> {
        let blacklisted = self.data.blacklist.read().blacklisted(lesson_id);
        if blacklisted.unwrap_or(false) {
            return Ok(None);
        }

        let cached_score = self.lesson_cache.read().get(lesson_id).cloned();
        if let Some(score) = cached_score {
            return Ok(score);
        }

        let exercises = self.data.unit_graph.read().get_lesson_exercises(lesson_id);
        let score = match exercises {
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
                    Ok(None)
                } else {
                    let avg_score: f32 = valid_exercises
                        .iter()
                        .map(|id| self.get_exercise_score(id))
                        .collect::<Result<Vec<f32>>>()? // grcov-excl-line
                        .into_iter()
                        .sum::<f32>()
                        / valid_exercises.len() as f32;
                    Ok(Some(avg_score))
                }
            }
        };

        if score.is_ok() {
            self.lesson_cache
                .write()
                .insert(*lesson_id, *score.as_ref().unwrap());
        }
        score
    }

    /// Returns the average score of all the lesson scores in the given course.
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

    /// Returns the score for the given unit. A return value of `Ok(None)` indicates that there is
    /// not a valid score for the unit, such as when the unit is blacklisted. The unit should be
    /// considered as a satisfied dependency if that is the case.
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
