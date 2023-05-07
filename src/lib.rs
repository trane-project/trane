//! Trane is an automated practice system for the acquisition of complex and highly hierarchical
//! skills. It is based on the principles of spaced repetition, mastery learning, and chunking.
//!
//! Given a set of exercises which have been bundled into lessons and further bundled in courses, as
//! well as the dependency relationships between those lessons and courses, Trane selects exercises
//! to present to the user. It makes sure that exercises from a course or lesson are not presented
//! to the user until the exercises in their dependencies have been sufficiently mastered. It also
//! makes sure to keep the balance of exercises so that the difficulty of the exercises lies
//! slightly outside the user's current mastery.
//!
//! You can think of this process as progressing through the skill tree of a character in a video
//! game, but applied to arbitrary skills, which are defined in plain-text files which define the
//! exercises, their bundling into lessons and courses, and the dependency relationships between
//! them.
//!
//! Trane is named after John Coltrane, whose nickname Trane was often used in wordplay with the
//! word train (as in the vehicle) to describe the overwhelming power of his playing. It is used
//! here as a play on its homophone (as in "training a new skill").
//!
//@<lp-example-3
//! Here's an overview of some of the most important modules in this crate and their purpose:
//! - [data](crate::data): Contains the basic data structures used by Trane.
//! - [graph](crate::graph): Defines the graph used by Trane to list the units of material and the
//!   dependencies among them.
//! - [course_library](crate::course_library): Reads a collection of courses, lessons, and exercises
//!   from the file system and provides basic utilities for working with them.
//! - [scheduler](crate::scheduler): Defines the algorithm used by Trane to select exercises to
//!   present to the user.
//! - [practice_stats](crate::practice_stats): Stores the results of practice sessions for use in
//!   determining the next batch of exercises.
//! - [blacklist](crate::blacklist): Defines the list of units the student wishes to hide, either
//!   because their material has already been mastered or they do not wish to learn it.
//! - [scorer](crate::scorer): Calculates a score for an exercise based on the results and
//!   timestamps of previous trials.
//>@lp-example-3

pub mod blacklist;
pub mod course_builder;
pub mod course_library;
pub mod data;
pub mod error;
pub mod filter_manager;
pub mod graph;
pub mod mantra_miner;
pub mod practice_stats;
pub mod repository_manager;
pub mod review_list;
pub mod scheduler;
pub mod scorer;
pub mod study_session_manager;
pub mod testutil;

use anyhow::Result;
use parking_lot::RwLock;
use review_list::{ReviewList, ReviewListDB};
use std::{path::Path, sync::Arc};
use study_session_manager::{LocalStudySessionManager, StudySessionManager};
use ustr::{Ustr, UstrMap, UstrSet};

use crate::mantra_miner::TraneMantraMiner;
use blacklist::{Blacklist, BlacklistDB};
use course_library::{CourseLibrary, GetUnitGraph, LocalCourseLibrary};
use data::{
    filter::SavedFilter, CourseManifest, ExerciseManifest, ExerciseTrial, LessonManifest,
    MasteryScore, SchedulerOptions, SchedulerPreferences, UnitType, UserPreferences,
};
use filter_manager::{FilterManager, LocalFilterManager};
use graph::UnitGraph;
use practice_stats::{PracticeStats, PracticeStatsDB};
use repository_manager::{LocalRepositoryManager, RepositoryManager};
use scheduler::{data::SchedulerData, DepthFirstScheduler, ExerciseFilter, ExerciseScheduler};

/// The path to the folder inside each course library containing the user data.
pub const TRANE_CONFIG_DIR_PATH: &str = ".trane";

/// The path to the SQLite database containing the results of previous exercise trials.
pub const PRACTICE_STATS_PATH: &str = "practice_stats.db";

/// The path to the SQLite database containing the list of units to ignore during scheduling.
pub const BLACKLIST_PATH: &str = "blacklist.db";

/// The path to the SQLite database containing the list of units the student wishes to review.
pub const REVIEW_LIST_PATH: &str = "review_list.db";

/// The path to the directory containing unit filters saved by the user.
pub const FILTERS_DIR: &str = "filters";

/// The path to the directory containing study sessions saved by the user.
pub const STUDY_SESSIONS_DIR: &str = "study_sessions";

/// The path to the file containing user preferences.
pub const USER_PREFERENCES_PATH: &str = "user_preferences.json";

/// The name of the directory where repositories will be downloaded.
const DOWNLOAD_DIRECTORY: &str = "managed_courses";

/// The name of the directory where the details on all repositories will be stored. This directory
/// will be created under the `.trane` directory at the root of the Trane library.
const REPOSITORY_DIRECTORY: &str = "repositories";

/// Trane is a library for the acquisition of highly hierarchical knowledge and skills based on the
/// principles of mastery learning and spaced repetition. Given a list of courses, its lessons and
/// corresponding exercises, Trane presents the student with a list of exercises based on the
/// demonstrated mastery of previous exercises. It makes sure that new material and skills are not
/// introduced until the prerequisite material and skills have been sufficiently mastered.
pub struct Trane {
    /// The path to the root of the course library.
    library_root: String,

    /// The object managing the list of courses, lessons, and exercises to be skipped.
    blacklist: Arc<RwLock<dyn Blacklist + Send + Sync>>,

    /// The object managing all the course, lesson, and exercise info.
    course_library: Arc<RwLock<dyn CourseLibrary + Send + Sync>>,

    /// The object managing unit filters saved by the user.
    filter_manager: Arc<RwLock<dyn FilterManager + Send + Sync>>,

    /// The object managing the information on previous exercise trials.
    practice_stats: Arc<RwLock<dyn PracticeStats + Send + Sync>>,

    /// The object managing git repositories containing courses.
    repo_manager: Arc<RwLock<dyn RepositoryManager + Send + Sync>>,

    /// The object managing the list of units to review.
    review_list: Arc<RwLock<dyn ReviewList + Send + Sync>>,

    /// The object managing access to all the data needed by the scheduler. It's saved separately
    /// from the scheduler so that tests can have access to it.
    #[allow(dead_code)]
    scheduler_data: SchedulerData,

    /// The object managing the scheduling algorithm.
    scheduler: DepthFirstScheduler,

    /// The object managing the study sessions saved by the user.
    study_session_manager: Arc<RwLock<dyn StudySessionManager + Send + Sync>>,

    /// The dependency graph of courses and lessons in the course library.
    unit_graph: Arc<RwLock<dyn UnitGraph + Send + Sync>>,

    /// An instance of the mantra miner that "recites" Tara Sarasvati's mantra while Trane runs.
    mantra_miner: TraneMantraMiner,
}

impl Trane {
    /// Creates the scheduler options, overriding any values with those specified in the user
    /// preferences.
    fn create_scheduler_options(preferences: &Option<SchedulerPreferences>) -> SchedulerOptions {
        let mut options = SchedulerOptions::default();
        if let Some(preferences) = preferences {
            if let Some(batch_size) = preferences.batch_size {
                options.batch_size = batch_size;
            }
        }
        options
    }

    /// Creates a new instance of the library given the path to the root of a course library. The
    /// user data will be stored in a directory named `.trane` inside the library root directory.
    /// The working directory will be used to resolve relative paths.
    pub fn new(working_dir: &Path, library_root: &Path) -> Result<Trane> {
        let config_path = library_root.join(Path::new(TRANE_CONFIG_DIR_PATH));

        // The course library must be created first because it makes sure to initialize all the
        // required directories if they are missing.
        let course_library = Arc::new(RwLock::new(LocalCourseLibrary::new(
            &working_dir.join(library_root),
        )?));

        // Build all the components needed to create a Trane instance.
        let unit_graph = course_library.write().get_unit_graph();
        let practice_stats = Arc::new(RwLock::new(PracticeStatsDB::new_from_disk(
            config_path.join(PRACTICE_STATS_PATH).to_str().unwrap(),
        )?));
        let blacklist = Arc::new(RwLock::new(BlacklistDB::new_from_disk(
            config_path.join(BLACKLIST_PATH).to_str().unwrap(),
        )?));
        let review_list = Arc::new(RwLock::new(ReviewListDB::new_from_disk(
            config_path.join(REVIEW_LIST_PATH).to_str().unwrap(),
        )?));
        let filter_manager = Arc::new(RwLock::new(LocalFilterManager::new(
            config_path.join(FILTERS_DIR).to_str().unwrap(),
        )?));
        let study_sessions_manager = Arc::new(RwLock::new(LocalStudySessionManager::new(
            config_path.join(STUDY_SESSIONS_DIR).to_str().unwrap(),
        )?));
        let repo_manager = Arc::new(RwLock::new(LocalRepositoryManager::new(library_root)?));
        let mut mantra_miner = TraneMantraMiner::default();
        mantra_miner.mantra_miner.start()?;

        // Build the scheduler options and data.
        let user_preferences = course_library.read().get_user_preferences();
        let options = Self::create_scheduler_options(&user_preferences.scheduler);
        options.verify()?;
        let scheduler_data = SchedulerData {
            options,
            course_library: course_library.clone(),
            unit_graph: unit_graph.clone(),
            practice_stats: practice_stats.clone(),
            blacklist: blacklist.clone(),
            review_list: review_list.clone(),
            filter_manager: filter_manager.clone(),
            frequency_map: Arc::new(RwLock::new(UstrMap::default())),
        };

        Ok(Trane {
            blacklist,
            course_library,
            filter_manager,
            library_root: library_root.to_str().unwrap().to_string(),
            practice_stats,
            repo_manager,
            review_list,
            scheduler_data: scheduler_data.clone(),
            scheduler: DepthFirstScheduler::new(scheduler_data),
            study_session_manager: study_sessions_manager,
            unit_graph,
            mantra_miner,
        })
    }

    /// Returns the path to the root of the course library.
    pub fn library_root(&self) -> String {
        self.library_root.clone()
    }

    /// Returns the number of mantras that have been recited by the mantra miner.
    pub fn mantra_count(&self) -> usize {
        self.mantra_miner.mantra_miner.count()
    }

    /// Returns a clone of the data used by the scheduler. This function is needed by tests that
    /// need to verify internal methods.
    #[allow(dead_code)]
    fn get_scheduler_data(&self) -> SchedulerData {
        self.scheduler_data.clone()
    }
}

// grcov-excl-start: The following implementation blocks simply expose the interfaces already
// implemented and tested by the various submodules. Therefore, the next line excludes this section
// from the final report.

impl Blacklist for Trane {
    fn add_to_blacklist(&mut self, unit_id: &Ustr) -> Result<()> {
        // Make sure to invalidate any cached scores for the given unit.
        self.scheduler.invalidate_cached_score(unit_id);
        self.blacklist.write().add_to_blacklist(unit_id)
    }

    fn remove_from_blacklist(&mut self, unit_id: &Ustr) -> Result<()> {
        // Make sure to invalidate any cached scores for the given unit.
        self.scheduler.invalidate_cached_score(unit_id);
        self.blacklist.write().remove_from_blacklist(unit_id)
    }

    fn blacklisted(&self, unit_id: &Ustr) -> Result<bool> {
        self.blacklist.read().blacklisted(unit_id)
    }

    fn all_blacklist_entries(&self) -> Result<Vec<Ustr>> {
        self.blacklist.read().all_blacklist_entries()
    }
}

impl CourseLibrary for Trane {
    fn get_course_manifest(&self, course_id: &Ustr) -> Option<CourseManifest> {
        self.course_library.read().get_course_manifest(course_id)
    }

    fn get_lesson_manifest(&self, lesson_id: &Ustr) -> Option<LessonManifest> {
        self.course_library.read().get_lesson_manifest(lesson_id)
    }

    fn get_exercise_manifest(&self, exercise_id: &Ustr) -> Option<ExerciseManifest> {
        self.course_library
            .read()
            .get_exercise_manifest(exercise_id)
    }

    fn get_course_ids(&self) -> Vec<Ustr> {
        self.course_library.read().get_course_ids()
    }

    fn get_lesson_ids(&self, course_id: &Ustr) -> Result<Vec<Ustr>> {
        self.course_library.read().get_lesson_ids(course_id)
    }

    fn get_exercise_ids(&self, lesson_id: &Ustr) -> Result<Vec<Ustr>> {
        self.course_library.read().get_exercise_ids(lesson_id)
    }

    fn get_all_exercise_ids(&self) -> Result<Vec<Ustr>> {
        self.course_library.read().get_all_exercise_ids()
    }

    fn search(&self, query: &str) -> Result<Vec<Ustr>> {
        self.course_library.read().search(query)
    }

    fn get_user_preferences(&self) -> UserPreferences {
        self.course_library.read().get_user_preferences()
    }
}

impl FilterManager for Trane {
    fn get_filter(&self, id: &str) -> Option<SavedFilter> {
        self.filter_manager.read().get_filter(id)
    }

    fn list_filters(&self) -> Vec<(String, String)> {
        self.filter_manager.read().list_filters()
    }
}

impl PracticeStats for Trane {
    fn get_scores(&self, exercise_id: &Ustr, num_scores: usize) -> Result<Vec<ExerciseTrial>> {
        self.practice_stats
            .read()
            .get_scores(exercise_id, num_scores)
    }

    fn record_exercise_score(
        &mut self,
        exercise_id: &Ustr,
        score: MasteryScore,
        timestamp: i64,
    ) -> Result<()> {
        self.practice_stats
            .write()
            .record_exercise_score(exercise_id, score, timestamp)
    }

    fn trim_scores(&mut self, num_scores: usize) -> Result<()> {
        self.practice_stats.write().trim_scores(num_scores)
    }
}

impl RepositoryManager for Trane {
    fn add_repo(&mut self, url: &str, repo_id: Option<String>) -> Result<()> {
        self.repo_manager.write().add_repo(url, repo_id)
    }

    fn remove_repo(&mut self, repo_id: &str) -> Result<()> {
        self.repo_manager.write().remove_repo(repo_id)
    }

    fn update_repo(&self, repo_id: &str) -> Result<()> {
        self.repo_manager.read().update_repo(repo_id)
    }

    fn update_all_repos(&self) -> Result<()> {
        self.repo_manager.read().update_all_repos()
    }

    fn list_repos(&self) -> Result<Vec<data::RepositoryMetadata>> {
        self.repo_manager.read().list_repos()
    }
}

impl ReviewList for Trane {
    fn add_to_review_list(&mut self, unit_id: &Ustr) -> Result<()> {
        self.review_list.write().add_to_review_list(unit_id)
    }

    fn remove_from_review_list(&mut self, unit_id: &Ustr) -> Result<()> {
        self.review_list.write().remove_from_review_list(unit_id)
    }

    fn all_review_list_entries(&self) -> Result<Vec<Ustr>> {
        self.review_list.read().all_review_list_entries()
    }
}

impl ExerciseScheduler for Trane {
    fn get_exercise_batch(
        &self,
        filter: Option<ExerciseFilter>,
    ) -> Result<Vec<(Ustr, ExerciseManifest)>> {
        self.scheduler.get_exercise_batch(filter)
    }

    fn score_exercise(
        &self,
        exercise_id: &Ustr,
        score: MasteryScore,
        timestamp: i64,
    ) -> Result<()> {
        self.scheduler.score_exercise(exercise_id, score, timestamp)
    }

    fn invalidate_cached_score(&self, unit_id: &Ustr) {
        self.scheduler.invalidate_cached_score(unit_id)
    }

    fn get_scheduler_options(&self) -> SchedulerOptions {
        self.scheduler.get_scheduler_options()
    }

    fn set_scheduler_options(&mut self, options: SchedulerOptions) {
        self.scheduler.set_scheduler_options(options)
    }

    fn reset_scheduler_options(&mut self) {
        self.scheduler.reset_scheduler_options()
    }
}

impl StudySessionManager for Trane {
    fn get_study_session(&self, id: &str) -> Option<data::filter::StudySession> {
        self.study_session_manager.read().get_study_session(id)
    }

    fn list_study_sessions(&self) -> Vec<(String, String)> {
        self.study_session_manager.read().list_study_sessions()
    }
}

impl UnitGraph for Trane {
    fn add_course(&mut self, course_id: &Ustr) -> Result<()> {
        self.unit_graph.write().add_course(course_id)
    }

    fn add_lesson(&mut self, lesson_id: &Ustr, course_id: &Ustr) -> Result<()> {
        self.unit_graph.write().add_lesson(lesson_id, course_id)
    }

    fn add_exercise(&mut self, exercise_id: &Ustr, lesson_id: &Ustr) -> Result<()> {
        self.unit_graph.write().add_exercise(exercise_id, lesson_id)
    }

    fn add_dependencies(
        &mut self,
        unit_id: &Ustr,
        unit_type: UnitType,
        dependencies: &[Ustr],
    ) -> Result<()> {
        self.unit_graph
            .write()
            .add_dependencies(unit_id, unit_type, dependencies)
    }

    fn get_unit_type(&self, unit_id: &Ustr) -> Option<UnitType> {
        self.unit_graph.read().get_unit_type(unit_id)
    }

    fn get_course_lessons(&self, course_id: &Ustr) -> Option<UstrSet> {
        self.unit_graph.read().get_course_lessons(course_id)
    }

    fn get_course_starting_lessons(&self, course_id: &Ustr) -> Option<UstrSet> {
        self.unit_graph
            .read()
            .get_course_starting_lessons(course_id)
    }

    fn update_starting_lessons(&mut self) {
        self.unit_graph.write().update_starting_lessons()
    }

    fn get_lesson_course(&self, lesson_id: &Ustr) -> Option<Ustr> {
        self.unit_graph.read().get_lesson_course(lesson_id)
    }

    fn get_lesson_exercises(&self, lesson_id: &Ustr) -> Option<UstrSet> {
        self.unit_graph.read().get_lesson_exercises(lesson_id)
    }

    fn get_exercise_lesson(&self, exercise_id: &Ustr) -> Option<Ustr> {
        self.unit_graph.read().get_exercise_lesson(exercise_id)
    }

    fn get_dependencies(&self, unit_id: &Ustr) -> Option<UstrSet> {
        self.unit_graph.read().get_dependencies(unit_id)
    }

    fn get_dependents(&self, unit_id: &Ustr) -> Option<UstrSet> {
        self.unit_graph.read().get_dependents(unit_id)
    }

    fn get_dependency_sinks(&self) -> UstrSet {
        self.unit_graph.read().get_dependency_sinks()
    }

    fn check_cycles(&self) -> Result<()> {
        self.unit_graph.read().check_cycles()
    }

    fn generate_dot_graph(&self) -> String {
        self.unit_graph.read().generate_dot_graph()
    }
}

// grcov-excl-stop

#[cfg(test)]
mod test {
    use anyhow::Result;
    use std::{fs::*, os::unix::prelude::PermissionsExt, thread, time::Duration};

    use crate::{
        data::{SchedulerOptions, SchedulerPreferences, UserPreferences},
        Trane,
    };

    /// Verifies retrieving the root of a library.
    #[test]
    fn library_root() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let trane = Trane::new(dir.path(), dir.path())?;
        assert_eq!(trane.library_root(), dir.path().to_str().unwrap());
        Ok(())
    }

    /// Verifies that the mantra-miner starts up and has a valid count.
    #[test]
    fn mantra_count() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let trane = Trane::new(dir.path(), dir.path())?;
        thread::sleep(Duration::from_millis(1000));
        assert!(trane.mantra_count() > 0);
        Ok(())
    }

    /// Verifies that opening a library if the path to the config directory exists but is not a
    /// directory.
    #[test]
    fn config_dir_is_file() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let trane_path = dir.path().join(".trane");
        File::create(&trane_path)?;
        assert!(Trane::new(dir.path(), dir.path()).is_err());
        Ok(())
    }

    /// Verifies that opening a library fails if the directory has bad permissions.
    #[test]
    fn bad_dir_permissions() -> Result<()> {
        let dir = tempfile::tempdir()?;
        set_permissions(&dir, Permissions::from_mode(0o000))?;
        assert!(Trane::new(dir.path(), dir.path()).is_err());
        Ok(())
    }

    /// Verifies that opening a library fails if the config directory has bad permissions.
    #[test]
    fn bad_config_dir_permissions() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let config_dir_path = dir.path().join(".trane");
        create_dir(&config_dir_path)?;
        set_permissions(&config_dir_path, Permissions::from_mode(0o000))?;
        assert!(Trane::new(dir.path(), dir.path()).is_err());
        Ok(())
    }

    /// Verifies retrieving the scheduler data from the library.
    #[test]
    fn scheduler_data() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let trane = Trane::new(dir.path(), dir.path())?;
        trane.get_scheduler_data();
        Ok(())
    }

    /// Verifies building the scheduler options from the user preferences.
    #[test]
    fn scheduler_options() -> Result<()> {
        // Test with no preferences.
        let user_preferences = UserPreferences {
            scheduler: None,
            improvisation: None,
            transcription: None,
            ignored_paths: vec![],
        };
        let options = Trane::create_scheduler_options(&user_preferences.scheduler);
        assert_eq!(options.batch_size, SchedulerOptions::default().batch_size);

        // Test with preferences.
        let user_preferences = UserPreferences {
            scheduler: Some(SchedulerPreferences {
                batch_size: Some(10),
            }),
            improvisation: None,
            transcription: None,
            ignored_paths: vec![],
        };
        let options = Trane::create_scheduler_options(&user_preferences.scheduler);
        assert_eq!(options.batch_size, 10);
        Ok(())
    }
}
