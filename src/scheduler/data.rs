//! Module containing the data used by the scheduler and functions to make it easier to use.
use anyhow::{anyhow, Result};
use parking_lot::RwLock;
use std::{collections::HashSet, sync::Arc};

use crate::{
    blacklist::Blacklist,
    course_library::CourseLibrary,
    data::{
        filter::{FilterOp, MetadataFilter},
        CourseManifest, ExerciseManifest, LessonManifest, SchedulerOptions, UnitType,
    },
    graph::UnitGraph,
    practice_stats::PracticeStats,
};

/// A struct encapsulating all the state needed to schedule exercises.
#[derive(Clone)]
pub(crate) struct SchedulerData {
    /// The options used to run this scheduler.
    pub options: SchedulerOptions,

    /// The course library storing manifests and info about units.
    pub course_library: Arc<RwLock<dyn CourseLibrary + Send + Sync>>,

    /// The dependency graph of courses and lessons.
    pub unit_graph: Arc<RwLock<dyn UnitGraph + Send + Sync>>,

    /// The list of previous exercise results.
    pub practice_stats: Arc<RwLock<dyn PracticeStats + Send + Sync>>,

    /// The list of units to skip during scheduling.
    pub blacklist: Arc<RwLock<dyn Blacklist + Send + Sync>>,
}

impl SchedulerData {
    /// Returns the UID of the lesson with the given ID.
    pub fn get_uid(&self, unit_id: &str) -> Result<u64> {
        self.unit_graph
            .read()
            .get_uid(unit_id)
            .ok_or_else(|| anyhow!("missing UID for unit with ID {}", unit_id))
    }

    /// Returns the ID of the lesson with the given UID.
    pub fn get_id(&self, unit_uid: u64) -> Result<String> {
        self.unit_graph
            .read()
            .get_id(unit_uid)
            .ok_or_else(|| anyhow!("missing ID for unit with UID {}", unit_uid))
    }

    /// Returns the uid of the course to which the lesson with the given UID belongs.
    pub fn get_course_uid(&self, lesson_uid: u64) -> Result<u64> {
        self.unit_graph
            .read()
            .get_lesson_course(lesson_uid)
            .ok_or_else(|| anyhow!("missing course UID for lesson with UID {}", lesson_uid))
    }

    /// Returns the type of the given unit.
    pub fn get_unit_type(&self, unit_uid: u64) -> Result<UnitType> {
        self.unit_graph
            .read()
            .get_unit_type(unit_uid)
            .ok_or_else(|| anyhow!("missing unit type for unit with UID {}", unit_uid))
    }

    /// Returns the manifest for the course with the given UID.
    pub fn get_course_manifest(&self, course_uid: u64) -> Result<CourseManifest> {
        let course_id = self.get_id(course_uid)?;
        self.course_library
            .read()
            .get_course_manifest(&course_id)
            .ok_or_else(|| anyhow!("missing manifest for course with ID {}", course_id))
    }

    /// Returns the manifest for the course with the given UID.
    pub fn get_lesson_manifest(&self, lesson_uid: u64) -> Result<LessonManifest> {
        let lesson_id = self.get_id(lesson_uid)?;
        self.course_library
            .read()
            .get_lesson_manifest(&lesson_id)
            .ok_or_else(|| anyhow!("missing manifest for lesson with ID {}", lesson_id))
    }

    /// Returns the manifest for the exercise with the given UID.
    pub fn get_exercise_manifest(&self, exercise_uid: u64) -> Result<ExerciseManifest> {
        let exercise_id = self.get_id(exercise_uid)?;
        self.course_library
            .read()
            .get_exercise_manifest(&exercise_id)
            .ok_or_else(|| anyhow!("missing manifest for exercise with ID {}", exercise_id))
    }

    /// Returns whether the unit with the given UID is blacklisted.
    pub fn blacklisted_uid(&self, unit_uid: u64) -> Result<bool> {
        let unit_id = self.get_id(unit_uid)?;
        self.blacklist.read().blacklisted(&unit_id)
    }

    /// Returns whether the unit with the given ID is blacklisted.
    pub fn blacklisted_id(&self, unit_id: &str) -> Result<bool> {
        self.blacklist.read().blacklisted(unit_id)
    }

    /// Returns all the units that are dependencies of the unit with the given UID.
    pub fn get_all_dependents(&self, unit_uid: u64) -> Vec<u64> {
        return self
            .unit_graph
            .read()
            .get_dependents(unit_uid)
            .unwrap_or_default()
            .into_iter()
            .collect();
    }

    /// Returns the value of the course_id field in the manifest of the given lesson.
    pub fn get_lesson_course_id(&self, lesson_uid: u64) -> Result<String> {
        Ok(self.get_lesson_manifest(lesson_uid)?.course_id)
    }

    /// Returns whether the unit exists in the library. Some units will exists in the unit graph
    /// because they are a dependency of another but their data might not exist in the library.
    pub fn unit_exists(&self, unit_uid: u64) -> Result<bool, String> {
        let unit_id = self.unit_graph.read().get_id(unit_uid);
        if unit_id.is_none() {
            return Ok(false);
        }
        let unit_type = self.unit_graph.read().get_unit_type(unit_uid);
        if unit_type.is_none() {
            return Ok(false);
        }

        let library = self.course_library.read();
        match unit_type.unwrap() {
            UnitType::Course => match library.get_course_manifest(&unit_id.unwrap()) {
                None => Ok(false),
                Some(_) => Ok(true),
            },
            UnitType::Lesson => match library.get_lesson_manifest(&unit_id.unwrap()) {
                None => Ok(false),
                Some(_) => Ok(true),
            },
            UnitType::Exercise => match library.get_exercise_manifest(&unit_id.unwrap()) {
                None => Ok(false),
                Some(_) => Ok(true),
            },
        }
    }

    /// Returns the exercises contained within the given unit.
    pub fn get_lesson_exercises(&self, unit_uid: u64) -> Vec<u64> {
        self.unit_graph
            .read()
            .get_lesson_exercises(unit_uid)
            .unwrap_or_default()
            .into_iter()
            .collect()
    }

    /// Returns the number of lessons in the given course.
    pub fn get_num_lessons_in_course(&self, course_uid: u64) -> i64 {
        let lessons: HashSet<u64> = self
            .unit_graph
            .read()
            .get_course_lessons(course_uid)
            .unwrap_or_default();
        lessons.len() as i64
    }

    /// Applies the metadata filter to the given unit.
    pub fn apply_metadata_filter(
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
    pub fn unit_passes_filter(
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
                    // There's no lesson nor course filter, so the course passes the filter.
                    (None, None) => Ok(true),
                    //  There's only a lesson filter. Return false so that the the course is skipped
                    //  and the decision is made based on the lesson.
                    (Some(_), None) => Ok(false),
                    // There's only a course filter, so return whether the course passed the filter.
                    (None, Some(_)) => Ok(course_passes.unwrap_or(false)),
                    // There's both a lesson and course filter. The behavior depends on the logical
                    // op used in the filter.
                    (Some(_), Some(_)) => match metadata_filter.op {
                        // If the op is All, return whether the course passed the filter.
                        FilterOp::All => Ok(course_passes.unwrap_or(false)),
                        // If the op is Any, return false so that the course is skipped and the
                        // decision is made based on the lesson.
                        FilterOp::Any => Ok(false),
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
                    // There's no lesson nor course filter, so the lesson passes the filter.
                    (None, None) => Ok(true),
                    // There's only a lesson filter, so return whether the lesson passed the filter.
                    (Some(_), None) => Ok(lesson_passes.unwrap_or(false)),
                    // There's only a course filter, so return whether the course passed the filter.
                    (None, Some(_)) => Ok(course_passes.unwrap_or(false)),
                    // There's both a lesson and course filter. The behavior depends on the logical
                    // op used in the filter.
                    (Some(_), Some(_)) => match metadata_filter.op {
                        // If the op is All, return whether the lesson and the course passed the
                        // filters.
                        FilterOp::All => {
                            Ok(lesson_passes.unwrap_or(false) && course_passes.unwrap_or(false))
                        }
                        // If the op is Any, return whether the lesson or the course passed the
                        // filter.
                        FilterOp::Any => {
                            Ok(lesson_passes.unwrap_or(false) || course_passes.unwrap_or(false))
                        }
                    },
                }
            }
        }
    }
}
