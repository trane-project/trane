pub mod blacklist;
pub mod course_builder;
pub mod course_library;
pub mod data;
pub mod filter_manager;
pub mod graph;
pub mod practice_stats;
pub mod scheduler;
pub mod scorer;

use std::{
    fs::create_dir,
    path::Path,
    sync::{Arc, RwLock},
};

use anyhow::{anyhow, Context, Result};
use blacklist::{BlackListDB, Blacklist};
use course_library::{CourseLibrary, GetUnitGraph, LocalCourseLibrary};
use data::{filter::*, *};
use filter_manager::{FilterManager, LocalFilterManager};
use graph::{DebugUnitGraph, UnitGraph};
use practice_stats::{PracticeStats, PracticeStatsDB};
use scheduler::{DepthFirstScheduler, ExerciseScheduler, SchedulerData};

/// The path to the folder inside each course library containing the user data.
const TRANE_CONFIG_DIR_PATH: &str = ".trane";

/// The path to the SQLite database containing the results of previous exercise trials.
const PRACTICE_STATS_PATH: &str = "practice_stats.db";

/// The path to the SQLite database containing the list of units to be skipped during scheduling.
const BLACKLIST_PATH: &str = "blacklist.db";

/// The path to the directory containing unit filters saved by the user.
const FILTERS_DIR: &str = "filters";

/// Trane is a library for the acquisition of highly hierarchical knowledge and skills based on the
/// principles of mastery learning and spaced repetition. Given a list of courses, its lessons and
/// correspondings exercises, Trane presents the student with a list of exercises based on the
/// demonstrated mastery of previous exercises. It makes sures that new material and skills are not
/// introduced until the prerequisite material and skills have been sufficiently mastered.
pub struct Trane {
    /// The path to the root of the course library.
    library_root: String,

    /// The object containing the list of courses, lessons, and exercises to be skipped.
    blacklist: Arc<RwLock<dyn Blacklist>>,

    /// The object containing all the course, lesson, and exercise info.
    course_library: Arc<RwLock<dyn CourseLibrary>>,

    /// The object containing unit filters saved by the user.
    filter_manager: Arc<RwLock<dyn FilterManager>>,

    /// The object containing the information on previous exercise trials.
    practice_stats: Arc<RwLock<dyn PracticeStats>>,

    /// The object containing the scheduling algorithm.
    scheduler: DepthFirstScheduler,

    /// The dependency graph of courses and lessons in the course library.
    unit_graph: Arc<RwLock<dyn UnitGraph>>,
}

impl Trane {
    /// Initializes the config directory at path .trane inside the library root.
    fn init_config_directory(library_root: &str) -> Result<()> {
        let root_path = Path::new(library_root);
        if !root_path.is_dir() {
            return Err(anyhow!("library_root must be the path to a directory"));
        }

        // Create the config folder inside the library root if it does not exist already.
        let trane_path = root_path.join(TRANE_CONFIG_DIR_PATH);
        if !trane_path.exists() {
            create_dir(trane_path.clone()).with_context(|| {
                format!(
                    "failed to create config directory at {}",
                    trane_path.display()
                )
            })?;
        } else {
            if !trane_path.is_dir() {
                return Err(anyhow!(
                    "config path .trane inside library must be a directory"
                ));
            }
        }

        // Create the filters directory if it doesn't exist.
        let filters_path = trane_path.join(FILTERS_DIR);
        if !filters_path.is_dir() {
            create_dir(filters_path.clone()).with_context(|| {
                format!(
                    "failed to create filters directory at {}",
                    filters_path.display()
                )
            })?;
        }

        Ok(())
    }

    /// Creates a new entrance of the library given the path to the root of a course library.
    /// The user data will be stored in a directory named .trane inside the library root directory.
    pub fn new(library_root: &str) -> Result<Trane> {
        Self::init_config_directory(library_root)?;
        let config_path = Path::new(library_root).join(Path::new(TRANE_CONFIG_DIR_PATH));

        let course_library = Arc::new(RwLock::new(LocalCourseLibrary::new(library_root)?));
        let unit_graph = course_library.read().unwrap().get_unit_graph();
        let practice_stats = Arc::new(RwLock::new(PracticeStatsDB::new_from_disk(
            config_path.join(PRACTICE_STATS_PATH).to_str().unwrap(),
        )?));
        let blacklist = Arc::new(RwLock::new(BlackListDB::new_from_disk(
            config_path.join(BLACKLIST_PATH).to_str().unwrap(),
        )?));
        let filter_manager = Arc::new(RwLock::new(LocalFilterManager::new(
            config_path.join(FILTERS_DIR).to_str().unwrap(),
        )?));
        let scheduler_data = SchedulerData {
            course_library: course_library.clone(),
            unit_graph: unit_graph.clone(),
            practice_stats: practice_stats.clone(),
            blacklist: blacklist.clone(),
        };

        Ok(Trane {
            blacklist: blacklist,
            course_library: course_library,
            filter_manager: filter_manager,
            library_root: library_root.to_string(),
            practice_stats: practice_stats,
            scheduler: DepthFirstScheduler::new(scheduler_data, SchedulerOptions::default()),
            unit_graph: unit_graph,
        })
    }

    pub fn library_root(&self) -> String {
        self.library_root.clone()
    }
}

impl Blacklist for Trane {
    fn add_unit(&mut self, unit_id: &str) -> Result<()> {
        self.blacklist.write().unwrap().add_unit(unit_id)
    }

    fn remove_unit(&mut self, unit_id: &str) -> Result<()> {
        self.blacklist.write().unwrap().remove_unit(unit_id)
    }

    fn blacklisted(&self, unit_id: &str) -> Result<bool> {
        self.blacklist.read().unwrap().blacklisted(unit_id)
    }

    fn all_entries(&self) -> Result<Vec<String>> {
        self.blacklist.read().unwrap().all_entries()
    }
}

impl CourseLibrary for Trane {
    fn get_course_manifest(&self, course_id: &str) -> Option<CourseManifest> {
        self.course_library
            .read()
            .unwrap()
            .get_course_manifest(course_id)
    }

    fn get_lesson_manifest(&self, lesson_id: &str) -> Option<LessonManifest> {
        self.course_library
            .read()
            .unwrap()
            .get_lesson_manifest(lesson_id)
    }

    fn get_exercise_manifest(&self, exercise_id: &str) -> Option<ExerciseManifest> {
        self.course_library
            .read()
            .unwrap()
            .get_exercise_manifest(exercise_id)
    }
}

impl FilterManager for Trane {
    fn get_filter(&self, id: &str) -> Option<NamedFilter> {
        self.filter_manager.read().unwrap().get_filter(id)
    }

    fn list_filters(&self) -> Vec<(String, String)> {
        self.filter_manager.read().unwrap().list_filters()
    }
}

impl PracticeStats for Trane {
    fn get_scores(&self, exercise_id: &str, num_scores: usize) -> Result<Vec<ExerciseTrial>> {
        self.practice_stats
            .read()
            .unwrap()
            .get_scores(exercise_id, num_scores)
    }

    fn record_exercise_score(
        &mut self,
        exercise_id: &str,
        score: MasteryScore,
        timestamp: i64,
    ) -> Result<()> {
        self.practice_stats
            .write()
            .unwrap()
            .record_exercise_score(exercise_id, score, timestamp)
    }
}

impl ExerciseScheduler for Trane {
    fn set_options(&self, options: SchedulerOptions) {
        self.scheduler.set_options(options);
    }

    fn get_exercise_batch(
        &self,
        filter: Option<&UnitFilter>,
    ) -> Result<Vec<(String, ExerciseManifest)>> {
        self.scheduler.get_exercise_batch(filter)
    }

    fn record_exercise_score(
        &self,
        exercise_id: &str,
        score: MasteryScore,
        timestamp: i64,
    ) -> Result<()> {
        self.scheduler
            .record_exercise_score(exercise_id, score, timestamp)
    }
}

impl DebugUnitGraph for Trane {
    fn get_uid(&self, unit_id: &str) -> Option<u64> {
        self.unit_graph.read().unwrap().get_uid(unit_id)
    }

    fn get_id(&self, unit_uid: u64) -> Option<String> {
        self.unit_graph.read().unwrap().get_id(unit_uid)
    }

    fn get_unit_type(&self, unit_uid: u64) -> Option<UnitType> {
        self.unit_graph.read().unwrap().get_unit_type(unit_uid)
    }
}
