//! Module defining the data structures used to schedule batches of exercises to show to the user.
//! The core of Trane's logic is in this module.
mod cache;
pub mod data;
mod filter;

use anyhow::Result;
use rand::{seq::SliceRandom, thread_rng};
use std::collections::{HashMap, HashSet};

use crate::{
    data::{
        filter::{MetadataFilter, UnitFilter},
        ExerciseManifest, MasteryScore, SchedulerOptions, UnitType,
    },
    scheduler::{cache::ScoreCache, data::SchedulerData, filter::CandidateFilter},
};

/// The batch size will be multiplied by this factor in order to expand the range of the search and
/// avoid always returning the same exercises. A search concludes early if it reaches a dead-end and
/// there are already more candidates than said product.
const MAX_CANDIDATE_FACTOR: usize = 10;

/// Contains functions used to retrieve a new batch of exercises.
pub trait ExerciseScheduler {
    /// Gets a new batch of exercises scheduled for a new trial.
    fn get_exercise_batch(
        &self,
        filter: Option<&UnitFilter>,
    ) -> Result<Vec<(String, ExerciseManifest)>>;

    /// Records the score of the given exercise's trial.
    fn record_exercise_score(
        &self,
        exercise_id: &str,
        score: MasteryScore,
        timestamp: i64,
    ) -> Result<()>;
}

/// A struct representing an element in the stack used during the graph search.
#[derive(Clone, Debug)]
struct StackItem {
    /// The UID of the unit contained in the item.
    unit_uid: u64,

    /// The number of hops the search needed to reach this item.
    num_hops: usize,
}

/// A struct representing an exercise selected during the search.
#[derive(Clone, Debug)]
struct Candidate {
    /// The UID of the exercise.
    exercise_uid: u64,

    /// The number of hops the graph search needed to reach this exercise.
    num_hops: usize,

    /// The exercise score.
    score: f32,
}

/// An exercise scheduler based on depth-first search.
pub(crate) struct DepthFirstScheduler {
    /// The data used to schedule exercises.
    data: SchedulerData,

    /// A cache of unit uid to its score.
    score_cache: ScoreCache,
}

impl DepthFirstScheduler {
    /// Creates a new simple scheduler.
    pub fn new(data: SchedulerData, options: SchedulerOptions) -> Self {
        Self {
            data: data.clone(),
            score_cache: ScoreCache::new(data, options),
        }
    }

    /// Shuffles the units and pushes them to the given stack.
    fn shuffle_to_stack(curr_unit: &StackItem, mut units: Vec<u64>, stack: &mut Vec<StackItem>) {
        units.shuffle(&mut thread_rng());
        stack.extend(units.iter().map(|uid| StackItem {
            unit_uid: *uid,
            num_hops: curr_unit.num_hops + 1,
        }));
    }

    /// Returns all the courses without dependencies. If some of those courses are missing, their
    /// dependents are added until there are no missing courses.
    fn get_all_starting_courses(&self) -> HashSet<u64> {
        let mut starting_courses = self.data.unit_graph.borrow().get_dependency_sinks();
        let mut num_courses = starting_courses.len();

        // Replace any missing courses with their dependents and repeat this process until there are
        // no missing courses.
        loop {
            let mut new_starting_courses = HashSet::new();
            for course_uid in starting_courses {
                if self.data.unit_exists(course_uid).unwrap() {
                    new_starting_courses.insert(course_uid);
                } else {
                    new_starting_courses.extend(self.data.get_all_dependents(course_uid).iter());
                }
            }
            starting_courses = new_starting_courses;

            if starting_courses.len() == num_courses {
                break;
            }
            num_courses = starting_courses.len();
        }

        // Some of the courses added may have existing dependencies. Remove them.
        starting_courses
            .into_iter()
            .filter(|course_uid| {
                let dependencies = self
                    .data
                    .unit_graph
                    .borrow()
                    .get_dependencies(*course_uid)
                    .unwrap_or_default();
                if dependencies.is_empty() {
                    return true;
                }
                dependencies
                    .iter()
                    .all(|uid| !self.data.unit_exists(*uid).unwrap())
            })
            .collect()
    }

    /// Returns all the starting lessons in the graph.
    fn get_all_starting_lessons(&self) -> Vec<StackItem> {
        let starting_courses = self.get_all_starting_courses();
        let mut starting_lessons: Vec<StackItem> = vec![];
        for course_uid in starting_courses {
            let lesson_uids = self
                .data
                .get_course_starting_lessons(course_uid)
                .unwrap_or_default();
            starting_lessons.extend(lesson_uids.into_iter().map(|uid| StackItem {
                unit_uid: uid,
                num_hops: 0,
            }));
        }
        starting_lessons.shuffle(&mut thread_rng());
        starting_lessons
    }

    /// Gets a list of scores for the given exercises.
    fn get_exercise_scores(&self, exercises: &[u64]) -> Result<Vec<f32>> {
        exercises
            .iter()
            .map(|exercise_uid| self.score_cache.get_exercise_score(*exercise_uid))
            .collect()
    }

    /// Returns the list of candidates selected from the given lesson along with the average score.
    fn get_candidates_from_lesson_helper(&self, item: &StackItem) -> Result<(Vec<Candidate>, f32)> {
        // Check whether the lesson or its course have been blacklisted.
        if self.data.blacklisted_uid(item.unit_uid).unwrap_or(false) {
            return Ok((vec![], 0.0));
        }
        let course_id = self.data.get_lesson_course_id(item.unit_uid)?;
        if self.data.blacklisted_id(&course_id).unwrap_or(false) {
            return Ok((vec![], 0.0));
        }

        let exercises = self.data.get_lesson_exercises(item.unit_uid);
        let exercise_scores = self.get_exercise_scores(&exercises)?;
        let candidates = exercises
            .into_iter()
            .filter(|uid| !self.data.blacklisted_uid(*uid).unwrap_or(false))
            .zip(exercise_scores.iter())
            .map(|(uid, score)| Candidate {
                exercise_uid: uid,
                num_hops: item.num_hops + 1,
                score: *score,
            })
            .collect::<Vec<Candidate>>();

        let avg_score = if exercise_scores.is_empty() {
            0.0
        } else {
            exercise_scores.iter().sum::<f32>() / (exercise_scores.len() as f32)
        };

        Ok((candidates, avg_score))
    }

    /// Returns the valid dependents which can be visited after the given unit. A valid dependent is
    /// a unit whose full dependencies are met.
    fn get_valid_dependents(
        &self,
        unit_uid: u64,
        metadata_filter: Option<&MetadataFilter>,
    ) -> Vec<u64> {
        let dependents = self.data.get_all_dependents(unit_uid);
        if dependents.is_empty() {
            return dependents;
        }

        // Any error during the filtering is ignored for the purpose of allowing the search to
        // continue if parts of the dependency graph are missing.
        dependents
            .into_iter()
            .filter(|uid| {
                let exists = self.data.unit_exists(*uid);
                if exists.is_err() {
                    return true;
                }

                let dependencies = self
                    .data
                    .unit_graph
                    .borrow()
                    .get_dependencies(*uid)
                    .unwrap_or_default();
                if dependencies.is_empty() {
                    return true;
                }

                let num_dependencies = dependencies.len();
                let met_dependencies = dependencies.into_iter().filter(|dep_uid| {
                    let passes_filter = self
                        .data
                        .unit_passes_filter(*dep_uid, metadata_filter)
                        .unwrap_or(false);
                    if !passes_filter {
                        return true;
                    }

                    let blacklisted = self.data.blacklisted_uid(*dep_uid);
                    if blacklisted.is_err() || blacklisted.unwrap() {
                        return true;
                    }

                    let course_id = self.data.get_lesson_course_id(*dep_uid).unwrap_or_default();
                    if self.data.blacklisted_id(&course_id).unwrap_or(false) {
                        return true;
                    }

                    let score = self.score_cache.get_unit_score(*dep_uid);
                    if score.is_err() || score.as_ref().unwrap().is_none() {
                        return true;
                    }
                    let avg_score = score.unwrap().unwrap();
                    avg_score >= self.data.options.passing_score
                });
                met_dependencies.count() == num_dependencies
            })
            .collect()
    }

    /// Searches for candidates across the entire graph.
    fn get_candidates_from_graph(
        &self,
        metadata_filter: Option<&MetadataFilter>,
    ) -> Result<Vec<Candidate>> {
        let mut stack: Vec<StackItem> = Vec::new();
        let starting_courses = self.get_all_starting_lessons();
        stack.extend(starting_courses.into_iter());

        let max_candidates = self.data.options.batch_size * MAX_CANDIDATE_FACTOR;
        let mut all_candidates: Vec<Candidate> = Vec::new();
        let mut visited: HashSet<u64> = HashSet::new();
        // Keeps track of the number of lessons that have been visited for each course. The search
        // will move onto the dependents of a course if all of its lessons have been visited.
        let mut pending_course_lessons: HashMap<u64, i64> = HashMap::new();

        while !stack.is_empty() {
            let curr_unit = stack.pop().unwrap();
            if visited.contains(&curr_unit.unit_uid) {
                continue;
            }

            let exists = self.data.unit_exists(curr_unit.unit_uid);
            if exists.is_err() || !exists.unwrap() {
                // Try to add the valid dependents of any unit which cannot be found so that missing
                // sections of the graph do not stop the search.
                visited.insert(curr_unit.unit_uid);
                let valid_deps = self.get_valid_dependents(curr_unit.unit_uid, metadata_filter);
                Self::shuffle_to_stack(&curr_unit, valid_deps, &mut stack);
                continue;
            }

            let unit_type = self.data.get_unit_type(curr_unit.unit_uid)?;
            if unit_type == UnitType::Course {
                if visited.contains(&curr_unit.unit_uid) {
                    continue;
                }

                let starting_lessons: Vec<u64> = self
                    .data
                    .get_course_starting_lessons(curr_unit.unit_uid)
                    .unwrap_or_default();
                Self::shuffle_to_stack(&curr_unit, starting_lessons, &mut stack);

                // Update the count of pending lessons. The course depends on each of its lessons
                // but the search only moves forward once all of its lessons have been visited.
                let pending_lessons = pending_course_lessons
                    .entry(curr_unit.unit_uid)
                    .or_insert_with(|| self.data.get_num_lessons_in_course(curr_unit.unit_uid));
                let passes_filter = self
                    .data
                    .unit_passes_filter(curr_unit.unit_uid, metadata_filter)
                    .unwrap_or(true);

                if *pending_lessons <= 0
                    || !passes_filter
                    || self
                        .data
                        .blacklisted_uid(curr_unit.unit_uid)
                        .unwrap_or(false)
                {
                    // There are no pending lessons, the course does not pass the metadata filter,
                    // or the unit is blacklisted. Push its valid dependents onto the stack.
                    visited.insert(curr_unit.unit_uid);
                    let valid_deps = self.get_valid_dependents(curr_unit.unit_uid, metadata_filter);
                    Self::shuffle_to_stack(&curr_unit, valid_deps, &mut stack);
                    continue;
                }

                // The course has pending lessons so do not mark it as visited. Simply continue with
                // the search.
                continue;
            }

            if unit_type == UnitType::Exercise {
                // The search only considers lessons and courses. Any exercise encountered by
                // mistake is ignored.
                continue;
            }

            // If the searched reached this point, the unit must be a lesson.
            visited.insert(curr_unit.unit_uid);

            // Update the number of lessons processed in the course.
            let course_uid = self.data.get_course_uid(curr_unit.unit_uid)?;
            let pending_lessons = pending_course_lessons
                .entry(course_uid)
                .or_insert_with(|| self.data.get_num_lessons_in_course(course_uid));
            *pending_lessons -= 1;
            if *pending_lessons <= 0 {
                // Once all of the lessons in the course have been visited, re-add the course to the
                // stack so the search can continue exploring its dependents.
                stack.push(StackItem {
                    unit_uid: course_uid,
                    num_hops: curr_unit.num_hops + 1,
                });
            }

            let valid_deps = self.get_valid_dependents(curr_unit.unit_uid, metadata_filter);
            let passes_filter = self
                .data
                .unit_passes_filter(curr_unit.unit_uid, metadata_filter)
                .unwrap_or(true);
            if !passes_filter {
                Self::shuffle_to_stack(&curr_unit, valid_deps, &mut stack);
                continue;
            }

            let (candidates, avg_score) = self.get_candidates_from_lesson_helper(&curr_unit)?;
            let num_candidates = candidates.len();
            all_candidates.extend(candidates);

            if num_candidates > 0 && avg_score < self.data.options.passing_score {
                // If the search reaches a dead-end and there are already enough candidates,
                // terminate the search. Otherwise, continue with the search.
                if all_candidates.len() >= max_candidates {
                    break;
                }
                continue;
            }

            Self::shuffle_to_stack(&curr_unit, valid_deps, &mut stack);
        }

        Ok(all_candidates)
    }

    /// Searches for candidates from the given course.
    fn get_candidates_from_course(&self, course_id: &str) -> Result<Vec<Candidate>> {
        let course_uid = self.data.get_uid(course_id)?;
        let starting_lessons = self
            .data
            .get_course_starting_lessons(course_uid)
            .unwrap_or_default();
        let mut stack: Vec<StackItem> = starting_lessons
            .into_iter()
            .map(|uid| StackItem {
                unit_uid: uid,
                num_hops: 0,
            })
            .collect();

        let max_candidates = self.data.options.batch_size * MAX_CANDIDATE_FACTOR;
        let mut all_candidates: Vec<Candidate> = Vec::new();
        let mut visited: HashSet<u64> = HashSet::new();
        visited.insert(course_uid);

        while !stack.is_empty() {
            let curr_unit = stack.pop().unwrap();
            if visited.contains(&curr_unit.unit_uid) {
                continue;
            } else {
                visited.insert(curr_unit.unit_uid);
            }

            let exists = self.data.unit_exists(curr_unit.unit_uid);
            if exists.is_err() || !exists.unwrap() {
                // Try to add the valid dependents of any unit which cannot be found.
                let valid_deps = self.get_valid_dependents(curr_unit.unit_uid, None);
                Self::shuffle_to_stack(&curr_unit, valid_deps, &mut stack);
                continue;
            }

            let unit_type = self.data.get_unit_type(curr_unit.unit_uid)?;
            if unit_type == UnitType::Course {
                // Skip any courses, as only exercises from the given course should be considered.
                continue;
            }

            if unit_type == UnitType::Exercise {
                // The search only considers lessons and courses. Any exercise encountered by
                // mistake is ignored.
                continue;
            }

            // If the searched reached this point, the unit must be a lesson.
            let lesson_course_id = self.data.get_lesson_course_id(curr_unit.unit_uid);
            if lesson_course_id.is_err() {
                continue;
            }
            if lesson_course_id.unwrap() != course_id {
                continue;
            }

            let (candidates, avg_score) = self.get_candidates_from_lesson_helper(&curr_unit)?;
            let num_candidates = candidates.len();
            all_candidates.extend(candidates);

            if num_candidates > 0 && avg_score < self.data.options.passing_score {
                if all_candidates.len() >= max_candidates {
                    break;
                }
                continue;
            }

            let valid_deps = self.get_valid_dependents(curr_unit.unit_uid, None);
            Self::shuffle_to_stack(&curr_unit, valid_deps, &mut stack);
        }

        Ok(all_candidates)
    }

    /// Searches for candidates from the given lesson.
    fn get_candidates_from_lesson(&self, lesson_id: &str) -> Result<Vec<Candidate>> {
        let lesson_uid = self.data.get_uid(lesson_id)?;
        let (candidates, _) = self.get_candidates_from_lesson_helper(&StackItem {
            unit_uid: lesson_uid,
            num_hops: 0,
        })?;
        Ok(candidates)
    }
}

impl ExerciseScheduler for DepthFirstScheduler {
    fn get_exercise_batch(
        &self,
        filter: Option<&UnitFilter>,
    ) -> Result<Vec<(String, ExerciseManifest)>> {
        let candidates = match filter {
            None => self.get_candidates_from_graph(None)?,
            Some(filter) => match filter {
                UnitFilter::CourseFilter { course_ids } => {
                    let mut candidates = Vec::new();
                    for course_id in course_ids {
                        candidates.extend(self.get_candidates_from_course(course_id)?.into_iter());
                    }
                    candidates
                }
                UnitFilter::LessonFilter { lesson_ids } => {
                    let mut candidates = Vec::new();
                    for lesson_id in lesson_ids {
                        candidates.extend(self.get_candidates_from_lesson(lesson_id)?.into_iter());
                    }
                    candidates
                }
                UnitFilter::MetadataFilter { filter } => {
                    self.get_candidates_from_graph(Some(filter))?
                }
            },
        };

        let filter = CandidateFilter::new(self.data.clone());
        filter.filter_candidates(candidates)
    }

    fn record_exercise_score(
        &self,
        exercise_id: &str,
        score: MasteryScore,
        timestamp: i64,
    ) -> Result<()> {
        let exercise_uid = self.data.get_uid(exercise_id)?;
        self.data
            .practice_stats
            .borrow_mut()
            .record_exercise_score(exercise_id, score, timestamp)?;
        self.score_cache.invalidate_cached_score(exercise_uid);
        Ok(())
    }
}
