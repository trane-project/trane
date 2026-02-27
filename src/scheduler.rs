//! Defines and implements the data structures used to schedule batches of exercises to show to the
//! user. This module is the core mechanism of how Trane guides students to mastery of the material.
//!
//! The scheduler has a few core goals:
//! 1. Schedule exercises the user has practiced before in order to improve them and keep them up to
//!    date if they have been mastered already.
//! 2. Once the current material has been sufficiently mastered, schedule exercises that list the
//!    current material as a dependency.
//! 3. Optimize the difficulty of the schedule exercises so that the user is neither frustrated
//!    because many of the exercises are too difficult or bored because they have become too easy.
//!    The optimal area lies slightly outside their current comfort zone.
//! 4. Record the scores self-reported by the user to use them to drive the decisions done in
//!    service of all the other goals.
//!
//! In more formal terms, the scheduler's job is to plan the most optimal traversal of the graph of
//! skills as the student's performance blocks or unblocks certain paths. The current implementation
//! uses depth-first search to traverse the graph and collect a large pool of exercises, a multiple
//! of the actual exercises included in the final batch. From this large pool, the candidates are
//! split in groups of exercises which each match a disjoint range of scores to be randomly selected
//! into a list of fixed size. The result is combined, shuffled, and becomes the final batch
//! presented to the student.

pub mod data;
mod filter;
mod review_knocker;
mod reward_propagator;
mod unit_scorer;

use anyhow::Result;
use chrono::Utc;
use rand::{rng, seq::SliceRandom};
use reward_propagator::RewardPropagator;
use ustr::{Ustr, UstrMap, UstrSet};

use crate::{
    data::{
        ExerciseManifest, FULL_CANDIDATES_SCORE, MasteryScore, PassingScoreOptions,
        SchedulerOptions, UnitType,
        filter::{ExerciseFilter, KeyValueFilter, UnitFilter},
    },
    error::ExerciseSchedulerError,
    scheduler::{
        data::SchedulerData, filter::CandidateFilter, review_knocker::ReviewKnocker,
        unit_scorer::UnitScorer,
    },
};

/// The scheduler returns early if the search reaches a dead end and the number of candidates is
/// bigger than the multiple of the final batch size and this value. This is to avoid the need to
/// search the entire graph if the search already found a decently sized pool of candidates.
const MAX_CANDIDATE_FACTOR: usize = 10;

/// The trait that defines the interface for the scheduler. Contains functions to request a new
/// batch of exercises and to provide Trane the self-reported scores for said exercises.
pub trait ExerciseScheduler {
    /// Gets a new batch of exercises scheduled for a new trial. Contains an optimal filter to
    /// restrict the units visited during the search with the purpose of allowing students to choose
    /// which material to practice. If the filter is not provided, the scheduler will search the
    /// entire graph.
    fn get_exercise_batch(
        &self,
        filter: Option<ExerciseFilter>,
    ) -> Result<Vec<ExerciseManifest>, ExerciseSchedulerError>;

    /// Records the score of the given exercise's trial. The scores are used by the scheduler to
    /// decide when to stop traversing a path and how to sort and filter all the found candidates
    /// into a final batch.
    fn score_exercise(
        &self,
        exercise_id: Ustr,
        score: MasteryScore,
        timestamp: i64,
    ) -> Result<(), ExerciseSchedulerError>;

    /// Gets the score for the given unit. The unit can be a course, lesson, or exercise.
    fn get_unit_score(&self, unit_id: Ustr) -> Result<Option<f32>, ExerciseSchedulerError>;

    /// Removes any cached scores for the given unit. The score will be recomputed the next time the
    /// score is needed.
    ///
    /// The scores for lessons and exercises are cached to save a large amount of unnecessary
    /// computation. Without the caller manually invalidating the cache using this call, it is not
    /// possible to know when the cached value becomes outdated with the current interface. The
    /// reason is that the calls to modify the blacklist are not known by the scheduler.
    ///
    /// However, the final users of Trane do not need to call this function because the `Trane`
    /// object in lib.rs takes care of clearing the cache when exposing the interface that modifies
    /// the blacklist.
    fn invalidate_cached_score(&self, unit_id: Ustr);

    /// Removes any cached scores from units with the given prefix. The same considerations as
    /// `invalidate_cached_score` apply.
    fn invalidate_cached_scores_with_prefix(&self, prefix: &str);

    /// Returns the options used to control the behavior of the scheduler.
    fn get_scheduler_options(&self) -> SchedulerOptions;

    /// Sets the options used to control the behavior of the scheduler.
    fn set_scheduler_options(&mut self, options: SchedulerOptions);

    /// Resets the options used to control the behavior of the scheduler to their default values.
    fn reset_scheduler_options(&mut self);
}

/// An item in the stack of units that are scheduled for traversal during the process of scheduling
/// the next batch of exercises.
struct StackItem {
    /// The ID of the unit contained in the item.
    unit_id: Ustr,

    /// The depth of this unit from the starting unit. That is, the number of hops the graph search
    /// needed to reach this exercise.
    depth: usize,
}

/// An exercise selected during the initial phase of the search and which will be grouped with all
/// the other candidates which fall in the same mastery window and filtered and randomly selected
/// to form the final batch.
#[derive(Clone, Debug)]
struct Candidate {
    /// The ID of the exercise.
    exercise_id: Ustr,

    // The ID of the exercise's lesson.
    lesson_id: Ustr,

    /// The ID of the exercise's course.
    course_id: Ustr,

    /// The depth of this unit from the starting unit. That is, the number of hops the graph search
    /// needed to reach this exercise.
    depth: f32,

    /// The score assigned to the exercise represented as a float number between 0.0 and 5.0.
    exercise_score: f32,

    /// The score assigned to the lesson represented as a float number between 0.0 and 5.0.
    lesson_score: f32,

    /// The score assigned to the course represented as a float number between 0.0 and 5.0.
    course_score: f32,

    /// The number of previous trials that have been recorded for this exercise.
    num_trials: usize,

    /// The number of days since the last trial for this exercise.
    last_seen: f32,

    /// The number of times this exercise has been scheduled during the run of this scheduler. This
    /// value will be used to assign more weight to exercises that have been scheduled less often.
    frequency: usize,

    /// Whether this candidate comes from a lesson where the search stopped because the lesson's
    /// average score is still below the passing score.
    dead_end: bool,
}

/// An implementation of [`ExerciseScheduler`] based on depth-first search.
pub struct DepthFirstScheduler {
    /// The external data used by the scheduler. Contains pointers to the graph, blacklist, and
    /// course library and provides convenient functions.
    data: SchedulerData,

    /// Contains the logic for computing the scores of exercises, lessons, and courses, as well as
    /// for deciding whether the dependencies of a unit are satisfied.
    unit_scorer: UnitScorer,

    /// Contains the logic for propagating rewards through the graph.
    reward_propagator: RewardPropagator,

    /// Contains the logic for knocking highly encompassed exercises into the final batch to ensure
    /// that they are not overrepresented.
    review_knocker: ReviewKnocker,

    /// The filter used to build the final batch of exercises among the candidates found during the
    /// graph search.
    filter: CandidateFilter,
}

impl DepthFirstScheduler {
    /// Creates a new scheduler.
    #[must_use]
    pub fn new(data: SchedulerData) -> Self {
        Self {
            data: data.clone(),
            unit_scorer: UnitScorer::new(data.clone(), data.options.clone()),
            reward_propagator: RewardPropagator { data: data.clone() },
            review_knocker: ReviewKnocker::new(data.clone()),
            filter: CandidateFilter::new(data),
        }
    }

    /// Shuffles the units and pushes them to the given stack. Used with the goal of ensuring that
    /// the units are traversed in a different order each time a new batch is requested.
    fn shuffle_to_stack(curr_unit: &StackItem, mut units: Vec<Ustr>, stack: &mut Vec<StackItem>) {
        units.shuffle(&mut rng());
        stack.extend(units.iter().map(|id| StackItem {
            unit_id: *id,
            depth: curr_unit.depth + 1,
        }));
    }

    /// Returns all the courses and lessons without dependencies which are used to initialize a
    /// search of the entire graph.
    fn get_all_starting_units(&self) -> UstrSet {
        // Replace any missing units with their dependents and repeat this process until there are
        // no missing courses.
        let mut starting_courses = self.data.unit_graph.read().get_dependency_sinks();
        loop {
            let mut new_starting_courses = UstrSet::default();
            for course_id in &starting_courses {
                if self.data.unit_exists(*course_id).unwrap_or(false) {
                    new_starting_courses.insert(*course_id);
                } else {
                    new_starting_courses.extend(self.data.get_all_dependents(*course_id).iter());
                }
            }
            if new_starting_courses.eq(&starting_courses) {
                break;
            }
            starting_courses = new_starting_courses;
        }

        // Some courses added to the original list in the previous steps might have other
        // dependencies, some of which exist in the course library. This means they cannot be
        // considered a starting course, so remove them from the final output.
        starting_courses
            .into_iter()
            .filter(|course_id| {
                self.data
                    .unit_graph
                    .read()
                    .get_dependencies(*course_id)
                    .unwrap_or_default()
                    .iter()
                    .all(|id| !self.data.unit_exists(*id).unwrap())
            })
            .collect()
    }

    /// Returns the lessons in the course that have no dependencies with other lessons in the course
    /// and whose dependencies are satisfied.
    pub fn get_course_valid_starting_lessons(
        &self,
        course_id: Ustr,
        metadata_filter: Option<&KeyValueFilter>,
    ) -> Result<Vec<Ustr>> {
        Ok(self
            .data
            .unit_graph
            .read()
            .get_starting_lessons(course_id)
            .unwrap_or_default()
            .into_iter()
            .filter(|id| {
                // Filter out lessons whose dependencies are not satisfied. Otherwise, those lessons
                // would be traversed prematurely.
                self.all_satisfied_dependencies(*id, metadata_filter)
            })
            .collect())
    }

    //@<lp-example-1
    /// Returns an initial stack with all the starting units in the graph that are used to search
    /// the entire graph.
    fn get_initial_stack(&self, metadata_filter: Option<&KeyValueFilter>) -> Vec<StackItem> {
        // First get all the starting units and then all of their starting lessons.
        let starting_units = self.get_all_starting_units();
        let mut initial_stack: Vec<StackItem> = vec![];
        for course_id in starting_units {
            // Set the depth to zero since all the starting units are at the same depth.
            let lesson_ids = self
                .get_course_valid_starting_lessons(course_id, metadata_filter)
                .unwrap_or_default();

            if lesson_ids.is_empty() {
                // For units with no lessons, insert the unit itself as a starting unit so that its
                // dependents are traversed.
                initial_stack.push(StackItem {
                    unit_id: course_id,
                    depth: 0,
                });
            } else {
                // Insert all the starting lessons in the stack.
                initial_stack.extend(
                    lesson_ids
                        .into_iter()
                        .map(|unit_id| StackItem { unit_id, depth: 0 }),
                );
            }
        }

        // Shuffle the lessons to follow a different ordering each time a new batch is requested.
        initial_stack.shuffle(&mut rng());
        initial_stack
    }
    //>@lp-example-1

    /// Selects the right number of candidates based on the score of the unit and the passing
    /// options.
    fn select_candidates(
        candidates: Vec<Candidate>,
        score: f32,
        options: &PassingScoreOptions,
    ) -> Vec<Candidate> {
        // Return early when there are no candidates or all should be returned. Candidate selection
        // should only apply to lessons above the minimum passing score.
        if candidates.is_empty() {
            return Vec::new();
        }
        if score >= FULL_CANDIDATES_SCORE || score < options.min_score {
            return candidates;
        }

        // For scores after passing, linearly interpolate from min_fraction at min_score to 1.0 at
        // FULL_SCALE. Make sure to return at least one candidate.
        let min_fraction = options.min_fraction.clamp(0.0, 1.0);
        let fraction = min_fraction
            + ((score - options.min_score) / (FULL_CANDIDATES_SCORE - options.min_score))
                * (1.0 - min_fraction);
        let clamped_fraction = fraction.clamp(0.0, 1.0);
        let mut num_to_select = (clamped_fraction * candidates.len() as f32).floor() as usize;
        if clamped_fraction > 0.0 && num_to_select == 0 {
            num_to_select = 1;
        }

        // Shuffle the candidates and select the right number.
        let mut candidates = candidates;
        candidates.shuffle(&mut rng());
        candidates.into_iter().take(num_to_select).collect()
    }

    /// Returns the list of candidates selected from the given lesson along with the average score.
    /// The average score is used to help decide whether to continue searching a path in the graph.
    fn get_candidates_from_lesson_helper(&self, item: &StackItem) -> Result<(Vec<Candidate>, f32)> {
        // Retrieve the lesson's exercises.
        let exercises = self.data.all_valid_exercises_in_lesson(item.unit_id);
        if exercises.is_empty() {
            // Return early to avoid division by zero later on.
            return Ok((vec![], 0.0));
        }

        // Generate a list of candidates from the lesson's exercises.
        let course_id = self.data.get_course_id(item.unit_id).unwrap_or_default();
        let course_score = self
            .unit_scorer
            .get_unit_score(course_id)?
            .unwrap_or_default();
        let lesson_score = self
            .unit_scorer
            .get_unit_score(item.unit_id)?
            .unwrap_or_default();
        let candidates = exercises
            .into_iter()
            .map(|exercise_id| {
                Ok(Candidate {
                    exercise_id,
                    lesson_id: item.unit_id, // It's assumed that the item is a lesson.
                    course_id,
                    depth: (item.depth + 1) as f32,
                    exercise_score: self
                        .unit_scorer
                        .get_unit_score(exercise_id)?
                        .unwrap_or_default(),
                    course_score,
                    lesson_score,
                    num_trials: self
                        .unit_scorer
                        .get_num_trials(exercise_id)?
                        .unwrap_or_default(),
                    last_seen: self
                        .unit_scorer
                        .get_last_seen_days(exercise_id)?
                        .unwrap_or_default(),
                    frequency: self.data.get_exercise_frequency(exercise_id),
                    dead_end: false,
                })
            })
            .collect::<Result<Vec<Candidate>>>()?;

        // Compute the lesson average directly from the candidate exercise scores and select the
        // right fraction of candidates based on the lesson average and passing options.
        let avg_score =
            candidates.iter().map(|c| c.exercise_score).sum::<f32>() / candidates.len() as f32;
        let selected_candidates =
            Self::select_candidates(candidates, avg_score, &self.data.options.passing_score_v2);
        Ok((selected_candidates, avg_score))
    }

    /// Returns the matching lessons in a course that have no matching dependents in the same
    /// course. These lessons represent the edge of progress within the filtered subset of the
    /// course.
    fn last_matching_lessons_in_course(
        &self,
        course_id: Ustr,
        metadata_filter: Option<&KeyValueFilter>,
    ) -> UstrSet {
        // Get the lessons that match the filter.
        let matching_lessons: UstrSet = self
            .data
            .unit_graph
            .read()
            .get_course_lessons(course_id)
            .unwrap_or_default()
            .into_iter()
            .filter(|lesson_id| {
                self.data
                    .unit_passes_filter(*lesson_id, metadata_filter)
                    .unwrap_or(false)
            })
            .collect();
        if matching_lessons.is_empty() {
            return UstrSet::default();
        }

        // Find the last matching lessons, which are those that do not have dependents on the other
        // lessons.
        matching_lessons
            .iter()
            .copied()
            .filter(|lesson_id| {
                let dependents = self
                    .data
                    .unit_graph
                    .read()
                    .get_dependents(*lesson_id)
                    .unwrap_or_default();
                dependents.is_disjoint(&matching_lessons)
            })
            .collect()
    }

    /// Resolves effective dependencies, bridging through units filtered out by metadata.
    fn resolve_effective_dependencies(
        &self,
        dependency_id: Ustr,
        metadata_filter: Option<&KeyValueFilter>,
        visited: &mut UstrSet,
    ) -> UstrSet {
        // Skip nodes that were already visited while resolving this dependency to avoid cycles.
        if !visited.insert(dependency_id) {
            return UstrSet::default(); // grcov-excl-line
        }

        // If the unit passes the metadata filter, it is an effective dependency.
        let passes_filter = self
            .data
            .unit_passes_filter(dependency_id, metadata_filter)
            .unwrap_or(false);
        if passes_filter {
            return [dependency_id].into_iter().collect();
        }

        match self.data.get_unit_type(dependency_id) {
            // For filtered-out lessons, bridge through the lesson dependencies. If the lesson is a
            // starting lesson in its course, also bridge through the course dependencies.
            Some(UnitType::Lesson) => {
                // Get the dependencies of the lesson.
                let mut next_dependencies: UstrSet = self
                    .data
                    .unit_graph
                    .read()
                    .get_dependencies(dependency_id)
                    .unwrap_or_default();

                // Starting lessons are effectively dependent on the course dependencies, so add
                // them as well.
                let course_id = self
                    .data
                    .get_lesson_course(dependency_id)
                    .unwrap_or_default();
                let is_starting_lesson = self
                    .data
                    .unit_graph
                    .read()
                    .get_starting_lessons(course_id)
                    .unwrap_or_default()
                    .contains(&dependency_id);
                if is_starting_lesson {
                    next_dependencies.extend(
                        self.data
                            .unit_graph
                            .read()
                            .get_dependencies(course_id)
                            .unwrap_or_default(),
                    );
                }

                next_dependencies
                    .into_iter()
                    .flat_map(|next_dependency| {
                        self.resolve_effective_dependencies(
                            next_dependency,
                            metadata_filter,
                            visited,
                        )
                        .into_iter()
                    })
                    .collect()
            }
            // For filtered-out courses, bridge to the last matching lessons. If there are no
            // matching lessons, bridge through the course dependencies.
            Some(UnitType::Course) => {
                let last_matching_lessons =
                    self.last_matching_lessons_in_course(dependency_id, metadata_filter);
                if !last_matching_lessons.is_empty() {
                    return last_matching_lessons;
                }

                self.data
                    .unit_graph
                    .read()
                    .get_dependencies(dependency_id)
                    .unwrap_or_default()
                    .into_iter()
                    .flat_map(|next_dependency| {
                        self.resolve_effective_dependencies(
                            next_dependency,
                            metadata_filter,
                            visited,
                        )
                        .into_iter()
                    })
                    .collect()
            }
            _ => UstrSet::default(),
        }
    }

    /// Returns whether an effective dependency can be considered as satisfied.
    fn satisfied_effective_dependency(&self, dependency_id: Ustr) -> bool {
        // Dependencies in the blacklist are considered as satisfied, so the search can continue
        // past them.
        let blacklisted = self.data.blacklisted(dependency_id);
        if blacklisted.unwrap_or(false) {
            return true;
        }

        // Dependencies which are a lesson belonging to a blacklisted course are considered as
        // satisfied, so the search can continue past them.
        let course_id = self
            .data
            .get_lesson_course(dependency_id)
            .unwrap_or_default();
        if self.data.blacklisted(course_id).unwrap_or(false) {
            return true;
        }

        // The dependency is considered as satisfied if it's been superseded by another unit.
        let superseding = self.unit_scorer.get_superseding_recursive(dependency_id);
        if let Some(superseding) = superseding
            && self.unit_scorer.is_superseded(dependency_id, &superseding)
        {
            return true;
        }

        // Finally, dependencies with a score equal or greater than the passing score are considered
        // as satisfied.
        let score = self.unit_scorer.get_unit_score(dependency_id);
        if let Ok(Some(score)) = score {
            score >= self.data.options.passing_score_v2.min_score
        } else {
            // If the score cannot be retrieved, consider the dependency as satisfied to avoid
            // blocking the search in the case of blacklisted or missing units.
            true
        }
    }

    /// Returns whether the given dependency is satisfied, bridging through filtered-out units to
    /// find the effective dependencies that should gate traversal.
    fn satisfied_dependency(
        &self,
        dependency_id: Ustr,
        metadata_filter: Option<&KeyValueFilter>,
    ) -> bool {
        let mut visited = UstrSet::default();
        let targets =
            self.resolve_effective_dependencies(dependency_id, metadata_filter, &mut visited);
        if targets.is_empty() {
            return true;
        }
        targets
            .into_iter()
            .all(|target| self.satisfied_effective_dependency(target))
    }

    /// Returns whether all the dependencies of the given unit are satisfied.
    fn all_satisfied_dependencies(
        &self,
        unit_id: Ustr,
        metadata_filter: Option<&KeyValueFilter>,
    ) -> bool {
        self.data
            .unit_graph
            .read()
            .get_dependencies(unit_id)
            .unwrap_or_default()
            .into_iter()
            .all(|dependency_id| self.satisfied_dependency(dependency_id, metadata_filter))
    }

    /// Returns the valid dependents which can be visited after the given unit. A valid dependent is
    /// a unit whose full dependencies are met.
    fn get_valid_dependents(
        &self,
        unit_id: Ustr,
        metadata_filter: Option<&KeyValueFilter>,
    ) -> Vec<Ustr> {
        self.data
            .get_all_dependents(unit_id)
            .into_iter()
            .filter(|unit_id| self.all_satisfied_dependencies(*unit_id, metadata_filter))
            .collect()
    }

    // Returns whether the given course should be skipped during the search. If so, the valid
    /// dependents of the course should be added to the stack.
    fn skip_course(
        &self,
        course_id: Ustr,
        metadata_filter: Option<&KeyValueFilter>,
        pending_course_lessons: &mut UstrMap<usize>,
    ) -> bool {
        // Check if the course is blacklisted.
        let blacklisted = self.data.blacklisted(course_id).unwrap_or(false);

        // Check if the course passes the metadata filter.
        let passes_filter = self
            .data
            .unit_passes_filter(course_id, metadata_filter)
            .unwrap_or(true);

        // Check the number of pending lessons in the course.
        let pending_lessons = pending_course_lessons
            .entry(course_id)
            .or_insert_with(|| self.data.get_num_lessons_in_course(course_id));

        // Check if the course has been superseded by another unit.
        let superseding_units = self
            .unit_scorer
            .get_superseding_recursive(course_id)
            .unwrap_or_default();
        let is_superseded = self
            .unit_scorer
            .is_superseded(course_id, &superseding_units);

        // The course should be skipped if the course is blacklisted, does not pass the filter, has
        // no pending lessons, or if it's been superseded.
        blacklisted || !passes_filter || *pending_lessons == 0 || is_superseded
    }

    /// Returns whether the given lesson should be skipped during the search. If so, the valid
    /// dependents of the lesson should be added to the stack.
    fn skip_lesson(&self, lesson_id: Ustr, metadata_filter: Option<&KeyValueFilter>) -> bool {
        // Check if the lesson is blacklisted.
        let blacklisted = self.data.blacklisted(lesson_id).unwrap_or(false);

        // Check if the lesson passes the metadata filter.
        let passes_filter = self
            .data
            .unit_passes_filter(lesson_id, metadata_filter)
            .unwrap_or(true);

        // Check if the lesson has been superseded by another unit.
        let superseding_units = self
            .unit_scorer
            .get_superseding_recursive(lesson_id)
            .unwrap_or_default();
        let is_lesson_superseded = self
            .unit_scorer
            .is_superseded(lesson_id, &superseding_units);

        // Check if the lesson's course has been superseded by another unit.
        let course_id = self.data.get_lesson_course(lesson_id).unwrap_or_default();
        let superseding_units = self
            .unit_scorer
            .get_superseding_recursive(course_id)
            .unwrap_or_default();
        let is_course_superseded = self
            .unit_scorer
            .is_superseded(course_id, &superseding_units);

        // The lesson should be skipped if it is blacklisted, does not pass the filter or if it or
        // its course have been superseded.
        blacklisted || !passes_filter || is_lesson_superseded || is_course_superseded
    }

    /// Searches for candidates across the entire graph. An optional metadata filter can be used to
    /// only select exercises from the courses and lessons that match the filter and ignore the rest
    /// of the graph while still respecting the dependency relationships.
    fn get_candidates_from_graph(
        &self,
        initial_stack: Vec<StackItem>,
        metadata_filter: Option<&KeyValueFilter>,
    ) -> Result<Vec<Candidate>> {
        // Initialize the stack with every starting lesson, which are those units with no
        // dependencies that are needed to reach all the units in the graph.
        let mut stack: Vec<StackItem> = Vec::new();
        stack.extend(initial_stack);

        // Initialize the list of candidates and the set of visited units.
        let max_candidates = self.data.options.batch_size * MAX_CANDIDATE_FACTOR;
        let mut all_candidates: Vec<Candidate> = Vec::new();
        let mut visited = UstrSet::default();

        // The dependency relationships between a course and its lessons are not explicitly encoded
        // in the graph. While this would simplify this section of the search logic, it would
        // require that courses are represented by two nodes. The first incoming node would connect
        // the course dependencies to the first lessons in the course. The second outgoing node
        // would connect the last lessons in the course to the course dependents.
        //
        // To get past this limitation, the search will only add the course dependents until all of
        // its lessons have been visited and mastered. This value is tracked by the
        // `pending_course_lessons` map.
        let mut pending_course_lessons: UstrMap<usize> = UstrMap::default();

        // Perform a depth-first search of the graph.
        while let Some(curr_unit) = stack.pop() {
            // Immediately skip the item if it has been visited.
            if visited.contains(&curr_unit.unit_id) {
                continue;
            }

            // The logic past this point depends on the type of the unit.
            let unit_type = self.data.get_unit_type(curr_unit.unit_id);
            if unit_type.is_none() {
                // The type of the unit is unknown. This can happen when a unit depends on some
                // missing unit not in the course library.
                continue; // grcov-excl-line
            }
            let unit_type = unit_type.unwrap();

            // Handle courses and lessons. Exercises are skipped by the search.
            if unit_type == UnitType::Course {
                // Retrieve the starting lessons in the course and add them to the stack.
                let starting_lessons: Vec<Ustr> = self
                    .get_course_valid_starting_lessons(curr_unit.unit_id, metadata_filter)
                    .unwrap_or_default();
                Self::shuffle_to_stack(&curr_unit, starting_lessons, &mut stack);

                // The course can be skipped. Add it to the visited set, push its valid dependents
                // onto the stack, and continue.
                if self.skip_course(
                    curr_unit.unit_id,
                    metadata_filter,
                    &mut pending_course_lessons,
                ) {
                    visited.insert(curr_unit.unit_id);
                    let valid_deps = self.get_valid_dependents(curr_unit.unit_id, metadata_filter);
                    Self::shuffle_to_stack(&curr_unit, valid_deps, &mut stack);
                }
            } else if unit_type == UnitType::Lesson {
                // If the searched reached this point, the unit must be a lesson.
                visited.insert(curr_unit.unit_id);

                // Update the number of lessons pending to be processed.
                let course_id = self.data.get_course_id(curr_unit.unit_id)?;
                let pending_lessons = pending_course_lessons
                    .entry(course_id)
                    .or_insert_with(|| self.data.get_num_lessons_in_course(course_id));
                if *pending_lessons > 0 {
                    *pending_lessons -= 1;
                }

                // Check whether there are pending lessons.
                if *pending_lessons == 0 {
                    // Once all the lessons in the course have been visited, re-add the course to
                    // the stack, so the search can continue exploring its dependents.
                    stack.push(StackItem {
                        unit_id: course_id,
                        depth: curr_unit.depth + 1,
                    });
                }

                // Retrieve the valid dependents of the lesson, and directly skip the lesson if
                // needed.
                let valid_deps = self.get_valid_dependents(curr_unit.unit_id, metadata_filter);
                if self.skip_lesson(curr_unit.unit_id, metadata_filter) {
                    Self::shuffle_to_stack(&curr_unit, valid_deps, &mut stack);
                    continue;
                }

                // Retrieve the candidates from the lesson and add them to the list of candidates.
                let (mut candidates, avg_score) =
                    self.get_candidates_from_lesson_helper(&curr_unit)?;
                let num_candidates = candidates.len();

                // Compare the score against the passing score to decide whether the search should
                // continue past this lesson.
                if num_candidates > 0 && avg_score < self.data.options.passing_score_v2.min_score {
                    for candidate in &mut candidates {
                        candidate.dead_end = true;
                    }
                    all_candidates.extend(candidates);

                    // Search reached a dead-end. If there are already enough candidates, terminate
                    // the search. Otherwise, continue with the search and shuffle the entire stack
                    // to prioritize other paths in the graph.
                    if all_candidates.len() >= max_candidates {
                        break; // grcov-excl-line
                    }
                    stack.shuffle(&mut rng());
                    continue;
                }

                // The search should continue past this lesson. Add the its candidates and continue
                // the search via its valid dependents.
                all_candidates.extend(candidates);
                Self::shuffle_to_stack(&curr_unit, valid_deps, &mut stack);
            }
        }

        Ok(all_candidates)
    }

    /// Searches for candidates from the given course.
    fn get_candidates_from_course(&self, course_ids: &[Ustr]) -> Result<Vec<Candidate>> {
        // Initialize the set of visited units and the stack with the starting lessons from the
        // courses. Add all starting lessons, even if their dependencies are not satisfied, because
        // the user specifically asked for questions from these courses.
        let mut stack: Vec<StackItem> = Vec::new();
        let mut visited = UstrSet::default();
        for course_id in course_ids {
            let starting_lessons = self
                .data
                .unit_graph
                .read()
                .get_starting_lessons(*course_id)
                .unwrap_or_default()
                .into_iter()
                .map(|id| StackItem {
                    unit_id: id,
                    depth: 0,
                });
            stack.extend(starting_lessons);
            visited.insert(*course_id);
        }

        // Initialize the list of candidates.
        let max_candidates = self.data.options.batch_size * MAX_CANDIDATE_FACTOR;
        let mut all_candidates: Vec<Candidate> = Vec::new();

        // Perform a depth-first search to find the candidates.
        while let Some(curr_unit) = stack.pop() {
            // Continue if the unit has been visited and update the list of visited units.
            if visited.contains(&curr_unit.unit_id) {
                continue; // grcov-excl-line
            }
            visited.insert(curr_unit.unit_id);

            // The logic past this point depends on the type of the unit. Only handle lessons. Other
            // courses and exercises are skipped by the search.
            let unit_type = self
                .data
                .get_unit_type(curr_unit.unit_id)
                .unwrap_or(UnitType::Course);
            if unit_type == UnitType::Lesson {
                // Ignore lessons from other courses that might have been added to the stack if a
                // lesson has dependencies from a course not in the input courses.
                let lesson_course_id = self
                    .data
                    .get_lesson_course(curr_unit.unit_id)
                    .unwrap_or_default();
                if !course_ids.contains(&lesson_course_id) {
                    continue;
                }

                // Retrieve the valid dependents of the lesson, and directly skip the lesson if
                // needed.
                let valid_deps = self.get_valid_dependents(curr_unit.unit_id, None);
                if self.skip_lesson(curr_unit.unit_id, None) {
                    Self::shuffle_to_stack(&curr_unit, valid_deps, &mut stack);
                    continue;
                }

                // Retrieve the candidates from the lesson and add them to the list of candidates.
                let (mut candidates, avg_score) =
                    self.get_candidates_from_lesson_helper(&curr_unit)?;
                let num_candidates = candidates.len();

                // Compare the score against the passing score to decide whether the search should
                // continue past this lesson.
                if num_candidates > 0 && avg_score < self.data.options.passing_score_v2.min_score {
                    for candidate in &mut candidates {
                        candidate.dead_end = true;
                    }
                    all_candidates.extend(candidates);

                    // Search reached a dead-end. If there are already enough candidates, terminate
                    // the search. Otherwise, continue with the search and shuffle the entire stack
                    // to prioritize other
                    if all_candidates.len() >= max_candidates {
                        break; // grcov-excl-line
                    }
                    stack.shuffle(&mut rng());
                    continue;
                }

                // The search should continue past this lesson. Add the its candidates and continue
                // the search via its valid dependents.
                all_candidates.extend(candidates);
                Self::shuffle_to_stack(&curr_unit, valid_deps, &mut stack);
            }
        }

        Ok(all_candidates)
    }

    /// Searches for candidates from the given lesson.
    fn get_candidates_from_lesson(&self, lesson_id: Ustr) -> Result<Vec<Candidate>> {
        let (candidates, _) = self.get_candidates_from_lesson_helper(&StackItem {
            unit_id: lesson_id,
            depth: 0,
        })?;
        Ok(candidates)
    }

    /// Searches for candidates from the units in the review list. This mode allows the student to
    /// exclusively practice the courses, lessons, and exercises they have marked for review.
    fn get_candidates_from_review_list(&self) -> Result<Vec<Candidate>> {
        // Retrieve candidates from each entry in the review list.
        let mut candidates = vec![];
        let review_list = self.data.review_list.read().get_review_list_entries()?;
        for unit_id in review_list {
            match self.data.get_unit_type_strict(unit_id)? {
                UnitType::Course => {
                    // If the unit is a course, use the course scheduler to retrieve candidates.
                    let course_ids = vec![unit_id];
                    candidates.extend(self.get_candidates_from_course(&course_ids)?);
                }
                UnitType::Lesson => {
                    // If the unit is a lesson, use the lesson scheduler to retrieve candidates.
                    candidates.extend(self.get_candidates_from_lesson(unit_id)?);
                }
                UnitType::Exercise => {
                    // Retrieve the exercise's lesson and course IDs.
                    let lesson_id = self.data.get_lesson_id(unit_id).unwrap_or_default();
                    let course_id = self.data.get_course_id(lesson_id).unwrap_or_default();

                    // If the unit is an exercise, directly add it to the list of candidates.
                    candidates.push(Candidate {
                        exercise_id: unit_id,
                        lesson_id,
                        course_id,
                        depth: 0.0,
                        exercise_score: self
                            .unit_scorer
                            .get_unit_score(unit_id)?
                            .unwrap_or_default(),
                        lesson_score: self
                            .unit_scorer
                            .get_unit_score(lesson_id)?
                            .unwrap_or_default(),
                        course_score: self
                            .unit_scorer
                            .get_unit_score(course_id)?
                            .unwrap_or_default(),
                        num_trials: self
                            .unit_scorer
                            .get_num_trials(unit_id)?
                            .unwrap_or_default(),
                        last_seen: self
                            .unit_scorer
                            .get_last_seen_days(unit_id)?
                            .unwrap_or_default(),
                        frequency: *self.data.frequency_map.read().get(&unit_id).unwrap_or(&0),
                        dead_end: false,
                    });
                }
            }
        }

        Ok(candidates)
    }

    /// Retrieves an initial batch of candidates based on the given filter.
    fn get_initial_candidates(&self, filter: Option<ExerciseFilter>) -> Result<Vec<Candidate>> {
        // Retrieve an initial list of candidates based on the type of the filter.
        let candidates = match filter {
            None => {
                // If the filter is empty, retrieve candidates from the entire graph. This mode is
                // Trane's default.
                let initial_stack = self.get_initial_stack(None);
                self.get_candidates_from_graph(initial_stack, None)?
            }
            Some(filter) => match filter {
                // Otherwise, use the given filter to select how candidates are retrieved.
                ExerciseFilter::UnitFilter(filter) => match filter {
                    UnitFilter::CourseFilter { course_ids } => {
                        self.get_candidates_from_course(&course_ids[..])?
                    }
                    UnitFilter::LessonFilter { lesson_ids } => {
                        let mut candidates = Vec::new();
                        for lesson_id in lesson_ids {
                            candidates
                                .extend(self.get_candidates_from_lesson(lesson_id)?.into_iter());
                        }
                        candidates
                    }
                    UnitFilter::MetadataFilter { filter } => {
                        let initial_stack = self.get_initial_stack(Some(&filter));
                        self.get_candidates_from_graph(initial_stack, Some(&filter))?
                    }
                    UnitFilter::ReviewListFilter => self.get_candidates_from_review_list()?,
                    UnitFilter::Dependents { unit_ids } => {
                        let initial_stack = unit_ids
                            .iter()
                            .map(|unit_id| StackItem {
                                unit_id: *unit_id,
                                depth: 0,
                            })
                            .collect();
                        self.get_candidates_from_graph(initial_stack, None)?
                    }
                    UnitFilter::Dependencies { unit_ids, depth } => {
                        let dependencies: Vec<Ustr> = unit_ids
                            .iter()
                            .flat_map(|unit_id| {
                                self.data.get_dependencies_at_depth(*unit_id, depth)
                            })
                            .collect();
                        let initial_stack = dependencies
                            .iter()
                            .map(|unit_id| StackItem {
                                unit_id: *unit_id,
                                depth: 0,
                            })
                            .collect();
                        self.get_candidates_from_graph(initial_stack, None)?
                    }
                },
                ExerciseFilter::StudySession(session_data) => {
                    let unit_filter = self
                        .data
                        .get_session_filter(&session_data, Utc::now())?
                        .map(ExerciseFilter::UnitFilter);
                    self.get_initial_candidates(unit_filter)?
                }
            },
        };

        Ok(candidates)
    }
}

impl ExerciseScheduler for DepthFirstScheduler {
    fn get_exercise_batch(
        &self,
        filter: Option<ExerciseFilter>,
    ) -> Result<Vec<ExerciseManifest>, ExerciseSchedulerError> {
        // Retrieve an initial batch of candidates based on the type of the filter.
        let initial_candidates = self
            .get_initial_candidates(filter)
            .map_err(ExerciseSchedulerError::GetExerciseBatch)?;

        // Knock out highly encompassed exercises from the initial batch to ensure that they are not
        // overrepresented in the final batch.
        let knocked_out = self.review_knocker.knock_out_reviews(initial_candidates);

        // Sort the candidates into buckets, select the right number from each, and convert them
        // into a final batch of exercises.
        let final_candidates = self
            .filter
            .filter_candidates(knocked_out)
            .map_err(ExerciseSchedulerError::GetExerciseBatch)?;

        // Increment the frequency of the exercises in the batch. These exercises will have a lower
        // chance of being selected in the future, so that exercises that have not been selected as
        // often have a higher chance of being selected.
        for exercise_manifest in &final_candidates {
            self.data.increment_exercise_frequency(exercise_manifest.id);
        }

        Ok(final_candidates)
    }

    fn score_exercise(
        &self,
        exercise_id: Ustr,
        score: MasteryScore,
        timestamp: i64,
    ) -> Result<(), ExerciseSchedulerError> {
        // Write the score to the practice stats database.
        self.data
            .practice_stats
            .write()
            .record_exercise_score(exercise_id, score.clone(), timestamp)
            .map_err(|e| ExerciseSchedulerError::ScoreExercise(e.into()))?;
        self.unit_scorer.invalidate_cached_score(exercise_id);

        // Propagate the rewards along the unit graph and store them.
        let rewards = self
            .reward_propagator
            .propagate_rewards(exercise_id, &score, timestamp);
        for (unit_id, reward) in &rewards {
            // Ignore rewards for units with no previous scores.
            let unit_score = self
                .unit_scorer
                .get_unit_score(*unit_id)
                .unwrap_or_default()
                .unwrap_or_default();
            if unit_score == 0.0 {
                continue;
            }

            let updated = self
                .data
                .practice_rewards
                .write()
                .record_unit_reward(*unit_id, reward)
                .map_err(|e| ExerciseSchedulerError::ScoreExercise(e.into()))?;
            if updated {
                self.unit_scorer.invalidate_cached_score(*unit_id);
            }
        }

        Ok(())
    }

    #[cfg_attr(coverage, coverage(off))]
    fn get_unit_score(&self, unit_id: Ustr) -> Result<Option<f32>, ExerciseSchedulerError> {
        self.unit_scorer
            .get_unit_score(unit_id)
            .map_err(|e| ExerciseSchedulerError::GetUnitScore(unit_id, e))
    }

    #[cfg_attr(coverage, coverage(off))]
    fn invalidate_cached_score(&self, unit_id: Ustr) {
        self.unit_scorer.invalidate_cached_score(unit_id);
    }

    #[cfg_attr(coverage, coverage(off))]
    fn invalidate_cached_scores_with_prefix(&self, prefix: &str) {
        self.unit_scorer
            .invalidate_cached_scores_with_prefix(prefix);
    }

    fn get_scheduler_options(&self) -> SchedulerOptions {
        self.data.options.clone()
    }

    fn set_scheduler_options(&mut self, options: SchedulerOptions) {
        self.data.options = options;
    }

    fn reset_scheduler_options(&mut self) {
        self.data.options = SchedulerOptions::default();
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use ustr::Ustr;

    use super::*;

    fn candidate(id: u32, lesson_score: f32, exercise_score: f32, depth: f32) -> Candidate {
        let exercise_id = format!("exercise-{id}");
        Candidate {
            exercise_id: Ustr::from(exercise_id.as_str()),
            lesson_id: Ustr::from("lesson"),
            course_id: Ustr::from("course"),
            depth,
            exercise_score,
            lesson_score,
            course_score: 0.0,
            num_trials: 0,
            last_seen: 0.0,
            frequency: 0,
            dead_end: false,
        }
    }

    /// Helper function to easily generate test cases for the `select_candidates` function.
    fn select(
        lesson_score: f32,
        num_candidates: usize,
        options: PassingScoreOptions,
    ) -> Vec<Candidate> {
        let candidates = (0..num_candidates)
            .map(|idx| candidate(idx as u32, lesson_score, 0.0, 1.0))
            .collect::<Vec<_>>();
        DepthFirstScheduler::select_candidates(candidates, lesson_score, &options)
    }

    /// Verifies that an empty list of candidates results in an empty selection.
    #[test]
    fn select_candidates_empty() {
        assert!(
            DepthFirstScheduler::select_candidates(vec![], 0.0, &PassingScoreOptions::default(),)
                .is_empty()
        );
    }

    /// Verifies that when the lesson score is below the minimum score, the right fraction of
    /// candidates is selected.
    #[test]
    fn select_candidates_below_minimum_score() {
        let candidates = select(
            2.0,
            5,
            PassingScoreOptions {
                min_score: 3.0,
                min_fraction: 0.2,
            },
        );
        assert_eq!(candidates.len(), 5);
    }

    /// Verifies that when the lesson score is below the minimum score but the minimum fraction is
    /// zero, all candidates are selected.   
    #[test]
    fn select_candidates_below_minimum_score_with_zero_fraction() {
        let candidates = select(
            2.0,
            5,
            PassingScoreOptions {
                min_score: 3.0,
                min_fraction: 0.0,
            },
        );
        assert_eq!(candidates.len(), 5);
    }

    /// Verifies that when the lesson score is equal or above the minimum score, at least the minimum
    /// fraction of candidates is selected.
    #[test]
    fn select_candidates_minimum_score_guarantees_one() {
        let candidates = select(
            3.0,
            2,
            PassingScoreOptions {
                min_score: 3.0,
                min_fraction: 0.2,
            },
        );
        assert_eq!(candidates.len(), 1);
    }

    /// Verifies that when the lesson score is above the minimum score, the right fraction of
    /// candidates is selected.
    #[test]
    fn select_candidates_partial_selection() {
        let candidates = select(
            3.8,
            10,
            PassingScoreOptions {
                min_score: 3.0,
                min_fraction: 0.2,
            },
        );
        assert_eq!(candidates.len(), 8);
    }

    /// Verifies that when the lesson score is above the minimum score but the minimum fraction is
    /// zero, at least one candidate is selected.
    #[test]
    fn select_candidates_always_keep_one_when_fraction_positive() {
        let candidates = select(
            3.01,
            2,
            PassingScoreOptions {
                min_score: 3.0,
                min_fraction: 0.0,
            },
        );
        assert_eq!(candidates.len(), 1);
    }

    /// Verifies that when the lesson score is at the maximum, all candidates are selected.
    #[test]
    fn select_candidates_full_selection() {
        let candidates = select(
            5.0,
            11,
            PassingScoreOptions {
                min_score: 3.0,
                min_fraction: 0.2,
            },
        );
        assert_eq!(candidates.len(), 11);
    }
}
