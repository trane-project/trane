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
//! Below is an overview of some of the most important modules in this crate and their purpose.
//! Refer to the documentation of each module for more details.
//!
//! - [`data`]: Contains the basic data structures used by Trane.
//! - [`graph`]: Defines the graph used by Trane to list the units of material and the dependencies
//!   among them.
//! - [`course_library`]: Reads a collection of courses, lessons, and exercises from the file system
//!   and provides basic utilities for working with them.
//! - [`scheduler`]: Defines the algorithm used by Trane to select exercises to present to the user.
//! - [`practice_stats`]: Stores the results of practice sessions for use in determining the next
//!   batch of exercises.
//! - [`blacklist`]: Defines the list of units the student wishes to hide, either because their
//!   material has already been mastered or they do not wish to learn it.
//! - [`exercise_scorer`]: Calculates a score for an exercise based on the results and timestamps of
//!   previous trials.
//!
//>@lp-example-3

#![cfg_attr(coverage, feature(coverage_attribute))]
// Use pedantic warnings but disable some that are not useful.
#![warn(clippy::pedantic)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::float_cmp)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::too_many_lines)]

pub mod blacklist;
pub mod course_builder;
pub mod course_library;
pub mod data;
pub mod error;
pub mod exercise_scorer;
pub mod filter_manager;
pub mod graph;

pub mod practice_rewards;
pub mod practice_stats;
pub mod preferences_manager;
pub mod repository_manager;
pub mod review_list;
pub mod reward_scorer;
pub mod scheduler;
pub mod study_session_manager;
#[cfg_attr(coverage, coverage(off))]
pub mod test_utils;
pub mod transcription_downloader;
pub mod utils;

use anyhow::{Context, Result, bail, ensure};
use error::*;
use parking_lot::RwLock;
use std::{
    fs::{File, create_dir},
    io::Write,
    path::Path,
    sync::Arc,
};
use ustr::{Ustr, UstrMap, UstrSet};

use crate::{
    blacklist::{Blacklist, LocalBlacklist},
    course_library::{CourseLibrary, GetUnitGraph, LocalCourseLibrary, SerializedCourseLibrary},
    data::{
        CourseManifest, ExerciseManifest, ExerciseTrial, LessonManifest, MasteryScore,
        RepositoryMetadata, SchedulerOptions, SchedulerPreferences, UnitType, UserPreferences,
        filter::{ExerciseFilter, SavedFilter},
    },
    filter_manager::{FilterManager, LocalFilterManager},
    graph::UnitGraph,
    practice_rewards::{LocalPracticeRewards, PracticeRewards},
    practice_stats::{LocalPracticeStats, PracticeStats},
    preferences_manager::{LocalPreferencesManager, PreferencesManager},
    repository_manager::{LocalRepositoryManager, RepositoryManager},
    review_list::{LocalReviewList, ReviewList},
    scheduler::{DepthFirstScheduler, ExerciseScheduler, data::SchedulerData},
    study_session_manager::{LocalStudySessionManager, StudySessionManager},
    transcription_downloader::{LocalTranscriptionDownloader, TranscriptionDownloader},
};

/// The path to the folder inside each course library containing the user data.
pub const TRANE_CONFIG_DIR_PATH: &str = ".trane";

/// The path to the `SQLite` database containing the results of previous exercise trials.
pub const PRACTICE_STATS_PATH: &str = "practice_stats.db";

/// The path to the `SQLite` database containing the rewards for lessons and courses.
pub const PRACTICE_REWARDS_PATH: &str = "practice_rewards.db";

/// The path to the `SQLite` database containing the list of units to ignore during scheduling.
pub const BLACKLIST_PATH: &str = "blacklist.db";

/// The path to the `SQLite` database containing the list of units the student wishes to review.
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

    /// The object managing rewards for lessons and courses.
    practice_rewards: Arc<RwLock<dyn PracticeRewards + Send + Sync>>,

    /// The object managing the user preferences.
    preferences_manager: Arc<RwLock<dyn PreferencesManager + Send + Sync>>,

    /// The object managing git repositories containing courses.
    repo_manager: Arc<RwLock<dyn RepositoryManager + Send + Sync>>,

    /// The object managing the list of units to review.
    review_list: Arc<RwLock<dyn ReviewList + Send + Sync>>,

    /// The object managing access to all the data needed by the scheduler. It's saved separately
    /// from the scheduler so that tests can have access to it.
    scheduler_data: SchedulerData,

    /// The object managing the scheduling algorithm.
    scheduler: DepthFirstScheduler,

    /// The object managing the study sessions saved by the user.
    study_session_manager: Arc<RwLock<dyn StudySessionManager + Send + Sync>>,

    /// The dependency graph of courses and lessons in the course library.
    unit_graph: Arc<RwLock<dyn UnitGraph + Send + Sync>>,

    /// An optional instance of the transcription downloader.
    transcription_downloader: Arc<RwLock<dyn TranscriptionDownloader + Send + Sync>>,
}

impl Trane {
    /// Creates the scheduler options, overriding any values with those specified in the user
    /// preferences.
    #[cfg_attr(coverage, coverage(off))]
    fn create_scheduler_options(preferences: Option<&SchedulerPreferences>) -> SchedulerOptions {
        let mut options = SchedulerOptions::default();
        if let Some(preferences) = preferences
            && let Some(batch_size) = preferences.batch_size
        {
            options.batch_size = batch_size;
        }
        options
    }

    /// Initializes the config directory at path `.trane` inside the library root.
    #[cfg_attr(coverage, coverage(off))]
    fn init_config_directory(library_root: &Path) -> Result<()> {
        // Verify that the library root is a directory.
        ensure!(
            library_root.is_dir(),
            "library root {} is not a directory",
            library_root.display(),
        );

        // Create the config folder inside the library root if it does not exist already.
        let trane_path = library_root.join(TRANE_CONFIG_DIR_PATH);
        if !trane_path.exists() {
            create_dir(trane_path.clone()).context("failed to create config directory")?;
        } else if !trane_path.is_dir() {
            bail!("config path .trane inside library must be a directory");
        }

        // Create the `filters` directory if it does not exist already.
        let filters_path = trane_path.join(FILTERS_DIR);
        if !filters_path.is_dir() {
            create_dir(filters_path.clone()).context("failed to create filters directory")?;
        }

        // Create the `study_sessions` directory if it does not exist already.
        let sessions_path = trane_path.join(STUDY_SESSIONS_DIR);
        if !sessions_path.is_dir() {
            create_dir(sessions_path.clone())
                .context("failed to create study_sessions directory")?;
        }

        // Create the user preferences file if it does not exist already.
        let user_prefs_path = trane_path.join(USER_PREFERENCES_PATH);
        if !user_prefs_path.exists() {
            let mut file = File::create(user_prefs_path.clone())
                .context("failed to create user_preferences.json file")?;
            let default_prefs = UserPreferences::default();
            let prefs_json = serde_json::to_string_pretty(&default_prefs)? + "\n";
            file.write_all(prefs_json.as_bytes())
                .context("failed to write to user_preferences.json file")?;
        } else if !user_prefs_path.is_file() {
            bail!("user preferences file must be a regular file");
        }
        Ok(())
    }

    /// A helper function to create all the components needed to create a Trane instance that takes
    /// the already created preferences manager and course library as parameters.
    #[cfg_attr(coverage, coverage(off))]
    fn new_local_helper(
        library_root: &Path,
        preferences_manager: Arc<RwLock<LocalPreferencesManager>>,
        course_library: Arc<RwLock<LocalCourseLibrary>>,
    ) -> Result<Trane> {
        let config_path = library_root.join(Path::new(TRANE_CONFIG_DIR_PATH));
        let user_preferences = preferences_manager.read().get_user_preferences()?;
        let transcription_preferences = user_preferences.transcription.clone().unwrap_or_default();
        let transcription_downloader = Arc::new(RwLock::new(LocalTranscriptionDownloader {
            preferences: transcription_preferences,
            link_store: course_library.clone(),
        }));
        let unit_graph = course_library.write().get_unit_graph();
        let practice_stats = Arc::new(RwLock::new(LocalPracticeStats::new_from_disk(
            config_path.join(PRACTICE_STATS_PATH).to_str().unwrap(),
        )?));
        let practice_rewards = Arc::new(RwLock::new(LocalPracticeRewards::new_from_disk(
            config_path.join(PRACTICE_REWARDS_PATH).to_str().unwrap(),
        )?));
        let blacklist = Arc::new(RwLock::new(LocalBlacklist::new_from_disk(
            config_path.join(BLACKLIST_PATH).to_str().unwrap(),
        )?));
        let review_list = Arc::new(RwLock::new(LocalReviewList::new_from_disk(
            config_path.join(REVIEW_LIST_PATH).to_str().unwrap(),
        )?));
        let filter_manager = Arc::new(RwLock::new(LocalFilterManager::new(
            config_path.join(FILTERS_DIR).to_str().unwrap(),
        )?));
        let study_sessions_manager = Arc::new(RwLock::new(LocalStudySessionManager::new(
            config_path.join(STUDY_SESSIONS_DIR).to_str().unwrap(),
        )?));
        let repo_manager = Arc::new(RwLock::new(LocalRepositoryManager::new(library_root)?));
        let options = Self::create_scheduler_options(user_preferences.scheduler.as_ref());
        options.verify()?;
        let scheduler_data = SchedulerData {
            options,
            course_library: course_library.clone(),
            unit_graph: unit_graph.clone(),
            practice_stats: practice_stats.clone(),
            practice_rewards: practice_rewards.clone(),
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
            practice_rewards,
            preferences_manager,
            repo_manager,
            review_list,
            scheduler_data: scheduler_data.clone(),
            scheduler: DepthFirstScheduler::new(scheduler_data),
            study_session_manager: study_sessions_manager,
            unit_graph,
            transcription_downloader,
        })
    }

    /// Creates a new local instance of the Trane given the path to the root of a course library.
    /// The user data will be stored in a directory named `.trane` inside the library root
    /// directory. The working directory will be used to resolve relative paths.
    #[cfg_attr(coverage, coverage(off))]
    pub fn new_local(working_dir: &Path, library_root: &Path) -> Result<Trane> {
        // Initialize the config directory.
        Self::init_config_directory(library_root)?;

        // Build all the preferences manager and course library.
        let preferences_manager = Arc::new(RwLock::new(LocalPreferencesManager {
            path: library_root
                .join(TRANE_CONFIG_DIR_PATH)
                .join(USER_PREFERENCES_PATH),
        }));
        let user_preferences = preferences_manager.read().get_user_preferences()?;
        let course_library = Arc::new(RwLock::new(LocalCourseLibrary::new(
            &working_dir.join(library_root),
            user_preferences.clone(),
        )?));

        // Call the helper function to create the rest of the components.
        Self::new_local_helper(library_root, preferences_manager, course_library)
    }

    /// Creates a new local instance of the Trane given the path to the root of a course library.
    /// The user data will be stored in a directory named `.trane` inside the library root
    /// directory. The working directory will be used to resolve relative paths.
    #[cfg_attr(coverage, coverage(off))]
    pub fn new_local_from_serialized(
        library_root: &Path,
        serialized_library: SerializedCourseLibrary,
    ) -> Result<Trane> {
        // Initialize the config directory.
        Self::init_config_directory(library_root)?;

        // Build all the preferences manager and course library. The course library will be
        // initialized from the serialized data.
        let preferences_manager = Arc::new(RwLock::new(LocalPreferencesManager {
            path: library_root
                .join(TRANE_CONFIG_DIR_PATH)
                .join(USER_PREFERENCES_PATH),
        }));
        let user_preferences = preferences_manager.read().get_user_preferences()?;
        let course_library = Arc::new(RwLock::new(LocalCourseLibrary::new_from_serialized(
            serialized_library,
            user_preferences.clone(),
        )?));

        // Call the helper function to create the rest of the components.
        Self::new_local_helper(library_root, preferences_manager, course_library)
    }

    /// Returns the path to the root of the course library.
    pub fn library_root(&self) -> String {
        self.library_root.clone()
    }

    /// Returns a clone of the data used by the scheduler. This function is needed by tests that
    /// need to verify internal methods.
    #[allow(dead_code)]
    fn get_scheduler_data(&self) -> SchedulerData {
        self.scheduler_data.clone()
    }
}

#[cfg_attr(coverage, coverage(off))]
impl Blacklist for Trane {
    fn add_to_blacklist(&mut self, unit_id: Ustr) -> Result<(), BlacklistError> {
        // Make sure to invalidate any cached scores for the given unit.
        self.scheduler.invalidate_cached_score(unit_id);
        self.blacklist.write().add_to_blacklist(unit_id)
    }

    fn remove_from_blacklist(&mut self, unit_id: Ustr) -> Result<(), BlacklistError> {
        // Make sure to invalidate any cached scores for the given unit.
        self.scheduler.invalidate_cached_score(unit_id);
        self.blacklist.write().remove_from_blacklist(unit_id)
    }

    fn remove_prefix_from_blacklist(&mut self, prefix: &str) -> Result<(), BlacklistError> {
        // Make sure to invalidate any cached scores of units with the given prefix.
        self.scheduler.invalidate_cached_scores_with_prefix(prefix);
        self.blacklist.write().remove_prefix_from_blacklist(prefix)
    }

    fn blacklisted(&self, unit_id: Ustr) -> Result<bool, BlacklistError> {
        self.blacklist.read().blacklisted(unit_id)
    }

    fn get_blacklist_entries(&self) -> Result<Vec<Ustr>, BlacklistError> {
        self.blacklist.read().get_blacklist_entries()
    }
}

#[cfg_attr(coverage, coverage(off))]
impl CourseLibrary for Trane {
    fn get_course_manifest(&self, course_id: Ustr) -> Option<CourseManifest> {
        self.course_library.read().get_course_manifest(course_id)
    }

    fn get_lesson_manifest(&self, lesson_id: Ustr) -> Option<LessonManifest> {
        self.course_library.read().get_lesson_manifest(lesson_id)
    }

    fn get_exercise_manifest(&self, exercise_id: Ustr) -> Option<ExerciseManifest> {
        self.course_library
            .read()
            .get_exercise_manifest(exercise_id)
    }

    fn get_course_ids(&self) -> Vec<Ustr> {
        self.course_library.read().get_course_ids()
    }

    fn get_lesson_ids(&self, course_id: Ustr) -> Option<Vec<Ustr>> {
        self.course_library.read().get_lesson_ids(course_id)
    }

    fn get_exercise_ids(&self, lesson_id: Ustr) -> Option<Vec<Ustr>> {
        self.course_library.read().get_exercise_ids(lesson_id)
    }

    fn get_all_exercise_ids(&self, unit_id: Option<Ustr>) -> Vec<Ustr> {
        self.course_library.read().get_all_exercise_ids(unit_id)
    }

    fn get_matching_prefix(&self, prefix: &str, unit_type: Option<UnitType>) -> UstrSet {
        self.course_library
            .read()
            .get_matching_prefix(prefix, unit_type)
    }

    fn search(&self, query: &str) -> Result<Vec<Ustr>, CourseLibraryError> {
        self.course_library.read().search(query)
    }
}

#[cfg_attr(coverage, coverage(off))]
impl ExerciseScheduler for Trane {
    fn get_exercise_batch(
        &self,
        filter: Option<ExerciseFilter>,
    ) -> Result<Vec<ExerciseManifest>, ExerciseSchedulerError> {
        self.scheduler.get_exercise_batch(filter)
    }

    fn score_exercise(
        &self,
        exercise_id: Ustr,
        score: MasteryScore,
        timestamp: i64,
    ) -> Result<(), ExerciseSchedulerError> {
        self.scheduler.score_exercise(exercise_id, score, timestamp)
    }

    fn get_unit_score(&self, unit_id: Ustr) -> Result<Option<f32>, ExerciseSchedulerError> {
        self.scheduler.get_unit_score(unit_id)
    }

    fn invalidate_cached_score(&self, unit_id: Ustr) {
        self.scheduler.invalidate_cached_score(unit_id);
    }

    fn invalidate_cached_scores_with_prefix(&self, prefix: &str) {
        self.scheduler.invalidate_cached_scores_with_prefix(prefix);
    }

    fn get_scheduler_options(&self) -> SchedulerOptions {
        self.scheduler.get_scheduler_options()
    }

    fn set_scheduler_options(&mut self, options: SchedulerOptions) {
        self.scheduler.set_scheduler_options(options);
    }

    fn reset_scheduler_options(&mut self) {
        self.scheduler.reset_scheduler_options();
    }
}

#[cfg_attr(coverage, coverage(off))]
impl FilterManager for Trane {
    fn get_filter(&self, id: &str) -> Option<SavedFilter> {
        self.filter_manager.read().get_filter(id)
    }

    fn list_filters(&self) -> Vec<(String, String)> {
        self.filter_manager.read().list_filters()
    }
}

#[cfg_attr(coverage, coverage(off))]
impl PracticeRewards for Trane {
    fn get_rewards(
        &self,
        unit_id: Ustr,
        num_rewards: usize,
    ) -> Result<Vec<data::UnitReward>, PracticeRewardsError> {
        self.practice_rewards
            .read()
            .get_rewards(unit_id, num_rewards)
    }

    fn record_unit_reward(
        &mut self,
        unit_id: Ustr,
        reward: &data::UnitReward,
    ) -> Result<bool, PracticeRewardsError> {
        self.practice_rewards
            .write()
            .record_unit_reward(unit_id, reward)
    }

    fn trim_rewards(&mut self, num_rewards: usize) -> Result<(), PracticeRewardsError> {
        self.practice_rewards.write().trim_rewards(num_rewards)
    }

    fn remove_rewards_with_prefix(&mut self, prefix: &str) -> Result<(), PracticeRewardsError> {
        self.practice_rewards
            .write()
            .remove_rewards_with_prefix(prefix)
    }
}

#[cfg_attr(coverage, coverage(off))]
impl PracticeStats for Trane {
    fn get_scores(
        &self,
        exercise_id: Ustr,
        num_scores: usize,
    ) -> Result<Vec<ExerciseTrial>, PracticeStatsError> {
        self.practice_stats
            .read()
            .get_scores(exercise_id, num_scores)
    }

    fn record_exercise_score(
        &mut self,
        exercise_id: Ustr,
        score: MasteryScore,
        timestamp: i64,
    ) -> Result<(), PracticeStatsError> {
        self.practice_stats
            .write()
            .record_exercise_score(exercise_id, score, timestamp)
    }

    fn trim_scores(&mut self, num_scores: usize) -> Result<(), PracticeStatsError> {
        self.practice_stats.write().trim_scores(num_scores)
    }

    fn remove_scores_with_prefix(&mut self, prefix: &str) -> Result<(), PracticeStatsError> {
        self.practice_stats
            .write()
            .remove_scores_with_prefix(prefix)
    }
}
#[cfg_attr(coverage, coverage(off))]
impl PreferencesManager for Trane {
    fn get_user_preferences(&self) -> Result<UserPreferences, PreferencesManagerError> {
        self.preferences_manager.read().get_user_preferences()
    }

    fn set_user_preferences(
        &mut self,
        preferences: UserPreferences,
    ) -> Result<(), PreferencesManagerError> {
        self.preferences_manager
            .write()
            .set_user_preferences(preferences)
    }
}

#[cfg_attr(coverage, coverage(off))]
impl RepositoryManager for Trane {
    fn add_repo(
        &mut self,
        url: &str,
        repo_id: Option<String>,
    ) -> Result<(), RepositoryManagerError> {
        self.repo_manager.write().add_repo(url, repo_id)
    }

    fn remove_repo(&mut self, repo_id: &str) -> Result<(), RepositoryManagerError> {
        self.repo_manager.write().remove_repo(repo_id)
    }

    fn update_repo(&self, repo_id: &str) -> Result<(), RepositoryManagerError> {
        self.repo_manager.read().update_repo(repo_id)
    }

    fn update_all_repos(&self) -> Result<(), RepositoryManagerError> {
        self.repo_manager.read().update_all_repos()
    }

    fn list_repos(&self) -> Vec<RepositoryMetadata> {
        self.repo_manager.read().list_repos()
    }
}

#[cfg_attr(coverage, coverage(off))]
impl ReviewList for Trane {
    fn add_to_review_list(&mut self, unit_id: Ustr) -> Result<(), ReviewListError> {
        self.review_list.write().add_to_review_list(unit_id)
    }

    fn remove_from_review_list(&mut self, unit_id: Ustr) -> Result<(), ReviewListError> {
        self.review_list.write().remove_from_review_list(unit_id)
    }

    fn get_review_list_entries(&self) -> Result<Vec<Ustr>, ReviewListError> {
        self.review_list.read().get_review_list_entries()
    }
}

#[cfg_attr(coverage, coverage(off))]
impl StudySessionManager for Trane {
    fn get_study_session(&self, id: &str) -> Option<data::filter::StudySession> {
        self.study_session_manager.read().get_study_session(id)
    }

    fn list_study_sessions(&self) -> Vec<(String, String)> {
        self.study_session_manager.read().list_study_sessions()
    }
}

#[cfg_attr(coverage, coverage(off))]
impl TranscriptionDownloader for Trane {
    fn is_transcription_asset_downloaded(&self, exercise_id: Ustr) -> bool {
        self.transcription_downloader
            .read()
            .is_transcription_asset_downloaded(exercise_id)
    }

    fn download_transcription_asset(
        &self,
        exercise_id: Ustr,
        force_download: bool,
    ) -> Result<(), TranscriptionDownloaderError> {
        self.transcription_downloader
            .write()
            .download_transcription_asset(exercise_id, force_download)
    }

    fn transcription_download_path(&self, exercise_id: Ustr) -> Option<std::path::PathBuf> {
        self.transcription_downloader
            .read()
            .transcription_download_path(exercise_id)
    }

    fn transcription_download_path_alias(&self, exercise_id: Ustr) -> Option<std::path::PathBuf> {
        self.transcription_downloader
            .read()
            .transcription_download_path_alias(exercise_id)
    }
}

#[cfg_attr(coverage, coverage(off))]
impl UnitGraph for Trane {
    fn add_course(&mut self, course_id: Ustr) -> Result<(), UnitGraphError> {
        self.unit_graph.write().add_course(course_id)
    }

    fn add_lesson(&mut self, lesson_id: Ustr, course_id: Ustr) -> Result<(), UnitGraphError> {
        self.unit_graph.write().add_lesson(lesson_id, course_id)
    }

    fn add_exercise(&mut self, exercise_id: Ustr, lesson_id: Ustr) -> Result<(), UnitGraphError> {
        self.unit_graph.write().add_exercise(exercise_id, lesson_id)
    }

    fn add_dependencies(
        &mut self,
        unit_id: Ustr,
        unit_type: UnitType,
        dependencies: &[Ustr],
    ) -> Result<(), UnitGraphError> {
        self.unit_graph
            .write()
            .add_dependencies(unit_id, unit_type, dependencies)
    }

    fn add_encompassed(
        &mut self,
        unit_id: Ustr,
        dependencies: &[Ustr],
        encompassed: &[(Ustr, f32)],
    ) -> Result<(), UnitGraphError> {
        self.unit_graph
            .write()
            .add_encompassed(unit_id, dependencies, encompassed)
    }

    fn set_encompasing_equals_dependency(&mut self) {
        self.unit_graph.write().encompasing_equals_dependency();
    }

    fn encompasing_equals_dependency(&self) -> bool {
        self.unit_graph.read().encompasing_equals_dependency()
    }

    fn add_superseded(&mut self, unit_id: Ustr, superseded: &[Ustr]) {
        self.unit_graph.write().add_superseded(unit_id, superseded);
    }

    fn get_unit_type(&self, unit_id: Ustr) -> Option<UnitType> {
        self.unit_graph.read().get_unit_type(unit_id)
    }

    fn get_course_lessons(&self, course_id: Ustr) -> Option<UstrSet> {
        self.unit_graph.read().get_course_lessons(course_id)
    }

    fn get_starting_lessons(&self, course_id: Ustr) -> Option<UstrSet> {
        self.unit_graph.read().get_starting_lessons(course_id)
    }

    fn update_starting_lessons(&mut self) {
        self.unit_graph.write().update_starting_lessons();
    }

    fn get_lesson_course(&self, lesson_id: Ustr) -> Option<Ustr> {
        self.unit_graph.read().get_lesson_course(lesson_id)
    }

    fn get_lesson_exercises(&self, lesson_id: Ustr) -> Option<UstrSet> {
        self.unit_graph.read().get_lesson_exercises(lesson_id)
    }

    fn get_exercise_lesson(&self, exercise_id: Ustr) -> Option<Ustr> {
        self.unit_graph.read().get_exercise_lesson(exercise_id)
    }

    fn get_dependencies(&self, unit_id: Ustr) -> Option<UstrSet> {
        self.unit_graph.read().get_dependencies(unit_id)
    }

    fn get_dependents(&self, unit_id: Ustr) -> Option<UstrSet> {
        self.unit_graph.read().get_dependents(unit_id)
    }

    fn get_encompasses(&self, unit_id: Ustr) -> Option<Vec<(Ustr, f32)>> {
        self.unit_graph.read().get_encompasses(unit_id)
    }

    fn get_encompassed_by(&self, unit_id: Ustr) -> Option<Vec<(Ustr, f32)>> {
        self.unit_graph.read().get_encompassed_by(unit_id)
    }

    fn get_dependency_sinks(&self) -> UstrSet {
        self.unit_graph.read().get_dependency_sinks()
    }

    fn get_supersedes(&self, unit_id: Ustr) -> Option<UstrSet> {
        self.unit_graph.read().get_supersedes(unit_id)
    }

    fn get_superseded_by(&self, unit_id: Ustr) -> Option<UstrSet> {
        self.unit_graph.read().get_superseded_by(unit_id)
    }

    fn check_cycles(&self) -> Result<(), UnitGraphError> {
        self.unit_graph.read().check_cycles()
    }

    fn generate_dot_graph(&self, courses_only: bool) -> String {
        self.unit_graph.read().generate_dot_graph(courses_only)
    }
}

unsafe impl Send for Trane {}
unsafe impl Sync for Trane {}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use anyhow::Result;
    use std::{fs::*, os::unix::prelude::PermissionsExt};

    use crate::{
        FILTERS_DIR, STUDY_SESSIONS_DIR, TRANE_CONFIG_DIR_PATH, Trane, USER_PREFERENCES_PATH,
        data::{SchedulerOptions, SchedulerPreferences, UserPreferences},
    };

    /// Verifies retrieving the root of a library.
    #[test]
    fn library_root() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let trane = Trane::new_local(dir.path(), dir.path())?;
        assert_eq!(trane.library_root(), dir.path().to_str().unwrap());
        Ok(())
    }

    /// Verifies opening a course library with a path that is not a directory fails.
    #[test]
    fn library_root_is_not_dir() -> Result<()> {
        let file = tempfile::NamedTempFile::new()?;
        let result = Trane::new_local(file.path(), file.path());
        assert!(result.is_err());
        Ok(())
    }

    /// Verifies that opening a library if the path to the config directory exists but is not a
    /// directory.
    #[test]
    fn config_dir_is_file() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let trane_path = dir.path().join(".trane");
        File::create(trane_path)?;
        assert!(Trane::new_local(dir.path(), dir.path()).is_err());
        Ok(())
    }

    /// Verifies that opening a library fails if the directory has bad permissions.
    #[test]
    fn bad_dir_permissions() -> Result<()> {
        let dir = tempfile::tempdir()?;
        set_permissions(&dir, Permissions::from_mode(0o000))?;
        assert!(Trane::new_local(dir.path(), dir.path()).is_err());
        Ok(())
    }

    /// Verifies that opening a library fails if the config directory has bad permissions.
    #[test]
    fn bad_config_dir_permissions() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let config_dir_path = dir.path().join(".trane");
        create_dir(&config_dir_path)?;
        set_permissions(&config_dir_path, Permissions::from_mode(0o000))?;
        assert!(Trane::new_local(dir.path(), dir.path()).is_err());
        Ok(())
    }

    /// Verifies that opening a library fails if the user preferences file is not a file.
    #[test]
    fn user_preferences_file_is_a_dir() -> Result<()> {
        // Create directory `./trane/user_preferences.json` which is not a file.
        let temp_dir = tempfile::tempdir()?;
        std::fs::create_dir_all(
            temp_dir
                .path()
                .join(TRANE_CONFIG_DIR_PATH)
                .join(USER_PREFERENCES_PATH),
        )?;
        assert!(Trane::new_local(temp_dir.path(), temp_dir.path()).is_err());
        Ok(())
    }

    /// Verifies that opening a library fails if the filters directory cannot be created.
    #[test]
    fn cannot_create_filters_directory() -> Result<()> {
        // Create config directory.
        let temp_dir = tempfile::tempdir()?;
        let config_dir = temp_dir.path().join(TRANE_CONFIG_DIR_PATH);
        create_dir(config_dir.clone())?;

        // Set permissions of `.trane` directory to read-only.
        std::fs::set_permissions(temp_dir.path(), std::fs::Permissions::from_mode(0o444))?;

        assert!(Trane::new_local(temp_dir.path(), temp_dir.path()).is_err());
        Ok(())
    }

    /// Verifies that opening a library fails if the study sessions directory cannot be created.
    #[test]
    fn cannot_create_study_sessions() -> Result<()> {
        // Create config and filters directories.
        let temp_dir = tempfile::tempdir()?;
        let config_dir = temp_dir.path().join(TRANE_CONFIG_DIR_PATH);
        create_dir(config_dir.clone())?;
        let filters_dir = config_dir.join(FILTERS_DIR);
        create_dir(filters_dir)?;

        // Set permissions of `.trane` directory to read-only. This should prevent Trane from
        // creating the user preferences file.
        std::fs::set_permissions(config_dir, std::fs::Permissions::from_mode(0o500))?;
        assert!(Trane::new_local(temp_dir.path(), temp_dir.path()).is_err());
        Ok(())
    }

    /// Verifies that opening a library fails if the user preferences file cannot be created.
    #[test]
    fn cannot_create_user_preferences() -> Result<()> {
        // Create config, filters, and study sessions directories.
        let temp_dir = tempfile::tempdir()?;
        let config_dir = temp_dir.path().join(TRANE_CONFIG_DIR_PATH);
        create_dir(config_dir.clone())?;
        let filters_dir = config_dir.join(FILTERS_DIR);
        create_dir(filters_dir)?;
        let sessions_dir = config_dir.join(STUDY_SESSIONS_DIR);
        create_dir(sessions_dir)?;

        // Set permissions of `.trane` directory to read-only. This should prevent Trane from
        // creating the user preferences file.
        std::fs::set_permissions(config_dir, std::fs::Permissions::from_mode(0o500))?;
        assert!(Trane::new_local(temp_dir.path(), temp_dir.path()).is_err());
        Ok(())
    }

    /// Verifies retrieving the scheduler data from the library.
    #[test]
    fn scheduler_data() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let trane = Trane::new_local(dir.path(), dir.path())?;
        trane.get_scheduler_data();
        Ok(())
    }

    /// Verifies building the scheduler options from the user preferences.
    #[test]
    fn scheduler_options() {
        // Test with no preferences.
        let user_preferences = UserPreferences {
            scheduler: None,
            transcription: None,
            ignored_paths: vec![],
        };
        let options = Trane::create_scheduler_options(user_preferences.scheduler.as_ref());
        assert_eq!(options.batch_size, SchedulerOptions::default().batch_size);

        // Test with preferences.
        let user_preferences = UserPreferences {
            scheduler: Some(SchedulerPreferences {
                batch_size: Some(10),
            }),
            transcription: None,
            ignored_paths: vec![],
        };
        let options = Trane::create_scheduler_options(user_preferences.scheduler.as_ref());
        assert_eq!(options.batch_size, 10);
    }
}
