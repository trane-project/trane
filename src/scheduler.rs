//! Module defining the data structures used to schedule batches of exercises to show to the user.
//! The core of Trane's logic is in this module.
mod cache;

use anyhow::{anyhow, Result};
use rand::{distributions::WeightedIndex, prelude::Distribution, seq::SliceRandom, thread_rng};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

use crate::{
    blacklist::Blacklist,
    course_library::CourseLibrary,
    data::{
        filter::{FilterOp, MetadataFilter, UnitFilter},
        CourseManifest, ExerciseManifest, LessonManifest, MasteryScore, MasteryWindowOpts,
        SchedulerOptions, UnitType,
    },
    graph::UnitGraph,
    practice_stats::PracticeStats,
    scheduler::cache::ScoreCache,
};

/// The batch size will be multiplied by this factor in order to expand the range of the search and
/// avoid always returning the same exercises. A search concludes early if it reaches a dead-end and
/// there are already more candidates than said product.
const MAX_CANDIDATE_FACTOR: usize = 10;

/// Contains functions used to retrieve a new batch of exercises.
pub trait ExerciseScheduler {
    /// Sets the options used while searching for exercise candidates.
    fn set_options(&self, options: SchedulerOptions);

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

/// A struct encapsulating all the state needed to schedule exercises.
#[derive(Clone)]
pub(crate) struct SchedulerData {
    /// The course library storing manifests and info about units.
    pub course_library: Arc<RwLock<dyn CourseLibrary>>,

    /// The dependency graph of courses and lessons.
    pub unit_graph: Arc<RwLock<dyn UnitGraph>>,

    /// The list of previous exercise results.
    pub practice_stats: Arc<RwLock<dyn PracticeStats>>,

    /// The list of units to skip during scheduling.
    pub blacklist: Arc<RwLock<dyn Blacklist>>,
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
    /// The options used to schedule exercises.
    options: RefCell<SchedulerOptions>,

    /// The data used to schedule exercises.
    data: SchedulerData,

    /// A cache of unit uid to its score.
    score_cache: ScoreCache,
}

impl DepthFirstScheduler {
    /// Creates a new simple scheduler.
    pub fn new(data: SchedulerData, options: SchedulerOptions) -> Self {
        Self {
            options: RefCell::new(options.clone()),
            data: data.clone(),
            score_cache: ScoreCache::new(data, options),
        }
    }

    /// Returns the UID of the lesson with the given ID.
    fn get_uid(&self, unit_id: &str) -> Result<u64> {
        self.data
            .unit_graph
            .read()
            .unwrap()
            .get_uid(unit_id)
            .ok_or_else(|| anyhow!("missing UID for unit with ID {}", unit_id))
    }

    /// Returns the ID of the lesson with the given UID.
    fn get_id(&self, unit_uid: u64) -> Result<String> {
        self.data
            .unit_graph
            .read()
            .unwrap()
            .get_id(unit_uid)
            .ok_or_else(|| anyhow!("missing ID for unit with UID {}", unit_uid))
    }

    /// Returns the uid of the course to which the lesson with the given UID belongs.
    fn get_course_uid(&self, lesson_uid: u64) -> Result<u64> {
        self.data
            .unit_graph
            .read()
            .unwrap()
            .get_lesson_course(lesson_uid)
            .ok_or_else(|| anyhow!("missing course UID for lesson with UID {}", lesson_uid))
    }

    /// Returns the type of the given unit.
    fn get_unit_type(&self, unit_uid: u64) -> Result<UnitType> {
        self.data
            .unit_graph
            .read()
            .unwrap()
            .get_unit_type(unit_uid)
            .ok_or_else(|| anyhow!("missing unit type for unit with UID {}", unit_uid))
    }

    /// Returns the manifest for the course with the given UID.
    fn get_course_manifest(&self, course_uid: u64) -> Result<CourseManifest> {
        let course_id = self.get_id(course_uid)?;
        self.data
            .course_library
            .read()
            .unwrap()
            .get_course_manifest(&course_id)
            .ok_or_else(|| anyhow!("missing manifest for course with ID {}", course_id))
    }

    /// Returns the manifest for the course with the given UID.
    fn get_lesson_manifest(&self, lesson_uid: u64) -> Result<LessonManifest> {
        let lesson_id = self.get_id(lesson_uid)?;
        self.data
            .course_library
            .read()
            .unwrap()
            .get_lesson_manifest(&lesson_id)
            .ok_or_else(|| anyhow!("missing manifest for lesson with ID {}", lesson_id))
    }

    /// Returns the manifest for the exercise with the given UID.
    fn get_exercise_manifest(&self, exercise_uid: u64) -> Result<ExerciseManifest> {
        let exercise_id = self.get_id(exercise_uid)?;
        self.data
            .course_library
            .read()
            .unwrap()
            .get_exercise_manifest(&exercise_id)
            .ok_or_else(|| anyhow!("missing manifest for exercise with ID {}", exercise_id))
    }

    /// Returns the value of the course_id field in the manifest of the given lesson.
    fn get_lesson_course_id(&self, lesson_uid: u64) -> Result<String> {
        Ok(self.get_lesson_manifest(lesson_uid)?.course_id)
    }

    /// Returns whether the unit exists in the library. Some units will exists in the unit graph
    /// because they are a dependency of another but their data might not exist in the library.
    fn unit_exists(&self, unit_uid: u64) -> Result<bool, String> {
        let graph_guard = self.data.unit_graph.read().unwrap();

        let unit_id = graph_guard.get_id(unit_uid);
        if unit_id.is_none() {
            return Ok(false);
        }
        let unit_type = graph_guard.get_unit_type(unit_uid);
        if unit_type.is_none() {
            return Ok(false);
        }

        let library_guard = self.data.course_library.read().unwrap();
        match unit_type.unwrap() {
            UnitType::Course => match library_guard.get_course_manifest(&unit_id.unwrap()) {
                None => Ok(false),
                Some(_) => Ok(true),
            },
            UnitType::Lesson => match library_guard.get_lesson_manifest(&unit_id.unwrap()) {
                None => Ok(false),
                Some(_) => Ok(true),
            },
            UnitType::Exercise => match library_guard.get_exercise_manifest(&unit_id.unwrap()) {
                None => Ok(false),
                Some(_) => Ok(true),
            },
        }
    }

    /// Returns whether the unit with the given UID is blacklisted.
    fn blacklisted_uid(&self, unit_uid: u64) -> Result<bool> {
        let unit_id = self.get_id(unit_uid)?;
        self.data.blacklist.read().unwrap().blacklisted(&unit_id)
    }

    /// Returns whether the unit with the given ID is blacklisted.
    fn blacklisted_id(&self, unit_id: &str) -> Result<bool> {
        self.data.blacklist.read().unwrap().blacklisted(unit_id)
    }

    /// Returns all the units that are dependencies of the unit with the given UID.
    fn get_all_dependents(&self, unit_uid: u64) -> Vec<u64> {
        return self
            .data
            .unit_graph
            .read()
            .unwrap()
            .get_dependents(unit_uid)
            .unwrap_or_default()
            .into_iter()
            .collect();
    }

    /// Applies the metadata filter to the given unit.
    fn apply_metadata_filter(
        &self,
        unit_uid: u64,
        metadata_filter: &MetadataFilter,
    ) -> Result<Option<bool>> {
        let unit_type = self.get_unit_type(unit_uid)?;
        match unit_type {
            UnitType::Course => match &metadata_filter.course_filter {
                None => Ok(None),
                Some(filter) => {
                    let manifest = self.get_course_manifest(unit_uid)?;
                    Ok(Some(filter.apply(&manifest)))
                }
            },
            UnitType::Lesson => match &metadata_filter.lesson_filter {
                None => Ok(None),
                Some(filter) => {
                    let manifest = self.get_lesson_manifest(unit_uid)?;
                    Ok(Some(filter.apply(&manifest)))
                }
            },
            UnitType::Exercise => Err(anyhow!(
                "cannot apply metadata filter to exercise with UID {}",
                unit_uid
            )),
        }
    }

    /// Returns whether the unit passes the metadata filter, handling all interactions between
    /// lessons and course metadata filters.
    fn unit_passes_filter(
        &self,
        unit_uid: u64,
        metadata_filter: Option<&MetadataFilter>,
    ) -> Result<bool> {
        if metadata_filter.is_none() {
            return Ok(true);
        }
        let metadata_filter = metadata_filter.unwrap();

        let unit_type = self.get_unit_type(unit_uid)?;
        match unit_type {
            UnitType::Exercise => Err(anyhow!(
                "cannot apply metadata filter to exercise with UID {}",
                unit_uid
            )),
            UnitType::Course => {
                let course_passes = self
                    .apply_metadata_filter(unit_uid, metadata_filter)
                    .unwrap_or(None);
                match (
                    metadata_filter.lesson_filter.as_ref(),
                    metadata_filter.course_filter.as_ref(),
                ) {
                    (None, None) => Ok(true),
                    (Some(_), None) => Ok(false),
                    (None, Some(_)) => Ok(course_passes.unwrap_or(false)),
                    (Some(_), Some(_)) => match metadata_filter.op {
                        FilterOp::All => Ok(false),
                        FilterOp::Any => Ok(course_passes.unwrap_or(false)),
                    },
                }
            }
            UnitType::Lesson => {
                let lesson_manifest = self.get_lesson_manifest(unit_uid)?;
                let course_uid = self.get_uid(&lesson_manifest.course_id)?;

                let lesson_passes = self
                    .apply_metadata_filter(unit_uid, metadata_filter)
                    .unwrap_or(None);
                let course_passes = self
                    .apply_metadata_filter(course_uid, metadata_filter)
                    .unwrap_or(None);

                match (
                    metadata_filter.lesson_filter.as_ref(),
                    metadata_filter.course_filter.as_ref(),
                ) {
                    (None, None) => Ok(true),
                    (Some(_), None) => Ok(lesson_passes.unwrap_or(false)),
                    (None, Some(_)) => Ok(course_passes.unwrap_or(false)),
                    (Some(_), Some(_)) => match metadata_filter.op {
                        FilterOp::All => {
                            Ok(lesson_passes.unwrap_or(false) && course_passes.unwrap_or(false))
                        }
                        FilterOp::Any => {
                            Ok(lesson_passes.unwrap_or(false) || course_passes.unwrap_or(false))
                        }
                    },
                }
            }
        }
    }

    /// Returns the valid dependents which can be visited after the given unit. A valid dependent is
    /// a unit whose full dependencies are met.
    fn get_valid_dependents(
        &self,
        unit_uid: u64,
        metadata_filter: Option<&MetadataFilter>,
    ) -> Vec<u64> {
        let dependents = self.get_all_dependents(unit_uid);
        if dependents.is_empty() {
            return dependents;
        }

        // Any error during the filtering is ignored for the purpose of allowing the search to
        // continue if parts of the dependency graph are missing.
        dependents
            .into_iter()
            .filter(|uid| {
                let exists = self.unit_exists(*uid);
                if exists.is_err() {
                    return true;
                }

                let dependencies = self
                    .data
                    .unit_graph
                    .read()
                    .unwrap()
                    .get_dependencies(*uid)
                    .unwrap_or_default();
                if dependencies.is_empty() {
                    return true;
                }

                let num_dependencies = dependencies.len();
                let met_dependencies = dependencies.into_iter().filter(|dep_uid| {
                    let passes_filter = self
                        .unit_passes_filter(*dep_uid, metadata_filter)
                        .unwrap_or(false);
                    if !passes_filter {
                        return true;
                    }

                    let blacklisted = self.blacklisted_uid(*dep_uid);
                    if blacklisted.is_err() || blacklisted.unwrap() {
                        return true;
                    }

                    let course_id = self.get_lesson_course_id(*dep_uid).unwrap_or_default();
                    if self.blacklisted_id(&course_id).unwrap_or(false) {
                        return true;
                    }

                    let score = self.score_cache.get_unit_score(*dep_uid);
                    if score.is_err() || score.as_ref().unwrap().is_none() {
                        return true;
                    }
                    let avg_score = score.unwrap().unwrap();
                    avg_score >= self.options.borrow().passing_score
                });
                met_dependencies.count() == num_dependencies
            })
            .collect()
    }

    /// Shuffles the units and pushes them to the given stack.
    fn shuffle_to_stack(curr_unit: &StackItem, mut units: Vec<u64>, stack: &mut Vec<StackItem>) {
        units.shuffle(&mut thread_rng());
        stack.extend(units.iter().map(|uid| StackItem {
            unit_uid: *uid,
            num_hops: curr_unit.num_hops + 1,
        }));
    }

    /// Returns the number of lessons in the given course.
    fn get_num_lessons_in_course(&self, course_uid: u64) -> i64 {
        let lessons: HashSet<u64> = self
            .data
            .unit_graph
            .read()
            .unwrap()
            .get_course_lessons(course_uid)
            .unwrap_or_default();
        lessons.len() as i64
    }

    fn get_all_starting_courses(&self) -> HashSet<u64> {
        let mut starting_courses = self.data.unit_graph.read().unwrap().get_dependency_sinks();
        let mut num_courses = starting_courses.len();
        // Replace any missing courses with their dependents and repeat this process until there are
        // no missing courses.
        loop {
            let mut new_starting_courses = HashSet::new();
            for course_uid in starting_courses {
                if self.unit_exists(course_uid).unwrap() {
                    new_starting_courses.insert(course_uid);
                } else {
                    new_starting_courses.extend(self.get_all_dependents(course_uid).iter());
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
                    .read()
                    .unwrap()
                    .get_dependencies(*course_uid)
                    .unwrap_or_default();
                if dependencies.is_empty() {
                    return true;
                }
                dependencies
                    .iter()
                    .all(|uid| !self.unit_exists(*uid).unwrap())
            })
            .collect()
    }

    /// Returns all the starting lessons in the graph.
    fn get_all_starting_lessons(&self) -> Vec<StackItem> {
        let starting_courses = self.get_all_starting_courses();
        let mut starting_lessons: Vec<StackItem> = vec![];
        for course_uid in starting_courses {
            let lesson_uids = self
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

    /// Returns the starting lessons for the given course.
    fn get_course_starting_lessons(&self, course_uid: u64) -> Result<Vec<u64>> {
        let lessons: Vec<u64> = self
            .data
            .unit_graph
            .read()
            .unwrap()
            .get_course_starting_lessons(course_uid)
            .unwrap_or_default()
            .into_iter()
            .collect();
        Ok(lessons)
    }

    /// Returns the exercises contained within the given unit.
    fn get_lesson_exercises(&self, unit_uid: u64) -> Vec<u64> {
        self.data
            .unit_graph
            .read()
            .unwrap()
            .get_lesson_exercises(unit_uid)
            .unwrap_or_default()
            .into_iter()
            .collect()
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
        if self.blacklisted_uid(item.unit_uid).unwrap_or(false) {
            return Ok((vec![], 0.0));
        }
        let course_id = self.get_lesson_course_id(item.unit_uid)?;
        if self.blacklisted_id(&course_id).unwrap_or(false) {
            return Ok((vec![], 0.0));
        }

        let exercises = self.get_lesson_exercises(item.unit_uid);
        let exercise_scores = self.get_exercise_scores(&exercises)?;
        let candidates = exercises
            .into_iter()
            .filter(|uid| !self.blacklisted_uid(*uid).unwrap_or(false))
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

    /// Filters the candidates whose score fit in the given window.
    fn candidates_in_window(
        candidates: &[Candidate],
        window_opts: &MasteryWindowOpts,
    ) -> Vec<Candidate> {
        candidates
            .iter()
            .filter(|c| window_opts.in_window(c.score))
            .cloned()
            .collect()
    }

    /// Takes a list of candidates and randomly selectes num_selected candidates among them. The
    /// probabilities of selecting a candidate are weighted based on their score and the number of
    /// hops taken by the graph search to find them. Lower scores and higher number of hops give the
    /// candidate a higher chance of being selected. The function returns a tuple of the selected
    /// candidates and the remainder.
    fn select_candidates(
        candidates: Vec<Candidate>,
        num_selected: usize,
    ) -> (Vec<Candidate>, Vec<Candidate>) {
        if candidates.len() <= num_selected {
            return (candidates, vec![]);
        }

        // Create the list of weights for each candidate. A small value (0.5) is added to each
        // weight to prevent any of the weights from being zero.
        let mut weights: Vec<f32> = candidates
            .iter()
            .map(|c| 0.5 + (5.0 - c.score.min(5.0)) + (0.1 * (c.num_hops as f32)))
            .collect();
        let mut dist = WeightedIndex::new(&weights).unwrap();
        let mut rng = thread_rng();

        let mut selected = Vec::new();
        let mut turns = 0;
        while selected.len() < num_selected && turns <= num_selected {
            let index = dist.sample(&mut rng);
            selected.push(candidates[index].clone());

            // Update the weight of this index to zero so it's never selected again.
            weights[index] = 0.0;
            dist = WeightedIndex::new(&weights).unwrap();
            turns += 1;
        }

        // Compute all the candidates that were not selected.
        let mut remainder = vec![];
        for (i, weight) in weights.iter().enumerate() {
            if *weight != 0.0 {
                remainder.push(candidates[i].clone());
            }
        }
        (selected, remainder)
    }

    /// Takes a list of candidates and returns a vector of tuples of exercises IDs and manifests.
    fn candidates_to_exercises(
        &self,
        candidates: Vec<Candidate>,
    ) -> Result<Vec<(String, ExerciseManifest)>> {
        let mut exercises = candidates
            .into_iter()
            .map(|c| -> Result<(String, ExerciseManifest)> {
                let id = self.get_id(c.exercise_uid)?;
                let manifest = self.get_exercise_manifest(c.exercise_uid)?;
                Ok((id, manifest))
            })
            .collect::<Result<Vec<(String, ExerciseManifest)>>>()?;
        exercises.shuffle(&mut thread_rng());
        Ok(exercises)
    }

    /// Takes a list of exercises and filters them so that the end result is a list of exercise
    /// manifests which fit the options given to the scheduler.
    fn filter_candidates(
        &self,
        candidates: Vec<Candidate>,
    ) -> Result<Vec<(String, ExerciseManifest)>> {
        let options = self.options.borrow();
        let batch_size_float = options.batch_size as f32;

        let easy_candidates = Self::candidates_in_window(&candidates, &options.easy_window_opts);
        let current_candidates =
            Self::candidates_in_window(&candidates, &options.current_window_opts);
        let target_candidates =
            Self::candidates_in_window(&candidates, &options.target_window_opts);
        let mut final_candidates = Vec::new();

        let num_easy = (batch_size_float * options.easy_window_opts.percentage) as usize;
        let (easy_selected, easy_remainder) = Self::select_candidates(easy_candidates, num_easy);
        final_candidates.extend(easy_selected);

        let num_current = (batch_size_float * options.current_window_opts.percentage) as usize;
        let (current_selected, current_remainder) =
            Self::select_candidates(current_candidates, num_current);
        final_candidates.extend(current_selected);

        let remainder = options.batch_size - final_candidates.len();
        let (target_selected, _) = Self::select_candidates(target_candidates, remainder);
        final_candidates.extend(target_selected);

        if final_candidates.len() < options.batch_size {
            let remainder = options.batch_size - final_candidates.len();
            final_candidates.extend(
                current_remainder[..remainder.min(current_remainder.len())]
                    .iter()
                    .cloned(),
            );
        }
        if final_candidates.len() < options.batch_size {
            let remainder = options.batch_size - final_candidates.len();
            final_candidates.extend(
                easy_remainder[..remainder.min(easy_remainder.len())]
                    .iter()
                    .cloned(),
            );
        }

        self.candidates_to_exercises(candidates)
    }

    /// Searches for candidates across the entire graph.
    fn get_candidates_from_graph(
        &self,
        metadata_filter: Option<&MetadataFilter>,
    ) -> Result<Vec<Candidate>> {
        let mut stack: Vec<StackItem> = Vec::new();
        let starting_courses = self.get_all_starting_lessons();
        stack.extend(starting_courses.into_iter());

        let max_candidates = self.options.borrow().batch_size * MAX_CANDIDATE_FACTOR;
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

            let exists = self.unit_exists(curr_unit.unit_uid);
            if exists.is_err() || !exists.unwrap() {
                // Try to add the valid dependents of any unit which cannot be found so that missing
                // sections of the graph do not stop the search.
                visited.insert(curr_unit.unit_uid);
                let valid_deps = self.get_valid_dependents(curr_unit.unit_uid, metadata_filter);
                Self::shuffle_to_stack(&curr_unit, valid_deps, &mut stack);
                continue;
            }

            let unit_type = self.get_unit_type(curr_unit.unit_uid)?;
            if unit_type == UnitType::Course {
                if visited.contains(&curr_unit.unit_uid) {
                    continue;
                }

                let starting_lessons: Vec<u64> = self
                    .get_course_starting_lessons(curr_unit.unit_uid)
                    .unwrap_or_default();
                Self::shuffle_to_stack(&curr_unit, starting_lessons, &mut stack);

                // Update the count of pending lessons. The course depends on each of its lessons
                // but the search only moves forward once all of its lessons have been visited.
                let pending_lessons = pending_course_lessons
                    .entry(curr_unit.unit_uid)
                    .or_insert_with(|| self.get_num_lessons_in_course(curr_unit.unit_uid));
                let passes_filter = self
                    .unit_passes_filter(curr_unit.unit_uid, metadata_filter)
                    .unwrap_or(true);

                if *pending_lessons <= 0
                    || !passes_filter
                    || self.blacklisted_uid(curr_unit.unit_uid).unwrap_or(false)
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
            let course_uid = self.get_course_uid(curr_unit.unit_uid)?;
            let pending_lessons = pending_course_lessons
                .entry(course_uid)
                .or_insert_with(|| self.get_num_lessons_in_course(course_uid));
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
                .unit_passes_filter(curr_unit.unit_uid, metadata_filter)
                .unwrap_or(true);
            if !passes_filter {
                Self::shuffle_to_stack(&curr_unit, valid_deps, &mut stack);
                continue;
            }

            let (candidates, avg_score) = self.get_candidates_from_lesson_helper(&curr_unit)?;
            let num_candidates = candidates.len();
            all_candidates.extend(candidates);

            if num_candidates > 0 && avg_score < self.options.borrow().passing_score {
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
        let course_uid = self.get_uid(course_id)?;
        let starting_lessons = self
            .get_course_starting_lessons(course_uid)
            .unwrap_or_default();
        let mut stack: Vec<StackItem> = starting_lessons
            .into_iter()
            .map(|uid| StackItem {
                unit_uid: uid,
                num_hops: 0,
            })
            .collect();

        let max_candidates = self.options.borrow().batch_size * MAX_CANDIDATE_FACTOR;
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

            let exists = self.unit_exists(curr_unit.unit_uid);
            if exists.is_err() || !exists.unwrap() {
                // Try to add the valid dependents of any unit which cannot be found.
                let valid_deps = self.get_valid_dependents(curr_unit.unit_uid, None);
                Self::shuffle_to_stack(&curr_unit, valid_deps, &mut stack);
                continue;
            }

            let unit_type = self.get_unit_type(curr_unit.unit_uid)?;
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
            let lesson_course_id = self.get_lesson_course_id(curr_unit.unit_uid);
            if lesson_course_id.is_err() {
                continue;
            }
            if lesson_course_id.unwrap() != course_id {
                continue;
            }

            let (candidates, avg_score) = self.get_candidates_from_lesson_helper(&curr_unit)?;
            let num_candidates = candidates.len();
            all_candidates.extend(candidates);

            if num_candidates > 0 && avg_score < self.options.borrow().passing_score {
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
        let lesson_uid = self.get_uid(lesson_id)?;
        let (candidates, _) = self.get_candidates_from_lesson_helper(&StackItem {
            unit_uid: lesson_uid,
            num_hops: 0,
        })?;
        Ok(candidates)
    }
}

impl ExerciseScheduler for DepthFirstScheduler {
    fn set_options(&self, options: SchedulerOptions) {
        self.options.replace(options.clone());
        self.score_cache.set_options(options);
    }

    fn get_exercise_batch(
        &self,
        filter: Option<&UnitFilter>,
    ) -> Result<Vec<(String, ExerciseManifest)>> {
        let candidates = match filter {
            None => self.get_candidates_from_graph(None)?,
            Some(filter) => match filter {
                UnitFilter::CourseFilter { course_id } => {
                    self.get_candidates_from_course(course_id)?
                }
                UnitFilter::LessonFilter { lesson_id } => {
                    self.get_candidates_from_lesson(lesson_id)?
                }
                UnitFilter::MetadataFilter { filter } => {
                    self.get_candidates_from_graph(Some(filter))?
                }
            },
        };

        self.filter_candidates(candidates)
    }

    fn record_exercise_score(
        &self,
        exercise_id: &str,
        score: MasteryScore,
        timestamp: i64,
    ) -> Result<()> {
        let exercise_uid = self.get_uid(exercise_id)?;
        self.data
            .practice_stats
            .write()
            .unwrap()
            .record_exercise_score(exercise_id, score, timestamp)?;
        self.score_cache.invalidate_cached_score(exercise_uid);
        Ok(())
    }
}
