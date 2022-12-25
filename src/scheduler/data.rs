//! Defines the data used by the scheduler and several convenience functions.

use anyhow::{anyhow, Result};
use parking_lot::RwLock;
use std::sync::Arc;
use ustr::{Ustr, UstrMap, UstrSet};

use crate::{
    blacklist::{Blacklist, BlacklistDB},
    course_library::{CourseLibrary, LocalCourseLibrary},
    data::{
        filter::{MetadataFilter, UnitFilter},
        CourseManifest, ExerciseManifest, LessonManifest, SchedulerOptions, UnitType,
    },
    graph::{InMemoryUnitGraph, UnitGraph},
    practice_stats::PracticeStatsDB,
    review_list::ReviewListDB,
};

/// A struct encapsulating all the state needed by the scheduler.
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
    /// scheduler. The value is used to give more weight in the scorer to exercises that have been
    /// scheduled less often.
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

    /// Returns whether the unit exists in the library. Some units will exist in the unit graph
    /// because they are a dependency of another, but their data might not exist in the library.
    pub fn unit_exists(&self, unit_id: &Ustr) -> Result<bool> {
        // Retrieve the unit type. A missing unit type indicates the unit does not exist.
        let unit_type = self.unit_graph.read().get_unit_type(unit_id);
        if unit_type.is_none() {
            return Ok(false);
        }

        // Decide whether the unit exists by looking for its manifest.
        let library = self.course_library.read();
        match unit_type.unwrap() {
            UnitType::Course => Ok(library.get_course_manifest(unit_id).is_some()),
            UnitType::Lesson => Ok(library.get_lesson_manifest(unit_id).is_some()),
            UnitType::Exercise => Ok(library.get_exercise_manifest(unit_id).is_some()),
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

    /// Returns whether the unit passes the metadata filter, handling all interactions between
    /// lessons and course metadata filters.
    pub fn unit_passes_filter(
        &self,
        unit_id: &Ustr,
        metadata_filter: Option<&MetadataFilter>,
    ) -> Result<bool> {
        // All units pass if there is no filter.
        if metadata_filter.is_none() {
            return Ok(true);
        }

        // Decide how to handle the filter based on the unit type.
        let unit_type = self.get_unit_type(unit_id)?;
        match unit_type {
            // Exercises do not have metadata, so this operation is not supported.
            UnitType::Exercise => Err(anyhow!(
                "cannot apply metadata filter to exercise with ID {}",
                unit_id
            )),
            UnitType::Course => {
                // Retrieve the course manifest and check if the course passes the filter.
                let course_manifest = self.get_course_manifest(unit_id)?;
                Ok(UnitFilter::course_passes_metadata_filter(
                    metadata_filter.as_ref().unwrap(),
                    &course_manifest,
                ))
            }
            UnitType::Lesson => {
                // Retrieve the lesson and course manifests and check if the lesson passes the
                // filter.
                let course_manifest =
                    self.get_course_manifest(&self.get_lesson_course_id(unit_id)?)?;
                let lesson_manifest = self.get_lesson_manifest(unit_id)?;
                Ok(UnitFilter::lesson_passes_metadata_filter(
                    metadata_filter.as_ref().unwrap(),
                    &course_manifest,
                    &lesson_manifest,
                ))
            }
        }
    }

    /// Increments the value in the frequency map for the given exercise ID.
    pub fn increment_exercise_frequency(&self, exercise_id: &Ustr) {
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

#[cfg(test)]
mod test {
    use anyhow::Result;
    use lazy_static::lazy_static;
    use std::collections::BTreeMap;
    use ustr::Ustr;

    use crate::{
        data::{
            filter::{FilterOp, MetadataFilter},
            UnitType,
        },
        testutil::*,
    };

    static NUM_EXERCISES: usize = 2;

    lazy_static! {
        /// A simple set of courses to test the basic functionality of Trane.
        static ref TEST_LIBRARY: Vec<TestCourse> = vec![
            TestCourse {
                id: TestId(0, None, None),
                dependencies: vec![],
                metadata: BTreeMap::from([
                    (
                        "course_key_1".to_string(),
                        vec!["course_key_1:value_1".to_string()]
                    ),
                    (
                        "course_key_2".to_string(),
                        vec!["course_key_2:value_1".to_string()]
                    ),
                ]),
                lessons: vec![
                    TestLesson {
                        id: TestId(0, Some(0), None),
                        dependencies: vec![],
                        metadata: BTreeMap::from([
                            (
                                "lesson_key_1".to_string(),
                                vec!["lesson_key_1:value_1".to_string()]
                            ),
                            (
                                "lesson_key_2".to_string(),
                                vec!["lesson_key_2:value_1".to_string()]
                            ),
                        ]),
                        num_exercises: NUM_EXERCISES,
                    },
                    TestLesson {
                        id: TestId(0, Some(1), None),
                        dependencies: vec![TestId(0, Some(0), None)],
                        metadata: BTreeMap::from([
                            (
                                "lesson_key_1".to_string(),
                                vec!["lesson_key_1:value_2".to_string()]
                            ),
                            (
                                "lesson_key_2".to_string(),
                                vec!["lesson_key_2:value_2".to_string()]
                            ),
                        ]),
                        num_exercises: NUM_EXERCISES,
                    },
                ],
            },
        ];
    }

    /// Verifies that the scheduler data correctly knows which units exist and their types.
    #[test]
    fn unit_exists() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let library = init_simulation(&temp_dir.path(), &TEST_LIBRARY)?;
        let scheduler_data = library.get_scheduler_data();

        assert_eq!(
            scheduler_data.get_unit_type(&Ustr::from("0"))?,
            UnitType::Course
        );
        assert!(scheduler_data.unit_exists(&Ustr::from("0"))?);
        assert_eq!(
            scheduler_data.get_unit_type(&Ustr::from("0::0"))?,
            UnitType::Lesson
        );
        assert!(scheduler_data.unit_exists(&Ustr::from("0::0"))?);
        assert_eq!(
            scheduler_data.get_unit_type(&Ustr::from("0::0::0"))?,
            UnitType::Exercise
        );
        assert!(scheduler_data.unit_exists(&Ustr::from("0::0::0"))?);
        Ok(())
    }

    /// Verifies that a metadata filter cannot be applied to an exercise.
    #[test]
    fn exercise_metadata_filter() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let library = init_simulation(&temp_dir.path(), &TEST_LIBRARY)?;
        let scheduler_data = library.get_scheduler_data();
        let metadata_filter = MetadataFilter {
            course_filter: None,
            lesson_filter: None,
            op: FilterOp::Any,
        };
        assert!(scheduler_data
            .unit_passes_filter(&Ustr::from("0::0::0"), Some(&metadata_filter))
            .is_err());
        Ok(())
    }

    /// Verifies that the frequency of an exercise is correctly incremented when the exercise is
    /// scheduled.
    #[test]
    fn exercise_frequency() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let library = init_simulation(&temp_dir.path(), &TEST_LIBRARY)?;
        let scheduler_data = library.get_scheduler_data();

        assert_eq!(
            scheduler_data.get_exercise_frequency(&Ustr::from("0::0::0")),
            0.0
        );
        scheduler_data.increment_exercise_frequency(&Ustr::from("0::0::0"));
        assert_eq!(
            scheduler_data.get_exercise_frequency(&Ustr::from("0::0::0")),
            1.0
        );
        Ok(())
    }
}
