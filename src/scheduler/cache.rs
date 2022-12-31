//@<lp-example-2
//! Defines a cache that is used to retrieve unit scores and stores previously computed exercise and
//! lesson scores
//!
//! During performance testing, it was found that caching exercise and lesson scores significantly
//! improved the performance of exercise scheduling. Caching course scores had a negligible impact,
//! so they are not cached, although they are still computed through this cache for consistency.
//>@lp-example-2

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
        // Remove the unit from the exercise and lesson caches. This is safe to do even though the
        // unit is at most in one cache because the caches are disjoint.
        self.exercise_cache.write().remove(unit_id);
        self.lesson_cache.write().remove(unit_id);

        // If the unit is an exercise, invalidate the cached score of its parent lesson.
        if let Some(lesson_id) = self.data.unit_graph.read().get_exercise_lesson(unit_id) {
            self.lesson_cache.write().remove(&lesson_id);
        }
    }

    /// Returns the score for the given exercise.
    fn get_exercise_score(&self, exercise_id: &Ustr) -> Result<f32> {
        // Return the cached score if it exists.
        let cached_score = self.exercise_cache.read().get(exercise_id).cloned();
        if let Some(score) = cached_score {
            return Ok(score);
        }

        // Retrieve the exercise's previous trials and compute its score.
        let scores = self
            .data
            .practice_stats
            .read()
            .get_scores(exercise_id, self.options.read().num_trials)
            .unwrap_or_default();
        let score = self.scorer.score(&scores)?;
        self.exercise_cache.write().insert(*exercise_id, score);
        Ok(score)
    }

    /// Returns the average score of all the exercises in the given lesson.
    fn get_lesson_score(&self, lesson_id: &Ustr) -> Result<Option<f32>> {
        // Check if the unit is blacklisted. A blacklisted unit has no score.
        let blacklisted = self.data.blacklist.read().blacklisted(lesson_id);
        if blacklisted.unwrap_or(false) {
            return Ok(None);
        }

        // Return the cached score if it exists.
        let cached_score = self.lesson_cache.read().get(lesson_id).cloned();
        if let Some(score) = cached_score {
            return Ok(score);
        }

        // Compute the average score of all the exercises in the lesson.
        let exercises = self.data.unit_graph.read().get_lesson_exercises(lesson_id);
        let score = match exercises {
            None => {
                // A lesson with no exercises has no valid score.
                Ok(None)
            }
            Some(exercise_ids) => {
                // Compute the list of valid exercises. All blacklisted exercises are ignored.
                let valid_exercises = exercise_ids
                    .into_iter()
                    .filter(|exercise_id| {
                        let blacklisted = self.data.blacklist.read().blacklisted(exercise_id);
                        !blacklisted.unwrap_or(false)
                    })
                    .collect::<Vec<Ustr>>();

                if valid_exercises.is_empty() {
                    // If all exercises are blacklisted, the lesson has no valid score.
                    Ok(None)
                } else {
                    // Compute the average score of the valid exercises.
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

        // Update the cache with a valid score.
        if score.is_ok() {
            self.lesson_cache
                .write()
                .insert(*lesson_id, *score.as_ref().unwrap());
        }

        score
    }

    /// Returns the average score of all the lesson scores in the given course.
    fn get_course_score(&self, course_id: &Ustr) -> Result<Option<f32>> {
        // Check if the unit is blacklisted. A blacklisted course has no valid score.
        let blacklisted = self.data.blacklist.read().blacklisted(course_id);
        if blacklisted.unwrap_or(false) {
            return Ok(None);
        }

        // Compute the average score of all the lessons in the course.
        let lessons = self.data.unit_graph.read().get_course_lessons(course_id);
        match lessons {
            None => {
                // A course with no lessons has no valid score.
                Ok(None)
            }
            Some(lesson_ids) => {
                // Collect all the valid scores from the course's lessons.
                let valid_lesson_scores = lesson_ids
                    .into_iter()
                    .map(|lesson_id| self.get_lesson_score(&lesson_id))
                    .filter(|score| {
                        // Filter out any lesson whose score is not valid.
                        if score.as_ref().unwrap_or(&None).is_none() {
                            return false;
                        }
                        true
                    })
                    .map(|score| score.unwrap_or(Some(0.0)).unwrap())
                    .collect::<Vec<f32>>();

                // Return an invalid score if all the lesson scores are invalid. This can happen if
                // all the lessons in the course are blacklisted.
                if valid_lesson_scores.is_empty() {
                    return Ok(None);
                }

                // Compute the average of the valid lesson scores.
                let avg_score: f32 =
                    valid_lesson_scores.iter().sum::<f32>() / valid_lesson_scores.len() as f32;
                Ok(Some(avg_score))
            }
        }
    }

    /// Returns the score for the given unit. A return value of `Ok(None)` indicates that there is
    /// not a valid score for the unit, such as when the unit is blacklisted. Such a unit is
    /// considered a satisfied dependency.
    pub(super) fn get_unit_score(&self, unit_id: &Ustr) -> Result<Option<f32>> {
        // Decide which method to call based on the unit type.
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
                Err(e) => Err(e), // grcov-excl-line
                Ok(score) => Ok(Some(score)),
            },
        }
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use lazy_static::lazy_static;
    use std::collections::BTreeMap;
    use ustr::Ustr;

    use crate::{blacklist::Blacklist, data::SchedulerOptions, scheduler::ScoreCache, testutil::*};

    static NUM_EXERCISES: usize = 2;

    lazy_static! {
        /// A simple set of courses to test the basic functionality of Trane.
        static ref TEST_LIBRARY: Vec<TestCourse> = vec![
            TestCourse {
                id: TestId(0, None, None),
                dependencies: vec![],
                metadata: BTreeMap::from([
                    (
                        "course_key_1".to_string(),
                        vec!["course_key_1:value_1".to_string()]
                    ),
                    (
                        "course_key_2".to_string(),
                        vec!["course_key_2:value_1".to_string()]
                    ),
                ]),
                lessons: vec![
                    TestLesson {
                        id: TestId(0, Some(0), None),
                        dependencies: vec![],
                        metadata: BTreeMap::from([
                            (
                                "lesson_key_1".to_string(),
                                vec!["lesson_key_1:value_1".to_string()]
                            ),
                            (
                                "lesson_key_2".to_string(),
                                vec!["lesson_key_2:value_1".to_string()]
                            ),
                        ]),
                        num_exercises: NUM_EXERCISES,
                    },
                    TestLesson {
                        id: TestId(0, Some(1), None),
                        dependencies: vec![TestId(0, Some(0), None)],
                        metadata: BTreeMap::from([
                            (
                                "lesson_key_1".to_string(),
                                vec!["lesson_key_1:value_2".to_string()]
                            ),
                            (
                                "lesson_key_2".to_string(),
                                vec!["lesson_key_2:value_2".to_string()]
                            ),
                        ]),
                        num_exercises: NUM_EXERCISES,
                    },
                ],
            },
        ];
    }

    /// Verifies that a score of `None` is returned for a blacklisted course.
    #[test]
    fn blacklisted_course_score() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let mut library = init_test_simulation(&temp_dir.path(), &TEST_LIBRARY)?;
        let scheduler_data = library.get_scheduler_data();
        let cache = ScoreCache::new(scheduler_data, SchedulerOptions::default());

        let course_id = Ustr::from("0");
        library.add_to_blacklist(&course_id)?;
        assert_eq!(cache.get_course_score(&course_id)?, None);
        Ok(())
    }
}
