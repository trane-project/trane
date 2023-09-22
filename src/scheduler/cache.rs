//@<lp-example-2
//! Defines a cache that is used to retrieve unit scores and stores previously computed exercise and
//! lesson scores
//!
//! During performance testing, it was found that caching scores scores significantly improved the
//! performance of exercise scheduling.
//>@lp-example-2

use anyhow::{anyhow, Result};
use std::cell::RefCell;
use ustr::{Ustr, UstrMap};

use crate::{
    blacklist::Blacklist,
    data::{SchedulerOptions, UnitType},
    graph::UnitGraph,
    practice_stats::PracticeStats,
    scheduler::SchedulerData,
    scorer::{ExerciseScorer, SimpleScorer},
};

/// Stores information about a cached score.
#[derive(Clone)]
pub(super) struct CachedScore {
    /// The computed score.
    score: f32,

    /// The number of trials used to compute the score.
    num_trials: usize,
}

/// A cache of unit scores used to improve the performance of computing them during scheduling.
pub(super) struct ScoreCache {
    /// A mapping of exercise ID to cached score.
    exercise_cache: RefCell<UstrMap<CachedScore>>,

    /// A mapping of lesson ID to cached score.
    lesson_cache: RefCell<UstrMap<Option<f32>>>,

    /// A mapping of course ID to cached score.
    course_cache: RefCell<UstrMap<Option<f32>>>,

    /// The data used by the scheduler.
    data: SchedulerData,

    /// The options used to schedule exercises.
    options: SchedulerOptions,

    /// The scorer used to compute the score of an exercise based on its previous trials.
    scorer: Box<dyn ExerciseScorer + Send + Sync>,
}

impl ScoreCache {
    /// Constructs a new score cache.
    pub(super) fn new(data: SchedulerData, options: SchedulerOptions) -> Self {
        Self {
            exercise_cache: RefCell::new(UstrMap::default()),
            lesson_cache: RefCell::new(UstrMap::default()),
            course_cache: RefCell::new(UstrMap::default()),
            data,
            options,
            scorer: Box::new(SimpleScorer {}),
        }
    }

    /// Removes the cached score for the given unit.
    pub(super) fn invalidate_cached_score(&self, unit_id: &Ustr) {
        // Remove the unit from the exercise, lesson, and course caches. This is safe to do even
        // though the unit is at most in one cache because the different types of units are
        // disjoint.
        self.exercise_cache.borrow_mut().remove(unit_id);
        self.lesson_cache.borrow_mut().remove(unit_id);
        self.course_cache.borrow_mut().remove(unit_id);

        // If the unit is an exercise, invalidate the cached score of its lesson and course. If the
        // unit is a lesson, invalidate the cached score of its course.
        if let Some(lesson_id) = self.data.unit_graph.read().get_exercise_lesson(unit_id) {
            self.lesson_cache.borrow_mut().remove(&lesson_id);
            if let Some(course_id) = self.data.unit_graph.read().get_lesson_course(&lesson_id) {
                self.course_cache.borrow_mut().remove(&course_id);
            }
        } else if let Some(course_id) = self.data.unit_graph.read().get_lesson_course(unit_id) {
            self.course_cache.borrow_mut().remove(&course_id);
        }
    }

    /// Removes the cached score for any unit with the given prefix.
    pub(super) fn invalidate_cached_scores_with_prefix(&self, prefix: &str) {
        // Remove the unit from the exercise and lesson caches. This is safe to do even though the
        // unit is at most in one cache because the caches are disjoint.
        self.exercise_cache
            .borrow_mut()
            .retain(|unit_id, _| !unit_id.starts_with(prefix));
        self.lesson_cache
            .borrow_mut()
            .retain(|unit_id, _| !unit_id.starts_with(prefix));
    }

    /// Returns the score for the given exercise.
    fn get_exercise_score(&self, exercise_id: &Ustr) -> Result<f32> {
        // Return the cached score if it exists.
        let cached_score = self.exercise_cache.borrow().get(exercise_id).cloned();
        if let Some(cached_score) = cached_score {
            return Ok(cached_score.score);
        }

        // Retrieve the exercise's previous trials and compute its score.
        let scores = self
            .data
            .practice_stats
            .read()
            .get_scores(exercise_id, self.options.num_trials)
            .unwrap_or_default();
        let score = self.scorer.score(&scores)?;
        self.exercise_cache.borrow_mut().insert(
            *exercise_id,
            CachedScore {
                score,
                num_trials: scores.len(),
            },
        );
        Ok(score)
    }

    /// Returns the number of trials that were considered when computing the score for the given
    /// exercise.
    pub(super) fn get_num_trials(&self, exercise_id: &Ustr) -> Result<Option<usize>> {
        // Return the cached value if it exists.
        let cached_score = self.exercise_cache.borrow().get(exercise_id).cloned();
        if let Some(cached_score) = cached_score {
            return Ok(Some(cached_score.num_trials));
        }

        // Compute the exercise's score, which should populate the cache.
        self.get_exercise_score(exercise_id)?;

        // Return the cached value.
        let cached_score = self.exercise_cache.borrow().get(exercise_id).cloned();
        Ok(cached_score.map(|s| s.num_trials))
    }

    /// Returns the average score of all the exercises in the given lesson.
    fn get_lesson_score(&self, lesson_id: &Ustr) -> Result<Option<f32>> {
        // Check if the unit is blacklisted. A blacklisted unit has no score.
        let blacklist = self.data.blacklist.read();
        let blacklisted = blacklist.blacklisted(lesson_id);
        if blacklisted.unwrap_or(false) {
            return Ok(None);
        }

        // Return the cached score if it exists.
        let cached_score = self.lesson_cache.borrow().get(lesson_id).cloned();
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
                        let blacklisted = blacklist.blacklisted(exercise_id);
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
                        .sum::<Result<f32>>()? // grcov-excl-line
                        / valid_exercises.len() as f32;
                    Ok(Some(avg_score))
                }
            }
        };

        // Update the cache with a valid score.
        if score.is_ok() {
            self.lesson_cache
                .borrow_mut()
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

        // Return the cached score if it exists.
        let cached_score = self.course_cache.borrow().get(course_id).cloned();
        if let Some(score) = cached_score {
            return Ok(score);
        }

        // Compute the average score of all the lessons in the course.
        let lessons = self.data.unit_graph.read().get_course_lessons(course_id);
        let score = match lessons {
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
                    .collect::<Result<Vec<_>>>()?; // grcov-excl-line

                // Return an invalid score if all the lesson scores are invalid. This can happen if
                // all the lessons in the course are blacklisted.
                if valid_lesson_scores.is_empty() {
                    return Ok(None);
                }

                // Compute the average of the valid lesson scores.
                let avg_score: f32 = valid_lesson_scores
                    .iter()
                    .map(|s| s.unwrap_or_default())
                    .sum::<f32>()
                    / valid_lesson_scores.len() as f32;
                Ok(Some(avg_score))
            }
        };

        // Update the cache with a valid score.
        if score.is_ok() {
            self.course_cache
                .borrow_mut()
                .insert(*course_id, *score.as_ref().unwrap());
        }
        score
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
            }, // grcov-excl-line
        }
    }

    /// Returns whether all the exercises in the unit have valid scores.
    pub(super) fn all_valid_exercises_have_scores(&self, unit_id: &Ustr) -> bool {
        // Get all the valid exercises in the unit.
        let valid_exercises = self.data.all_valid_exercises(unit_id);
        if valid_exercises.is_empty() {
            return true;
        }

        // All valid exercises must have a score greater than 0.0.
        let scores: Vec<Result<f32>> = valid_exercises
            .into_iter()
            .map(|id| self.get_exercise_score(&id))
            .collect();
        scores
            .into_iter()
            .all(|score| score.is_ok() && score.unwrap() > 0.0)
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use lazy_static::lazy_static;
    use std::collections::BTreeMap;
    use ustr::Ustr;

    use crate::{
        blacklist::Blacklist,
        data::{MasteryScore, SchedulerOptions},
        scheduler::{cache::CachedScore, ExerciseScheduler, ScoreCache},
        testutil::*,
    };

    static NUM_EXERCISES: usize = 2;

    lazy_static! {
        /// A simple set of courses to test the basic functionality of Trane.
        static ref TEST_LIBRARY: Vec<TestCourse> = vec![
            TestCourse {
                id: TestId(0, None, None),
                dependencies: vec![],
                superseded: vec![],
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
                        superseded: vec![],
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
                        superseded: vec![],
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

    /// Verifies that scores are correctly invalidated.
    #[test]
    fn invalidate_cached_scores() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let library = init_test_simulation(&temp_dir.path(), &TEST_LIBRARY)?;
        let scheduler_data = library.get_scheduler_data();
        let cache = ScoreCache::new(scheduler_data, SchedulerOptions::default());

        // Insert some scores into the exercise and lesson caches.
        cache.exercise_cache.borrow_mut().insert(
            Ustr::from("a"),
            CachedScore {
                score: 5.0,
                num_trials: 1,
            },
        );
        cache.exercise_cache.borrow_mut().insert(
            Ustr::from("b::a"),
            CachedScore {
                score: 5.0,
                num_trials: 1,
            },
        );
        cache
            .lesson_cache
            .borrow_mut()
            .insert(Ustr::from("a::a"), Some(5.0));
        cache
            .lesson_cache
            .borrow_mut()
            .insert(Ustr::from("c::a"), Some(5.0));

        // Verify that the scores are present.
        assert_eq!(cache.get_exercise_score(&Ustr::from("a"))?, 5.0);
        assert_eq!(cache.get_exercise_score(&Ustr::from("b::a"))?, 5.0);
        assert_eq!(cache.get_lesson_score(&Ustr::from("a::a"))?, Some(5.0));
        assert_eq!(cache.get_lesson_score(&Ustr::from("c::a"))?, Some(5.0));

        // Invalidate prefix `a` and verify that the cached scores are removed.
        cache.invalidate_cached_scores_with_prefix("a");
        assert_eq!(cache.get_exercise_score(&Ustr::from("a"))?, 0.0);
        assert_eq!(cache.get_exercise_score(&Ustr::from("b::a"))?, 5.0);
        assert_eq!(cache.get_lesson_score(&Ustr::from("a::a"))?, None);
        assert_eq!(cache.get_lesson_score(&Ustr::from("c::a"))?, Some(5.0));

        // Invalidate units `b::a  and `c::a` and verify that the score is removed.
        cache.invalidate_cached_score(&Ustr::from("b::a"));
        cache.invalidate_cached_score(&Ustr::from("c::a"));
        assert_eq!(cache.get_exercise_score(&Ustr::from("b::a"))?, 0.0);
        assert_eq!(cache.get_lesson_score(&Ustr::from("c::a"))?, None);
        Ok(())
    }

    /// Verifies that the number of trials are cached along the exercise scores.
    #[test]
    fn get_num_trials() -> Result<()> {
        // Create a test library and send some scores.
        let temp_dir = tempfile::tempdir()?;
        let library = init_test_simulation(&temp_dir.path(), &TEST_LIBRARY)?;
        let scheduler_data = library.get_scheduler_data();
        let cache = ScoreCache::new(scheduler_data, SchedulerOptions::default());
        let exercise_id = Ustr::from("0::0::0");
        library.score_exercise(&exercise_id, MasteryScore::Four, 1)?;
        library.score_exercise(&exercise_id, MasteryScore::Five, 2)?;

        // Retrieve the number of trials twice. The second time should hit the cache.
        assert_eq!(Some(2), cache.get_num_trials(&exercise_id)?);
        assert_eq!(Some(2), cache.get_num_trials(&exercise_id)?);

        // Add another score and invalidate the cache. The change in the number of trials should be
        // reflected.
        library.score_exercise(&exercise_id, MasteryScore::Four, 3)?;
        cache.invalidate_cached_score(&exercise_id);
        assert_eq!(Some(3), cache.get_num_trials(&exercise_id)?);
        Ok(())
    }
}
