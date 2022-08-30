//! Module containing the data used by the scheduler and functions to make it easier to use.
use anyhow::{anyhow, Result};
use parking_lot::RwLock;
use std::sync::Arc;
use ustr::{Ustr, UstrMap, UstrSet};

use crate::{
    blacklist::{BlacklistDB, Blacklist},
    course_library::{CourseLibrary, LocalCourseLibrary},
    data::{
        filter::{FilterOp, MetadataFilter},
        CourseManifest, ExerciseManifest, LessonManifest, SchedulerOptions, UnitType,
    },
    graph::{InMemoryUnitGraph, UnitGraph},
    practice_stats::PracticeStatsDB,
    review_list::ReviewListDB,
};

/// A struct encapsulating all the state needed to schedule exercises.
#[derive(Clone)]
pub(crate) struct SchedulerData {
    /// The options used to run this scheduler.
    pub options: SchedulerOptions,

    /// The course library storing manifests and info about units.
    pub course_library: Arc<RwLock<LocalCourseLibrary>>,

    /// The dependency graph of courses and lessons.
    pub unit_graph: Arc<RwLock<InMemoryUnitGraph>>,

    /// The list of previous exercise results.
    pub practice_stats: Arc<RwLock<PracticeStatsDB>>,

    /// The list of units to skip during scheduling.
    pub blacklist: Arc<RwLock<BlacklistDB>>,

    /// The list of units which should be reviewed by the student.
    pub review_list: Arc<RwLock<ReviewListDB>>,

    /// A map storing the number of times an exercise has been scheduled during the lifetime of this
    /// scheduler.
    pub frequency_map: Arc<RwLock<UstrMap<f32>>>,
}

impl SchedulerData {
    /// Returns the ID of the course to which the lesson with the given ID belongs.
    pub fn get_course_id(&self, lesson_id: &Ustr) -> Result<Ustr> {
        self.unit_graph
            .read()
            .get_lesson_course(lesson_id)
            .ok_or_else(|| anyhow!("missing course ID for lesson with ID {}", lesson_id))
    }

    /// Returns the type of the given unit.
    pub fn get_unit_type(&self, unit_id: &Ustr) -> Result<UnitType> {
        self.unit_graph
            .read()
            .get_unit_type(unit_id)
            .ok_or_else(|| anyhow!("missing unit type for unit with ID {}", unit_id))
    }

    /// Returns the manifest for the course with the given ID.
    pub fn get_course_manifest(&self, course_id: &Ustr) -> Result<CourseManifest> {
        self.course_library
            .read()
            .get_course_manifest(course_id)
            .ok_or_else(|| anyhow!("missing manifest for course with ID {}", course_id))
    }

    /// Returns the manifest for the course with the given ID.
    pub fn get_lesson_manifest(&self, lesson_id: &Ustr) -> Result<LessonManifest> {
        self.course_library
            .read()
            .get_lesson_manifest(lesson_id)
            .ok_or_else(|| anyhow!("missing manifest for lesson with ID {}", lesson_id))
    }

    /// Returns the manifest for the exercise with the given ID.
    pub fn get_exercise_manifest(&self, exercise_id: &Ustr) -> Result<ExerciseManifest> {
        self.course_library
            .read()
            .get_exercise_manifest(exercise_id)
            .ok_or_else(|| anyhow!("missing manifest for exercise with ID {}", exercise_id))
    }

    /// Returns whether the unit with the given ID is blacklisted.
    pub fn blacklisted(&self, unit_id: &Ustr) -> Result<bool> {
        self.blacklist.read().blacklisted(unit_id)
    }

    /// Returns all the units that are dependencies of the unit with the given ID.
    pub fn get_all_dependents(&self, unit_id: &Ustr) -> Vec<Ustr> {
        return self
            .unit_graph
            .read()
            .get_dependents(unit_id)
            .unwrap_or_default()
            .into_iter()
            .collect();
    }

    /// Returns the value of the course_id field in the manifest of the given lesson.
    pub fn get_lesson_course_id(&self, lesson_id: &Ustr) -> Result<Ustr> {
        Ok(self.get_lesson_manifest(lesson_id)?.course_id)
    }

    /// Returns whether the unit exists in the library. Some units will exists in the unit graph
    /// because they are a dependency of another but their data might not exist in the library.
    pub fn unit_exists(&self, unit_id: &Ustr) -> Result<bool, String> {
        let unit_type = self.unit_graph.read().get_unit_type(unit_id);
        if unit_type.is_none() {
            return Ok(false);
        }

        let library = self.course_library.read();
        match unit_type.unwrap() {
            UnitType::Course => match library.get_course_manifest(unit_id) {
                None => Ok(false),
                Some(_) => Ok(true),
            },
            UnitType::Lesson => match library.get_lesson_manifest(unit_id) {
                None => Ok(false),
                Some(_) => Ok(true),
            },
            UnitType::Exercise => match library.get_exercise_manifest(unit_id) {
                None => Ok(false),
                Some(_) => Ok(true),
            },
        }
    }

    /// Returns the exercises contained within the given unit.
    pub fn get_lesson_exercises(&self, unit_id: &Ustr) -> Vec<Ustr> {
        self.unit_graph
            .read()
            .get_lesson_exercises(unit_id)
            .unwrap_or_default()
            .into_iter()
            .collect()
    }

    /// Returns the number of lessons in the given course.
    pub fn get_num_lessons_in_course(&self, course_id: &Ustr) -> i64 {
        let lessons: UstrSet = self
            .unit_graph
            .read()
            .get_course_lessons(course_id)
            .unwrap_or_default();
        lessons.len() as i64
    }

    /// Applies the metadata filter to the given unit.
    pub fn apply_metadata_filter(
        &self,
        unit_id: &Ustr,
        metadata_filter: &MetadataFilter,
    ) -> Result<Option<bool>> {
        let unit_type = self.get_unit_type(unit_id)?;
        match unit_type {
            UnitType::Course => match &metadata_filter.course_filter {
                None => Ok(None),
                Some(filter) => {
                    let manifest = self.get_course_manifest(unit_id)?;
                    Ok(Some(filter.apply(&manifest)))
                }
            },
            UnitType::Lesson => match &metadata_filter.lesson_filter {
                None => Ok(None),
                Some(filter) => {
                    let manifest = self.get_lesson_manifest(unit_id)?;
                    Ok(Some(filter.apply(&manifest)))
                }
            },
            UnitType::Exercise => Err(anyhow!(
                "cannot apply metadata filter to exercise with ID {}",
                unit_id
            )),
        }
    }

    /// Returns whether the unit passes the metadata filter, handling all interactions between
    /// lessons and course metadata filters.
    pub fn unit_passes_filter(
        &self,
        unit_id: &Ustr,
        metadata_filter: Option<&MetadataFilter>,
    ) -> Result<bool> {
        if metadata_filter.is_none() {
            return Ok(true);
        }
        let metadata_filter = metadata_filter.unwrap();

        let unit_type = self.get_unit_type(unit_id)?;
        match unit_type {
            UnitType::Exercise => Err(anyhow!(
                "cannot apply metadata filter to exercise with ID {}",
                unit_id
            )),
            UnitType::Course => {
                let course_passes = self
                    .apply_metadata_filter(unit_id, metadata_filter)
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
                let lesson_manifest = self.get_lesson_manifest(unit_id)?;
                let lesson_passes = self
                    .apply_metadata_filter(unit_id, metadata_filter)
                    .unwrap_or(None);
                let course_passes = self
                    .apply_metadata_filter(&lesson_manifest.course_id, metadata_filter)
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

    /// Increases the value in the frequency map for the given exercise ID.
    pub fn increase_exercise_frequency(&self, exercise_id: &Ustr) {
        let mut frequency_map = self.frequency_map.write();
        let frequency = frequency_map.entry(*exercise_id).or_insert(0.0);
        *frequency += 1.0;
    }

    /// Returns the frequency of the given exercise ID.
    pub fn get_exercise_frequency(&self, exercise_id: &Ustr) -> f32 {
        self.frequency_map
            .read()
            .get(exercise_id)
            .copied()
            .unwrap_or(0.0)
    }
}
