//! Implementation of Trane and all its associated traits, but using FFI types instead. Traits that
//! do not use FFI types are not redefined in this file, but an implementation of the original trait
//! is provided.

use anyhow::Result;
use std::path::Path;
use ustr::Ustr;
use ustr::UstrSet;

use crate::{
    blacklist::Blacklist,
    course_library::CourseLibrary,
    data::{ffi::filter::*, ffi::*, ExerciseTrial},
    error::*,
    filter_manager::FilterManager,
    graph::UnitGraph,
    practice_stats::PracticeStats,
    repository_manager::RepositoryManager,
    review_list::ReviewList,
    scheduler::ExerciseScheduler,
    study_session_manager::StudySessionManager,
    Trane,
};

/// An instance of Trane that uses FFI types for easier use in contexts where those types are
/// needed. For example, when using Trane from within a program that sends a serialized version of
/// the types to a Typescript frontend.
pub struct TraneFFI {
    trane: Trane,
}

impl TraneFFI {
    /// Creates and wraps a new instance of Trane for use with FFI.
    pub fn new(working_dir: &Path, library_root: &Path) -> Result<TraneFFI> {
        Ok(TraneFFI {
            trane: Trane::new(working_dir, library_root)?,
        })
    }
}

// grcov-excl-start: The following is just glue code to translate between the native and FFI traits.
// Thus, successful compilation is considered sufficient coverage.

impl Blacklist for TraneFFI {
    fn add_to_blacklist(&mut self, unit_id: &Ustr) -> Result<(), BlacklistError> {
        self.trane.add_to_blacklist(unit_id)
    }
    fn remove_from_blacklist(&mut self, unit_id: &Ustr) -> Result<(), BlacklistError> {
        self.trane.remove_from_blacklist(unit_id)
    }
    fn remove_prefix_from_blacklist(&mut self, prefix: &str) -> Result<(), BlacklistError> {
        self.trane.remove_prefix_from_blacklist(prefix)
    }
    fn blacklisted(&self, unit_id: &Ustr) -> Result<bool, BlacklistError> {
        self.trane.blacklisted(unit_id)
    }
    fn get_blacklist_entries(&self) -> Result<Vec<Ustr>, BlacklistError> {
        self.trane.get_blacklist_entries()
    }
}

/// The FFI version of the `CourseLibrary` trait.
#[allow(missing_docs)]
pub trait CourseLibraryFFI {
    fn get_course_manifest(&self, course_id: &Ustr) -> Option<CourseManifest>;
    fn get_lesson_manifest(&self, lesson_id: &Ustr) -> Option<LessonManifest>;
    fn get_exercise_manifest(&self, exercise_id: &Ustr) -> Option<ExerciseManifest>;
    fn get_course_ids(&self) -> Vec<Ustr>;
    fn get_lesson_ids(&self, course_id: &Ustr) -> Option<Vec<Ustr>>;
    fn get_exercise_ids(&self, lesson_id: &Ustr) -> Option<Vec<Ustr>>;
    fn get_all_exercise_ids(&self) -> Vec<Ustr>;
    fn search(&self, query: &str) -> Result<Vec<Ustr>, CourseLibraryError>;
    fn get_user_preferences(&self) -> UserPreferences;
}

impl CourseLibraryFFI for TraneFFI {
    fn get_course_manifest(&self, course_id: &Ustr) -> Option<CourseManifest> {
        self.trane.get_course_manifest(course_id).map(Into::into)
    }
    fn get_lesson_manifest(&self, lesson_id: &Ustr) -> Option<LessonManifest> {
        self.trane.get_lesson_manifest(lesson_id).map(Into::into)
    }

    fn get_exercise_manifest(&self, exercise_id: &Ustr) -> Option<ExerciseManifest> {
        self.trane
            .get_exercise_manifest(exercise_id)
            .map(Into::into)
    }
    fn get_course_ids(&self) -> Vec<Ustr> {
        self.trane.get_course_ids()
    }
    fn get_lesson_ids(&self, course_id: &Ustr) -> Option<Vec<Ustr>> {
        self.trane.get_lesson_ids(course_id)
    }
    fn get_exercise_ids(&self, lesson_id: &Ustr) -> Option<Vec<Ustr>> {
        self.trane.get_exercise_ids(lesson_id)
    }
    fn get_all_exercise_ids(&self) -> Vec<Ustr> {
        self.trane.get_all_exercise_ids()
    }
    fn search(&self, query: &str) -> Result<Vec<Ustr>, CourseLibraryError> {
        self.trane.search(query)
    }
    fn get_user_preferences(&self) -> UserPreferences {
        self.trane.get_user_preferences().into()
    }
}

/// The FFI version of the `ExerciseScheduler` trait.
#[allow(missing_docs)]
pub trait ExerciseSchedulerFFI {
    fn get_exercise_batch(
        &self,
        filter: Option<ExerciseFilter>,
    ) -> Result<Vec<(Ustr, ExerciseManifest)>, ExerciseSchedulerError>;
    fn score_exercise(
        &self,
        exercise_id: &Ustr,
        score: MasteryScore,
        timestamp: i64,
    ) -> Result<(), ExerciseSchedulerError>;
    fn invalidate_cached_score(&self, unit_id: &Ustr);
    fn invalidate_cached_scores_with_prefix(&self, prefix: &str);
    fn get_scheduler_options(&self) -> SchedulerOptions;
    fn set_scheduler_options(&mut self, options: SchedulerOptions);
    fn reset_scheduler_options(&mut self);
}

impl ExerciseSchedulerFFI for TraneFFI {
    fn get_exercise_batch(
        &self,
        filter: Option<ExerciseFilter>,
    ) -> Result<Vec<(Ustr, ExerciseManifest)>, ExerciseSchedulerError> {
        Ok(self
            .trane
            .get_exercise_batch(filter.map(Into::into))?
            .into_iter()
            .map(|(unit_id, manifest)| (unit_id, manifest.into()))
            .collect())
    }
    fn score_exercise(
        &self,
        exercise_id: &Ustr,
        score: MasteryScore,
        timestamp: i64,
    ) -> Result<(), ExerciseSchedulerError> {
        self.trane
            .score_exercise(exercise_id, score.into(), timestamp)
    }
    fn invalidate_cached_score(&self, unit_id: &Ustr) {
        self.trane.invalidate_cached_score(unit_id)
    }
    fn invalidate_cached_scores_with_prefix(&self, prefix: &str) {
        self.trane.invalidate_cached_scores_with_prefix(prefix)
    }
    fn get_scheduler_options(&self) -> SchedulerOptions {
        self.trane.get_scheduler_options().into()
    }
    fn set_scheduler_options(&mut self, options: SchedulerOptions) {
        self.trane.set_scheduler_options(options.into())
    }
    fn reset_scheduler_options(&mut self) {
        self.trane.reset_scheduler_options()
    }
}

/// The FFI version of the FilterManager trait.
#[allow(missing_docs)]
pub trait FilterManagerFFI {
    fn get_filter(&self, id: &str) -> Option<SavedFilter>;
    fn list_filters(&self) -> Vec<(String, String)>;
}

impl FilterManagerFFI for TraneFFI {
    fn get_filter(&self, id: &str) -> Option<SavedFilter> {
        self.trane.get_filter(id).map(Into::into)
    }
    fn list_filters(&self) -> Vec<(String, String)> {
        self.trane.list_filters()
    }
}

/// The FFI version of the `UnitManager` trait.
#[allow(missing_docs)]
pub trait PracticeStatsFFI {
    fn get_scores(
        &self,
        exercise_id: &Ustr,
        num_scores: usize,
    ) -> Result<Vec<ExerciseTrial>, PracticeStatsError>;
    fn record_exercise_score(
        &mut self,
        exercise_id: &Ustr,
        score: MasteryScore,
        timestamp: i64,
    ) -> Result<(), PracticeStatsError>;
    fn trim_scores(&mut self, num_scores: usize) -> Result<(), PracticeStatsError>;
}

impl PracticeStatsFFI for TraneFFI {
    fn get_scores(
        &self,
        exercise_id: &Ustr,
        num_scores: usize,
    ) -> Result<Vec<ExerciseTrial>, PracticeStatsError> {
        self.trane.get_scores(exercise_id, num_scores)
    }
    fn record_exercise_score(
        &mut self,
        exercise_id: &Ustr,
        score: MasteryScore,
        timestamp: i64,
    ) -> Result<(), PracticeStatsError> {
        self.trane
            .record_exercise_score(exercise_id, score.into(), timestamp)
    }
    fn trim_scores(&mut self, num_scores: usize) -> Result<(), PracticeStatsError> {
        self.trane.trim_scores(num_scores)
    }
}

/// The FFI version of the `RepositoryManager` trait.
#[allow(missing_docs)]
pub trait RepositoryManagerFFI {
    fn add_repo(
        &mut self,
        url: &str,
        repo_id: Option<String>,
    ) -> Result<(), RepositoryManagerError>;
    fn remove_repo(&mut self, repo_id: &str) -> Result<(), RepositoryManagerError>;
    fn update_repo(&self, repo_id: &str) -> Result<(), RepositoryManagerError>;
    fn update_all_repos(&self) -> Result<(), RepositoryManagerError>;
    fn list_repos(&self) -> Vec<RepositoryMetadata>;
}

impl RepositoryManagerFFI for TraneFFI {
    fn add_repo(
        &mut self,
        url: &str,
        repo_id: Option<String>,
    ) -> Result<(), RepositoryManagerError> {
        self.trane.add_repo(url, repo_id)
    }
    fn remove_repo(&mut self, repo_id: &str) -> Result<(), RepositoryManagerError> {
        self.trane.remove_repo(repo_id)
    }
    fn update_repo(&self, repo_id: &str) -> Result<(), RepositoryManagerError> {
        self.trane.update_repo(repo_id)
    }
    fn update_all_repos(&self) -> Result<(), RepositoryManagerError> {
        self.trane.update_all_repos()
    }
    fn list_repos(&self) -> Vec<RepositoryMetadata> {
        self.trane
            .list_repos()
            .into_iter()
            .map(Into::into)
            .collect()
    }
}

/// The FFI version of the `ReviewList` trait.
#[allow(missing_docs)]
pub trait ReviewListFFI {
    fn add_to_review_list(&mut self, unit_id: &Ustr) -> Result<(), ReviewListError>;
    fn remove_from_review_list(&mut self, unit_id: &Ustr) -> Result<(), ReviewListError>;
    fn get_review_list_entries(&self) -> Result<Vec<Ustr>, ReviewListError>;
}

impl ReviewListFFI for TraneFFI {
    fn add_to_review_list(&mut self, unit_id: &Ustr) -> Result<(), ReviewListError> {
        self.trane.add_to_review_list(unit_id)
    }
    fn remove_from_review_list(&mut self, unit_id: &Ustr) -> Result<(), ReviewListError> {
        self.trane.remove_from_review_list(unit_id)
    }
    fn get_review_list_entries(&self) -> Result<Vec<Ustr>, ReviewListError> {
        self.trane.get_review_list_entries()
    }
}

/// The FFI version of the `StudySessionManager` trait.
#[allow(missing_docs)]
pub trait StudySessionManagerFFI {
    fn get_study_session(&self, id: &str) -> Option<StudySession>;
    fn list_study_sessions(&self) -> Vec<(String, String)>;
}

impl StudySessionManagerFFI for TraneFFI {
    fn get_study_session(&self, id: &str) -> Option<StudySession> {
        self.trane.get_study_session(id).map(Into::into)
    }
    fn list_study_sessions(&self) -> Vec<(String, String)> {
        self.trane.list_study_sessions()
    }
}

/// The FFI version of the `UnitGraph` trait.
#[allow(missing_docs)]
pub trait UnitGraphFFI {
    fn add_course(&mut self, course_id: &Ustr) -> Result<(), UnitGraphError>;
    fn add_lesson(&mut self, lesson_id: &Ustr, course_id: &Ustr) -> Result<(), UnitGraphError>;
    fn add_exercise(&mut self, exercise_id: &Ustr, lesson_id: &Ustr) -> Result<(), UnitGraphError>;
    fn add_dependencies(
        &mut self,
        unit_id: &Ustr,
        unit_type: UnitType,
        dependencies: &[Ustr],
    ) -> Result<()>;
    fn get_unit_type(&self, unit_id: &Ustr) -> Option<UnitType>;
    fn get_course_lessons(&self, course_id: &Ustr) -> Option<UstrSet>;
    fn update_starting_lessons(&mut self);
    fn get_starting_lessons(&self, course_id: &Ustr) -> Option<UstrSet>;
    fn get_lesson_course(&self, lesson_id: &Ustr) -> Option<Ustr>;
    fn get_lesson_exercises(&self, lesson_id: &Ustr) -> Option<UstrSet>;
    fn get_exercise_lesson(&self, exercise_id: &Ustr) -> Option<Ustr>;
    fn get_dependencies(&self, unit_id: &Ustr) -> Option<UstrSet>;
    fn get_dependents(&self, unit_id: &Ustr) -> Option<UstrSet>;
    fn get_dependency_sinks(&self) -> UstrSet;
    fn check_cycles(&self) -> Result<(), UnitGraphError>;
    fn generate_dot_graph(&self) -> String;
}

impl UnitGraphFFI for TraneFFI {
    fn add_course(&mut self, course_id: &Ustr) -> Result<(), UnitGraphError> {
        self.trane.add_course(course_id)
    }
    fn add_lesson(&mut self, lesson_id: &Ustr, course_id: &Ustr) -> Result<(), UnitGraphError> {
        self.trane.add_lesson(lesson_id, course_id)
    }
    fn add_exercise(&mut self, exercise_id: &Ustr, lesson_id: &Ustr) -> Result<(), UnitGraphError> {
        self.trane.add_exercise(exercise_id, lesson_id)
    }
    fn add_dependencies(
        &mut self,
        unit_id: &Ustr,
        unit_type: UnitType,
        dependencies: &[Ustr],
    ) -> Result<()> {
        self.trane
            .add_dependencies(unit_id, unit_type.into(), dependencies)
    }
    fn get_unit_type(&self, unit_id: &Ustr) -> Option<UnitType> {
        self.trane.get_unit_type(unit_id).map(Into::into)
    }
    fn get_course_lessons(&self, course_id: &Ustr) -> Option<UstrSet> {
        self.trane.get_course_lessons(course_id)
    }
    fn update_starting_lessons(&mut self) {
        self.trane.update_starting_lessons()
    }
    fn get_starting_lessons(&self, course_id: &Ustr) -> Option<UstrSet> {
        self.trane.get_starting_lessons(course_id)
    }
    fn get_lesson_course(&self, lesson_id: &Ustr) -> Option<Ustr> {
        self.trane.get_lesson_course(lesson_id)
    }
    fn get_lesson_exercises(&self, lesson_id: &Ustr) -> Option<UstrSet> {
        self.trane.get_lesson_exercises(lesson_id)
    }
    fn get_exercise_lesson(&self, exercise_id: &Ustr) -> Option<Ustr> {
        self.trane.get_exercise_lesson(exercise_id)
    }
    fn get_dependencies(&self, unit_id: &Ustr) -> Option<UstrSet> {
        self.trane.get_dependencies(unit_id)
    }
    fn get_dependents(&self, unit_id: &Ustr) -> Option<UstrSet> {
        self.trane.get_dependents(unit_id)
    }
    fn get_dependency_sinks(&self) -> UstrSet {
        self.trane.get_dependency_sinks()
    }
    fn check_cycles(&self) -> Result<(), UnitGraphError> {
        self.trane.check_cycles()
    }
    fn generate_dot_graph(&self) -> String {
        self.trane.generate_dot_graph()
    }
}
