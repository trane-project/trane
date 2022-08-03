//! Trane is an automated learning system for the acquisition of complex and highly hierarchical
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

//! Trane is named after John Coltrane, whose nickname Trane was often used in wordplay with the
//! word train (as in the vehicle) to describe the overwhelming power of his playing. It is used
//! here as a play on its homophone (as in "training a new skill").

pub mod blacklist;
pub mod course_builder;
pub mod course_library;
pub mod data;
pub mod filter_manager;
pub mod graph;
pub mod practice_stats;
pub mod scheduler;
pub mod scorer;

use anyhow::{anyhow, Context, Result};
use parking_lot::RwLock;
use std::{fs::create_dir, path::Path, sync::Arc};
use ustr::{Ustr, UstrSet};

use blacklist::{BlackListDB, Blacklist};
use course_library::{CourseLibrary, GetUnitGraph, LocalCourseLibrary};
use data::{filter::*, *};
use filter_manager::{FilterManager, LocalFilterManager};
use graph::UnitGraph;
use practice_stats::{PracticeStats, PracticeStatsDB};
use scheduler::{data::SchedulerData, DepthFirstScheduler, ExerciseScheduler};

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
    blacklist: Arc<RwLock<dyn Blacklist + Send + Sync>>,

    /// The object containing all the course, lesson, and exercise info.
    course_library: Arc<RwLock<dyn CourseLibrary + Send + Sync>>,

    /// The object containing unit filters saved by the user.
    filter_manager: Arc<RwLock<dyn FilterManager + Send + Sync>>,

    /// The object containing the information on previous exercise trials.
    practice_stats: Arc<RwLock<dyn PracticeStats + Send + Sync>>,

    /// The object containing the scheduling algorithm.
    scheduler: DepthFirstScheduler,

    /// The dependency graph of courses and lessons in the course library.
    unit_graph: Arc<RwLock<dyn UnitGraph + Send + Sync>>,
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
        } else if !trane_path.is_dir() {
            return Err(anyhow!(
                "config path .trane inside library must be a directory"
            ));
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
        let unit_graph = course_library.write().get_unit_graph();
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
            options: SchedulerOptions::default(),
            course_library: course_library.clone(),
            unit_graph: unit_graph.clone(),
            practice_stats: practice_stats.clone(),
            blacklist: blacklist.clone(),
        };

        Ok(Trane {
            blacklist,
            course_library,
            filter_manager,
            library_root: library_root.to_string(),
            practice_stats,
            scheduler: DepthFirstScheduler::new(scheduler_data, SchedulerOptions::default()),
            unit_graph,
        })
    }

    /// Returns the path to the root of the course library.
    pub fn library_root(&self) -> String {
        self.library_root.clone()
    }
}

impl Blacklist for Trane {
    fn add_unit(&mut self, unit_id: &Ustr) -> Result<()> {
        self.blacklist.write().add_unit(unit_id)
    }

    fn remove_unit(&mut self, unit_id: &Ustr) -> Result<()> {
        self.blacklist.write().remove_unit(unit_id)
    }

    fn blacklisted(&self, unit_id: &Ustr) -> Result<bool> {
        self.blacklist.read().blacklisted(unit_id)
    }

    fn all_entries(&self) -> Result<Vec<Ustr>> {
        self.blacklist.read().all_entries()
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
}

impl FilterManager for Trane {
    fn get_filter(&self, id: &str) -> Option<NamedFilter> {
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
}

impl ExerciseScheduler for Trane {
    fn get_exercise_batch(
        &self,
        filter: Option<&UnitFilter>,
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
}

impl UnitGraph for Trane {
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
