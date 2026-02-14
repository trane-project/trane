//@<lp-example-2
//! Defines the system used to retrieve scores and rewards for units and come up with a final score.
//!
//! During performance testing, it was found that caching scores scores significantly improved the
//! performance of exercise scheduling.
//>@lp-example-2

use anyhow::{Result, anyhow};
use std::cell::RefCell;
use ustr::{Ustr, UstrMap, UstrSet};

use crate::{
    data::{SchedulerOptions, UnitType},
    exercise_scorer::{ExerciseScorer, PowerLawScorer},
    reward_scorer::{RewardScorer, WeightedRewardScorer},
    scheduler::SchedulerData,
};

/// Stores information about a cached score.
#[derive(Clone)]
pub(super) struct CachedScore {
    /// The computed score.
    score: f32,

    /// The number of trials used to compute the score.
    num_trials: usize,
}

/// Contains the logic to score units based on their previous scores and rewards, as well as the
/// logic to cache those scores for efficiency.
pub(super) struct UnitScorer {
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

    /// The object used to compute the score of an exercise based on its previous trials.
    exercise_scorer: Box<dyn ExerciseScorer + Send + Sync>,

    /// The object used to compute the reward of a unit based on its previous rewards.
    reward_scorer: Box<dyn RewardScorer + Send + Sync>,
}

impl UnitScorer {
    /// Constructs a new score cache.
    pub(super) fn new(data: SchedulerData, options: SchedulerOptions) -> Self {
        Self {
            exercise_cache: RefCell::new(UstrMap::default()),
            lesson_cache: RefCell::new(UstrMap::default()),
            course_cache: RefCell::new(UstrMap::default()),
            data,
            options,
            exercise_scorer: Box::new(PowerLawScorer {}),
            reward_scorer: Box::new(WeightedRewardScorer {}),
        }
    }

    /// Removes the cached score for the given unit and all units affected by an update to its
    /// score.
    pub(super) fn invalidate_cached_score(&self, unit_id: Ustr) {
        // Remove the unit itself from the cache.
        let is_exercise = self.exercise_cache.borrow_mut().remove(&unit_id).is_some();
        let is_lesson = self.lesson_cache.borrow_mut().remove(&unit_id).is_some();
        let is_course = self.course_cache.borrow_mut().remove(&unit_id).is_some();

        // If the unit is an exercise, invalidate the cached score of its lesson and course. If the
        // unit is a lesson, invalidate the cached score of its course.
        if is_exercise
            && let Some(lesson_id) = self.data.unit_graph.read().get_exercise_lesson(unit_id)
        {
            self.lesson_cache.borrow_mut().remove(&lesson_id);
            if let Some(course_id) = self.data.unit_graph.read().get_lesson_course(lesson_id) {
                self.course_cache.borrow_mut().remove(&course_id);
            }
        }

        // Invalidate the scores of all exercises in the lesson.
        if is_lesson {
            let exercises = self.data.unit_graph.read().get_lesson_exercises(unit_id);
            if let Some(exercise_ids) = exercises {
                for exercise_id in exercise_ids {
                    self.exercise_cache.borrow_mut().remove(&exercise_id);
                }
            }
        }

        // Invalidate the scores of all lessons in the course and all exercises in those
        // lessons.
        if is_course {
            let lessons = self.data.unit_graph.read().get_course_lessons(unit_id);
            if let Some(lesson_ids) = lessons {
                for lesson_id in lesson_ids {
                    self.lesson_cache.borrow_mut().remove(&lesson_id);
                    let exercises = self.data.unit_graph.read().get_lesson_exercises(lesson_id);
                    if let Some(exercise_ids) = exercises {
                        for exercise_id in exercise_ids {
                            self.exercise_cache.borrow_mut().remove(&exercise_id);
                        }
                    }
                }
            }
        }
    }

    /// Removes the cached score for any unit with the given prefix.
    pub(super) fn invalidate_cached_scores_with_prefix(&self, prefix: &str) {
        // Remove the unit from the exercise, lesson, and course caches. This is safe to do even
        // though the unit is at most in one cache because the caches are disjoint.
        self.exercise_cache
            .borrow_mut()
            .retain(|unit_id, _| !unit_id.starts_with(prefix));
        self.lesson_cache
            .borrow_mut()
            .retain(|unit_id, _| !unit_id.starts_with(prefix));
        self.course_cache
            .borrow_mut()
            .retain(|unit_id, _| !unit_id.starts_with(prefix));
    }

    /// Returns the score for the given exercise.
    fn get_exercise_score(&self, exercise_id: Ustr) -> Result<f32> {
        // Return the cached score if it exists.
        let cached_score = self.exercise_cache.borrow().get(&exercise_id).cloned();
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
        let score = self.exercise_scorer.score(&scores)?;

        // Retrieve the rewards for this exercise's lesson and course and compute the reward.
        let lesson_id = self
            .data
            .unit_graph
            .read()
            .get_exercise_lesson(exercise_id)
            .unwrap_or_default();
        let lesson_rewards = self
            .data
            .practice_rewards
            .read()
            .get_rewards(lesson_id, self.options.num_rewards)
            .unwrap_or_default();
        let course_id = self
            .data
            .unit_graph
            .read()
            .get_lesson_course(lesson_id)
            .unwrap_or_default();
        let course_rewards = self
            .data
            .practice_rewards
            .read()
            .get_rewards(course_id, self.options.num_rewards)
            .unwrap_or_default();
        let reward = self
            .reward_scorer
            .score_rewards(&course_rewards, &lesson_rewards)
            .unwrap_or_default();

        // The final score is the sum of the score and the reward. Do not add a reward for exercises
        // with no previous scores.
        let final_score = if scores.is_empty() {
            score
        } else {
            (score + reward).clamp(0.0, 5.0)
        };
        self.exercise_cache.borrow_mut().insert(
            exercise_id,
            CachedScore {
                score: final_score,
                num_trials: scores.len(),
            },
        );
        Ok(final_score)
    }

    /// Returns the number of trials that were considered when computing the score for the given
    /// exercise.
    pub(super) fn get_num_trials(&self, exercise_id: Ustr) -> Result<Option<usize>> {
        // Return the cached value if it exists.
        let cached_score = self.exercise_cache.borrow().get(&exercise_id).cloned();
        if let Some(cached_score) = cached_score {
            return Ok(Some(cached_score.num_trials));
        }

        // Compute the exercise's score, which populates the cache. Then, retrieve the number of
        // trials from the cache.
        self.get_exercise_score(exercise_id)?;
        let cached_score = self.exercise_cache.borrow().get(&exercise_id).cloned();
        Ok(cached_score.map(|s| s.num_trials))
    }

    /// Returns whether all the exercises in the unit have valid scores.
    pub(super) fn all_valid_exercises_have_scores(&self, unit_id: Ustr) -> bool {
        // Get all the valid exercises in the unit.
        let valid_exercises = self.data.all_valid_exercises(unit_id);
        if valid_exercises.is_empty() {
            return true;
        }

        // All valid exercises must have a score greater than 0.0.
        let scores: Vec<Result<f32>> = valid_exercises
            .into_iter()
            .map(|id| self.get_exercise_score(id))
            .collect();
        scores
            .into_iter()
            .all(|score| score.is_ok() && score.unwrap() > 0.0)
    }

    /// Returns whether the superseded unit can be considered as superseded by the superseding
    /// units.
    pub(super) fn is_superseded(&self, superseded_id: Ustr, superseding_ids: &UstrSet) -> bool {
        // Units with no superseding units are not superseded.
        if superseding_ids.is_empty() {
            return false;
        }

        // All the exercises from the superseded unit must have been seen at least once.
        if !self.all_valid_exercises_have_scores(superseded_id) {
            return false;
        }

        // All the superseding units must have a score equal or greater than the superseding score.
        let scores = superseding_ids
            .iter()
            .filter_map(|id| self.get_unit_score(*id).unwrap_or_default())
            .collect::<Vec<_>>();
        scores
            .iter()
            .all(|score| *score >= self.data.options.superseding_score)
    }

    /// Recursively check if each superseding unit has itself been superseded by another unit and
    /// replace them from the original set with those units.
    fn replace_superseding(&self, superseding_ids: UstrSet) -> UstrSet {
        let mut result = UstrSet::default();
        for id in superseding_ids {
            let superseding = self.data.get_superseding(id);
            if let Some(superseding) = superseding {
                // The unit has some superseding units of its own. If the unit has been superseded
                // by them, recursively call this function. Otherwise, add the unit to the result.
                if self.is_superseded(id, &superseding) {
                    result.extend(self.replace_superseding(superseding));
                } else {
                    result.insert(id);
                }
            } else {
                // The unit has no superseding units, so add it to the result.
                result.insert(id);
            }
        }
        result
    }

    /// Get the initial superseding units and then recursively replace them if they have been
    /// superseded.
    pub(super) fn get_superseding_recursive(&self, unit_id: Ustr) -> Option<UstrSet> {
        let superseding_ids = self.data.get_superseding(unit_id);
        superseding_ids.map(|ids| self.replace_superseding(ids))
    }

    /// Returns the average score of all the exercises in the given lesson.
    fn get_lesson_score(&self, lesson_id: Ustr) -> Result<Option<f32>> {
        // Return the cached score if it exists.
        let cached_score = self.lesson_cache.borrow().get(&lesson_id).copied();
        if let Some(score) = cached_score {
            return Ok(score);
        }

        // Check if the unit is blacklisted. A blacklisted unit has no score.
        let blacklist = self.data.blacklist.read();
        let blacklisted = blacklist.blacklisted(lesson_id);
        if blacklisted.unwrap_or(false) {
            self.lesson_cache.borrow_mut().insert(lesson_id, None);
            return Ok(None);
        }

        // Check if the lesson has been superseded. Superseded lessons have no score.
        let superseding_ids = self.get_superseding_recursive(lesson_id);
        if let Some(superseding_ids) = superseding_ids
            && self.is_superseded(lesson_id, &superseding_ids)
        {
            self.lesson_cache.borrow_mut().insert(lesson_id, None);
            return Ok(None);
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
                        let blacklisted = blacklist.blacklisted(*exercise_id);
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
                        .map(|id| self.get_exercise_score(*id))
                        .sum::<Result<f32>>()?
                        / valid_exercises.len() as f32;
                    Ok(Some(avg_score))
                }
            }
        };

        // Update the cache with a valid score.
        if let Ok(score) = score {
            self.lesson_cache.borrow_mut().insert(lesson_id, score);
        }
        score
    }

    /// Returns the average score of all the lesson scores in the given course.
    fn get_course_score(&self, course_id: Ustr) -> Result<Option<f32>> {
        // Return the cached score if it exists.
        let cached_score = self.course_cache.borrow().get(&course_id).copied();
        if let Some(score) = cached_score {
            return Ok(score);
        }

        // Check if the unit is blacklisted. A blacklisted course has no valid score.
        let blacklisted = self.data.blacklist.read().blacklisted(course_id);
        if blacklisted.unwrap_or(false) {
            self.course_cache.borrow_mut().insert(course_id, None);
            return Ok(None);
        }

        // Check if the course has been superseded. Superseded courses have no score.
        let superseding_ids = self.get_superseding_recursive(course_id);
        if let Some(superseding_ids) = superseding_ids
            && self.is_superseded(course_id, &superseding_ids)
        {
            self.course_cache.borrow_mut().insert(course_id, None);
            return Ok(None);
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
                    .map(|lesson_id| self.get_lesson_score(lesson_id))
                    .filter(|score| {
                        // Filter out any lesson whose score is not valid.
                        if score.as_ref().unwrap_or(&None).is_none() {
                            return false;
                        }
                        true
                    })
                    .collect::<Result<Vec<_>>>()?;

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
        if let Ok(score) = score {
            self.course_cache.borrow_mut().insert(course_id, score);
        }
        score
    }

    /// Returns the score for the given unit. A return value of `Ok(None)` indicates that there is
    /// not a valid score for the unit, such as when the unit is blacklisted. Such a unit is
    /// considered a satisfied dependency.
    pub(super) fn get_unit_score(&self, unit_id: Ustr) -> Result<Option<f32>> {
        // Decide which method to call based on the unit type.
        let unit_type = self
            .data
            .unit_graph
            .read()
            .get_unit_type(unit_id)
            .ok_or(anyhow!("missing unit type for unit with ID {unit_id}"))?;
        match unit_type {
            UnitType::Course => self.get_course_score(unit_id),
            UnitType::Lesson => self.get_lesson_score(unit_id),
            UnitType::Exercise => self.get_exercise_score(unit_id).map(Some),
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use anyhow::Result;
    use chrono::Utc;
    use std::{collections::BTreeMap, sync::LazyLock};
    use ustr::Ustr;

    use crate::{
        blacklist::Blacklist,
        data::{MasteryScore, SchedulerOptions},
        scheduler::{ExerciseScheduler, UnitScorer, unit_scorer::CachedScore},
        test_utils::*,
    };

    static NUM_EXERCISES: usize = 2;

    /// A simple set of courses to test the basic functionality of Trane.
    static TEST_LIBRARY: LazyLock<Vec<TestCourse>> = LazyLock::new(|| {
        vec![
            TestCourse {
                id: TestId(0, None, None),
                dependencies: vec![],
                superseded: vec![],
                metadata: BTreeMap::default(),
                lessons: vec![
                    TestLesson {
                        id: TestId(0, Some(0), None),
                        dependencies: vec![],
                        superseded: vec![],
                        metadata: BTreeMap::default(),
                        num_exercises: NUM_EXERCISES,
                    },
                    TestLesson {
                        id: TestId(0, Some(1), None),
                        dependencies: vec![TestId(0, Some(0), None)],
                        superseded: vec![],
                        metadata: BTreeMap::default(),
                        num_exercises: NUM_EXERCISES,
                    },
                ],
            },
            TestCourse {
                id: TestId(1, None, None),
                dependencies: vec![TestId(0, None, None)],
                superseded: vec![TestId(0, None, None)],
                metadata: BTreeMap::default(),
                lessons: vec![
                    TestLesson {
                        id: TestId(1, Some(0), None),
                        dependencies: vec![],
                        superseded: vec![],
                        metadata: BTreeMap::default(),
                        num_exercises: NUM_EXERCISES,
                    },
                    TestLesson {
                        id: TestId(1, Some(1), None),
                        dependencies: vec![TestId(1, Some(0), None)],
                        superseded: vec![TestId(1, Some(0), None)],
                        metadata: BTreeMap::default(),
                        num_exercises: NUM_EXERCISES,
                    },
                ],
            },
        ]
    });

    /// Verifies that a score of `None` is returned for a blacklisted course.
    #[test]
    fn blacklisted_course_score() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let mut library = init_test_simulation(temp_dir.path(), &TEST_LIBRARY)?;
        let scheduler_data = library.get_scheduler_data();
        let cache = UnitScorer::new(scheduler_data, SchedulerOptions::default());

        let course_id = Ustr::from("0");
        library.add_to_blacklist(course_id)?;
        assert_eq!(cache.get_course_score(course_id)?, None);
        Ok(())
    }

    /// Verifies that the score of a superseded course is None and is correctly cached.
    #[test]
    fn superseded_course_cached() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let library = init_test_simulation(temp_dir.path(), &TEST_LIBRARY)?;
        let scheduler_data = library.get_scheduler_data();
        let cache = UnitScorer::new(scheduler_data, SchedulerOptions::default());

        // Insert scores for every exercise to ensure course 0 has been superseded.
        let ts = Utc::now().timestamp();
        library.score_exercise(Ustr::from("0::0::0"), MasteryScore::Five, ts)?;
        library.score_exercise(Ustr::from("0::0::1"), MasteryScore::Five, ts)?;
        library.score_exercise(Ustr::from("0::1::0"), MasteryScore::Five, ts)?;
        library.score_exercise(Ustr::from("0::1::1"), MasteryScore::Five, ts)?;
        library.score_exercise(Ustr::from("1::0::0"), MasteryScore::Five, ts)?;
        library.score_exercise(Ustr::from("1::0::1"), MasteryScore::Five, ts)?;
        library.score_exercise(Ustr::from("1::1::0"), MasteryScore::Five, ts)?;
        library.score_exercise(Ustr::from("1::1::1"), MasteryScore::Five, ts)?;

        // Get the scores for course 0 twice. Once to populate the cache and once to retrieve the
        // cached value.
        assert_eq!(cache.get_course_score(Ustr::from("0"))?, None);
        assert_eq!(cache.get_course_score(Ustr::from("0"))?, None);
        Ok(())
    }

    /// Verifies that the score of a superseded lesson is None and is correctly cached.
    #[test]
    fn superseded_course_lesson_cached() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let library = init_test_simulation(temp_dir.path(), &TEST_LIBRARY)?;
        let scheduler_data = library.get_scheduler_data();
        let cache = UnitScorer::new(scheduler_data, SchedulerOptions::default());

        // Insert scores for every exercise to ensure lesson 1::0 has been superseded.
        let ts = Utc::now().timestamp();
        library.score_exercise(Ustr::from("0::0::0"), MasteryScore::Five, ts)?;
        library.score_exercise(Ustr::from("0::0::1"), MasteryScore::Five, ts)?;
        library.score_exercise(Ustr::from("0::1::0"), MasteryScore::Five, ts)?;
        library.score_exercise(Ustr::from("0::1::1"), MasteryScore::Five, ts)?;
        library.score_exercise(Ustr::from("1::0::0"), MasteryScore::Five, ts)?;
        library.score_exercise(Ustr::from("1::0::1"), MasteryScore::Five, ts)?;
        library.score_exercise(Ustr::from("1::1::0"), MasteryScore::Five, ts)?;
        library.score_exercise(Ustr::from("1::1::1"), MasteryScore::Five, ts)?;

        // Get the scores for lesson 1::0 twice. Once to populate the cache and once to retrieve the
        // cached value.
        assert_eq!(cache.get_lesson_score(Ustr::from("1::0"))?, None);
        assert_eq!(cache.get_lesson_score(Ustr::from("1::0"))?, None);
        Ok(())
    }

    /// Verifies that scores are correctly invalidated.
    #[test]
    fn invalidate_cached_scores() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let library = init_test_simulation(temp_dir.path(), &TEST_LIBRARY)?;
        let scheduler_data = library.get_scheduler_data();
        let cache = UnitScorer::new(scheduler_data, SchedulerOptions::default());

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
        assert_eq!(cache.get_exercise_score(Ustr::from("a"))?, 5.0);
        assert_eq!(cache.get_exercise_score(Ustr::from("b::a"))?, 5.0);
        assert_eq!(cache.get_lesson_score(Ustr::from("a::a"))?, Some(5.0));
        assert_eq!(cache.get_lesson_score(Ustr::from("c::a"))?, Some(5.0));

        // Invalidate prefix `a` and verify that the cached scores are removed.
        cache.invalidate_cached_scores_with_prefix("a");
        assert_eq!(cache.get_exercise_score(Ustr::from("a"))?, 0.0);
        assert_eq!(cache.get_exercise_score(Ustr::from("b::a"))?, 5.0);
        assert_eq!(cache.get_lesson_score(Ustr::from("a::a"))?, None);
        assert_eq!(cache.get_lesson_score(Ustr::from("c::a"))?, Some(5.0));

        // Invalidate units `b::a  and `c::a` and verify that the score is removed.
        cache.invalidate_cached_score(Ustr::from("b::a"));
        cache.invalidate_cached_score(Ustr::from("c::a"));
        assert_eq!(cache.get_exercise_score(Ustr::from("b::a"))?, 0.0);
        assert_eq!(cache.get_lesson_score(Ustr::from("c::a"))?, None);
        Ok(())
    }

    /// Verifies that the number of trials are cached along the exercise scores.
    #[test]
    fn get_num_trials() -> Result<()> {
        // Create a test library and send some scores.
        let temp_dir = tempfile::tempdir()?;
        let library = init_test_simulation(temp_dir.path(), &TEST_LIBRARY)?;
        let scheduler_data = library.get_scheduler_data();
        let cache = UnitScorer::new(scheduler_data, SchedulerOptions::default());
        let exercise_id = Ustr::from("0::0::0");
        library.score_exercise(exercise_id, MasteryScore::Four, 1)?;
        library.score_exercise(exercise_id, MasteryScore::Five, 2)?;

        // Retrieve the number of trials twice. The second time should hit the cache.
        assert_eq!(Some(2), cache.get_num_trials(exercise_id)?);
        assert_eq!(Some(2), cache.get_num_trials(exercise_id)?);

        // Add another score and invalidate the cache. The change in the number of trials should be
        // reflected.
        library.score_exercise(exercise_id, MasteryScore::Four, 3)?;
        cache.invalidate_cached_score(exercise_id);
        assert_eq!(Some(3), cache.get_num_trials(exercise_id)?);
        Ok(())
    }
}
