//! Module defining the data structures used to schedule batches of exercises to show to the user.
//! The core of Trane's logic is in this module.
mod cache;
pub mod data;
mod filter;

use anyhow::Result;
use rand::{seq::SliceRandom, thread_rng};
use std::collections::{HashMap, HashSet};
use ustr::Ustr;

use crate::{
    data::{
        filter::{MetadataFilter, UnitFilter},
        ExerciseManifest, MasteryScore, SchedulerOptions, UnitType,
    },
    graph::UnitGraph,
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
    ) -> Result<Vec<(Ustr, ExerciseManifest)>>;

    /// Records the score of the given exercise's trial.
    fn score_exercise(&self, exercise_id: &Ustr, score: MasteryScore, timestamp: i64)
        -> Result<()>;
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
        // Replace any missing courses with their dependents and repeat this process until there are
        // no missing courses.
        let mut starting_courses = self.data.unit_graph.read().get_dependency_sinks();
        loop {
            let mut new_starting_courses = HashSet::new();
            for course_uid in &starting_courses {
                if self.data.unit_exists(*course_uid).unwrap_or(false) {
                    new_starting_courses.insert(*course_uid);
                } else {
                    new_starting_courses.extend(self.data.get_all_dependents(*course_uid).iter());
                }
            }
            if new_starting_courses.len() == starting_courses.len() {
                break;
            }
            starting_courses = new_starting_courses;
        }

        // Remove all courses with existing dependencies.
        starting_courses
            .into_iter()
            .filter(|course_uid| {
                self.data
                    .unit_graph
                    .read()
                    .get_dependencies(*course_uid)
                    .unwrap_or_default()
                    .iter()
                    .all(|uid| !self.data.unit_exists(*uid).unwrap())
            })
            .collect()
    }

    /// Returns the lessons in the course that have no dependencies with other lessons in the course
    /// and whose dependencies are satisfied.
    pub fn get_course_starting_lessons(
        &self,
        course_uid: u64,
        metadata_filter: Option<&MetadataFilter>,
    ) -> Result<Vec<u64>> {
        Ok(self
            .data
            .unit_graph
            .read()
            .get_course_starting_lessons(course_uid)
            .unwrap_or_default()
            .into_iter()
            .filter(|uid| self.all_satisfied_dependencies(*uid, metadata_filter))
            .collect())
    }

    /// Returns all the starting lessons in the graph.
    fn get_all_starting_lessons(&self, metadata_filter: Option<&MetadataFilter>) -> Vec<StackItem> {
        let starting_courses = self.get_all_starting_courses();
        let mut starting_lessons: Vec<StackItem> = vec![];
        for course_uid in starting_courses {
            let lesson_uids = self
                .get_course_starting_lessons(course_uid, metadata_filter)
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

        // Generate a list of candidates from the lesson's exercises.
        let exercises = self.data.get_lesson_exercises(item.unit_uid);
        let exercise_scores = self.get_exercise_scores(&exercises)?;
        let candidates = exercises
            .into_iter()
            .zip(exercise_scores.iter())
            .filter(|(uid, _)| !self.data.blacklisted_uid(*uid).unwrap_or(false))
            .map(|(uid, score)| Candidate {
                exercise_uid: uid,
                num_hops: item.num_hops + 1,
                score: *score,
            })
            .collect::<Vec<Candidate>>();

        let avg_score = if exercise_scores.is_empty() {
            // Return 0.0 if there are no exercises to avoid division by zero.
            0.0
        } else {
            exercise_scores.iter().sum::<f32>() / (exercise_scores.len() as f32)
        };
        Ok((candidates, avg_score))
    }

    /// Returns whether the given dependency can be considered as satisfied. If all the dependencies
    /// of a unit are met, the search can continue with the unit.
    fn satisfied_dependency(
        &self,
        dependency_uid: u64,
        metadata_filter: Option<&MetadataFilter>,
    ) -> bool {
        // Dependencies which do not pass the filter are considered as satisfied.
        let passes_filter = self
            .data
            .unit_passes_filter(dependency_uid, metadata_filter)
            .unwrap_or(false);
        if !passes_filter {
            return true;
        }

        // Dependencies in the blacklist are considered as satisfied.
        let blacklisted = self.data.blacklisted_uid(dependency_uid);
        if blacklisted.unwrap_or(false) {
            return true;
        }

        // Dependencies which are a lesson belonging to a blacklisted course are considered as
        // satisfied.
        let course_id = self
            .data
            .get_lesson_course_id(dependency_uid)
            .unwrap_or_default();
        if self.data.blacklisted_id(&course_id).unwrap_or(false) {
            return true;
        }

        // Finally, dependencies with a score equal or greater than the passing score are considered
        // as satisfied.
        let score = self.score_cache.get_unit_score(dependency_uid);
        if score.is_err() || score.as_ref().unwrap().is_none() {
            return true;
        }
        let avg_score = score.unwrap().unwrap();
        avg_score >= self.data.options.passing_score
    }

    /// Returns whether all the dependencies of the given unit are satisfied.
    fn all_satisfied_dependencies(
        &self,
        unit_uid: u64,
        metadata_filter: Option<&MetadataFilter>,
    ) -> bool {
        // Any error during the filtering is ignored for the purpose of allowing the search to
        // continue if parts of the dependency graph are missing.
        if !self.data.unit_exists(unit_uid).unwrap_or(false) {
            // Ignore any missing unit.
            return true;
        }

        self.data
            .unit_graph
            .read()
            .get_dependencies(unit_uid)
            .unwrap_or_default()
            .into_iter()
            .filter(|dependency_uid| {
                // Ignore the implicit dependency between a lesson and its course.
                if let Some(course_uid) = self.data.unit_graph.read().get_lesson_course(unit_uid) {
                    if course_uid == *dependency_uid {
                        return false;
                    }
                }
                true
            })
            .all(|dependency_uid| self.satisfied_dependency(dependency_uid, metadata_filter))
    }

    /// Returns the valid dependents which can be visited after the given unit. A valid dependent is
    /// a unit whose full dependencies are met.
    fn get_valid_dependents(
        &self,
        unit_uid: u64,
        metadata_filter: Option<&MetadataFilter>,
    ) -> Vec<u64> {
        self.data
            .get_all_dependents(unit_uid)
            .into_iter()
            .filter(|unit_uid| self.all_satisfied_dependencies(*unit_uid, metadata_filter))
            .collect()
    }

    /// Searches for candidates across the entire graph.
    fn get_candidates_from_graph(
        &self,
        metadata_filter: Option<&MetadataFilter>,
    ) -> Result<Vec<Candidate>> {
        // Initialize stack with every starting lesson from the courses with no dpendencies.
        let mut stack: Vec<StackItem> = Vec::new();
        let starting_courses = self.get_all_starting_lessons(metadata_filter);
        stack.extend(starting_courses.into_iter());

        let max_candidates = self.data.options.batch_size * MAX_CANDIDATE_FACTOR;
        let mut all_candidates: Vec<Candidate> = Vec::new();
        let mut visited: HashSet<u64> = HashSet::new();
        // Keep track of the number of lessons that have been visited for each course. The search
        // will move onto the dependents of a course if all of its lessons have been visited.
        let mut pending_course_lessons: HashMap<u64, i64> = HashMap::new();

        while !stack.is_empty() {
            let curr_unit = stack.pop().unwrap();
            if visited.contains(&curr_unit.unit_uid) {
                continue;
            }

            if !self.data.unit_exists(curr_unit.unit_uid).unwrap_or(false) {
                // Try to add the valid dependents of any unit which cannot be found so that missing
                // sections of the graph do not stop the search.
                visited.insert(curr_unit.unit_uid);
                let valid_deps = self.get_valid_dependents(curr_unit.unit_uid, metadata_filter);
                Self::shuffle_to_stack(&curr_unit, valid_deps, &mut stack);
                continue;
            }

            let unit_type = self.data.get_unit_type(curr_unit.unit_uid)?;
            if unit_type == UnitType::Exercise {
                // The search only considers lessons and courses. Any exercise encountered by
                // mistake is ignored.
                continue;
            }

            if unit_type == UnitType::Course {
                if visited.contains(&curr_unit.unit_uid) {
                    continue;
                }

                let starting_lessons: Vec<u64> = self
                    .get_course_starting_lessons(curr_unit.unit_uid, metadata_filter)
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
                let blacklisted = self
                    .data
                    .blacklisted_uid(curr_unit.unit_uid)
                    .unwrap_or(false);

                if *pending_lessons <= 0 || !passes_filter || blacklisted {
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
                // If the lesson does not pass the metadata filter, push its valid dependents and
                // continue with the search.
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
    fn get_candidates_from_course(&self, course_ids: &[Ustr]) -> Result<Vec<Candidate>> {
        // Start the search with the starting lessons from the courses.
        let mut stack: Vec<StackItem> = Vec::new();
        let mut visited: HashSet<u64> = HashSet::new();
        for course_id in course_ids {
            let course_uid = self.data.get_uid(course_id)?;
            let starting_lessons = self
                .get_course_starting_lessons(course_uid, None)
                .unwrap_or_default()
                .into_iter()
                .map(|uid| StackItem {
                    unit_uid: uid,
                    num_hops: 0,
                });
            stack.extend(starting_lessons);
            visited.insert(course_uid);
        }

        let max_candidates = self.data.options.batch_size * MAX_CANDIDATE_FACTOR;
        let mut all_candidates: Vec<Candidate> = Vec::new();
        while !stack.is_empty() {
            let curr_unit = stack.pop().unwrap();
            if visited.contains(&curr_unit.unit_uid) {
                continue;
            } else {
                visited.insert(curr_unit.unit_uid);
            }

            if !self.data.unit_exists(curr_unit.unit_uid).unwrap_or(false) {
                // Try to add the valid dependents of any unit which cannot be found so that missing
                // sections of the graph do not stop the search.
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
            let lesson_course_id = self
                .data
                .get_lesson_course_id(curr_unit.unit_uid)
                .unwrap_or_default();
            if !course_ids.contains(&lesson_course_id) {
                // Ignore any lessons from other courses.
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

            let valid_deps = self.get_valid_dependents(curr_unit.unit_uid, None);
            Self::shuffle_to_stack(&curr_unit, valid_deps, &mut stack);
        }

        Ok(all_candidates)
    }

    /// Searches for candidates from the given lesson.
    fn get_candidates_from_lesson(&self, lesson_id: &Ustr) -> Result<Vec<Candidate>> {
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
    ) -> Result<Vec<(Ustr, ExerciseManifest)>> {
        let candidates = match filter {
            None => {
                // Retrieve candidates from the entire graph.
                self.get_candidates_from_graph(None)?
            }
            Some(filter) => match filter {
                UnitFilter::CourseFilter { course_ids } => {
                    // Retrieve candidates from the given courses.
                    self.get_candidates_from_course(&course_ids[..])?
                }
                UnitFilter::LessonFilter { lesson_ids } => {
                    // Retrieve candidate from the given lessons.
                    let mut candidates = Vec::new();
                    for lesson_id in lesson_ids {
                        candidates.extend(self.get_candidates_from_lesson(lesson_id)?.into_iter());
                    }
                    candidates
                }
                UnitFilter::MetadataFilter { filter } => {
                    // Retrieve candidates from the entire graph but only if the exercises belongs
                    // to a course or lesson matching the given metadata filter.
                    self.get_candidates_from_graph(Some(filter))?
                }
            },
        };

        let filter = CandidateFilter::new(self.data.clone());
        filter.filter_candidates(candidates)
    }

    fn score_exercise(
        &self,
        exercise_id: &Ustr,
        score: MasteryScore,
        timestamp: i64,
    ) -> Result<()> {
        let exercise_uid = self.data.get_uid(exercise_id)?;
        self.data
            .practice_stats
            .write()
            .record_exercise_score(exercise_id, score, timestamp)?;
        self.score_cache.invalidate_cached_score(exercise_uid);
        Ok(())
    }
}
