//! Defines the data used by the scheduler and several convenience functions.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::sync::Arc;
use ustr::{Ustr, UstrMap, UstrSet};

use crate::{
    blacklist::{Blacklist, BlacklistDB},
    course_library::{CourseLibrary, LocalCourseLibrary},
    data::{
        filter::{KeyValueFilter, SavedFilter, SessionPart, StudySessionData, UnitFilter},
        CourseManifest, ExerciseManifest, LessonManifest, SchedulerOptions, UnitType,
    },
    filter_manager::{FilterManager, LocalFilterManager},
    graph::{InMemoryUnitGraph, UnitGraph},
    practice_stats::PracticeStatsDB,
    review_list::ReviewListDB,
};

/// A struct encapsulating all the state needed by the scheduler.
#[derive(Clone)]
pub struct SchedulerData {
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

    /// The manager used to access unit filters saved by the user.
    pub filter_manager: Arc<RwLock<LocalFilterManager>>,

    /// A map storing the number of times an exercise has been scheduled during the lifetime of this
    /// scheduler. The value is used to give more weight in the scorer to exercises that have been
    /// scheduled less often.
    pub frequency_map: Arc<RwLock<UstrMap<f32>>>,
}

impl SchedulerData {
    /// Returns the ID of the course to which the lesson with the given ID belongs.
    #[inline(always)]
    pub fn get_course_id(&self, lesson_id: &Ustr) -> Result<Ustr> {
        self.unit_graph
            .read()
            .get_lesson_course(lesson_id)
            .ok_or_else(|| anyhow!("missing course ID for lesson with ID {}", lesson_id))
    }

    /// Returns the type of the given unit.
    #[inline(always)]
    pub fn get_unit_type(&self, unit_id: &Ustr) -> Result<UnitType> {
        self.unit_graph
            .read()
            .get_unit_type(unit_id)
            .ok_or_else(|| anyhow!("missing unit type for unit with ID {}", unit_id))
    }

    /// Returns the manifest for the course with the given ID.
    #[inline(always)]
    pub fn get_course_manifest(&self, course_id: &Ustr) -> Result<CourseManifest> {
        self.course_library
            .read()
            .get_course_manifest(course_id)
            .ok_or_else(|| anyhow!("missing manifest for course with ID {}", course_id))
    }

    /// Returns the manifest for the course with the given ID.
    #[inline(always)]
    pub fn get_lesson_manifest(&self, lesson_id: &Ustr) -> Result<LessonManifest> {
        self.course_library
            .read()
            .get_lesson_manifest(lesson_id)
            .ok_or_else(|| anyhow!("missing manifest for lesson with ID {}", lesson_id))
    }

    /// Returns the manifest for the exercise with the given ID.
    #[inline(always)]
    pub fn get_exercise_manifest(&self, exercise_id: &Ustr) -> Result<ExerciseManifest> {
        self.course_library
            .read()
            .get_exercise_manifest(exercise_id)
            .ok_or_else(|| anyhow!("missing manifest for exercise with ID {}", exercise_id))
    }

    /// Returns whether the unit with the given ID is blacklisted.
    #[inline(always)]
    pub fn blacklisted(&self, unit_id: &Ustr) -> Result<bool> {
        let blacklisted = self.blacklist.read().blacklisted(unit_id)?;
        Ok(blacklisted)
    }

    /// Returns all the units that are dependencies of the unit with the given ID.
    #[inline(always)]
    pub fn get_all_dependents(&self, unit_id: &Ustr) -> Vec<Ustr> {
        return self
            .unit_graph
            .read()
            .get_dependents(unit_id)
            .unwrap_or_default()
            .into_iter()
            .collect();
    }

    /// Returns all the units superseded by the unit with the given ID.
    #[inline(always)]
    pub fn get_superseded(&self, unit_id: &Ustr) -> Option<UstrSet> {
        return self.unit_graph.read().get_superseded(unit_id);
    }

    /// Returns all the units that supersede the unit with the given ID.
    #[inline(always)]
    pub fn get_superseded_by(&self, unit_id: &Ustr) -> Option<UstrSet> {
        return self.unit_graph.read().get_superseded_by(unit_id);
    }

    /// Returns all the dependencies of the unit with the given ID at the given depth.
    pub fn get_dependencies_at_depth(&self, unit_id: &Ustr, depth: usize) -> Vec<Ustr> {
        // Search for the dependencies at the given depth.
        let mut dependencies = vec![];
        let mut stack = vec![(*unit_id, 0)];
        while let Some((candidate_id, candidate_depth)) = stack.pop() {
            if candidate_depth == depth {
                // Reached the end of the search.
                dependencies.push(candidate_id);
                continue;
            }

            // Otherwise, look up the dependencies of the candidate and continue the search.
            let candidate_dependencies = self.unit_graph.read().get_dependencies(&candidate_id);
            match candidate_dependencies {
                Some(candidate_dependencies) => {
                    if candidate_dependencies.is_empty() {
                        // No more dependencies to search. Add the candidate to the final list.
                        dependencies.push(candidate_id)
                    } else {
                        // Continue the search with the dependencies of the candidate.
                        stack.extend(
                            candidate_dependencies
                                .into_iter()
                                .map(|dependency| (dependency, candidate_depth + 1)),
                        );
                    }
                }
                None => dependencies.push(candidate_id),
            }
        }

        // Remove any units not found in the graph. This can happen if a unit claims a dependency on
        // a unit not found in the graph.
        dependencies
            .retain(|dependency| self.unit_graph.read().get_unit_type(dependency).is_some());
        dependencies
    }

    /// Returns the value of the course_id field in the manifest of the given lesson.
    #[inline(always)]
    pub fn get_lesson_course(&self, lesson_id: &Ustr) -> Option<Ustr> {
        self.unit_graph.read().get_lesson_course(lesson_id)
    }

    /// Returns whether the unit exists in the library. Some units will exist in the unit graph
    /// because they are a dependency of another, but their data might not exist in the library.
    #[inline(always)]
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
    #[inline(always)]
    pub fn get_lesson_exercises(&self, unit_id: &Ustr) -> Vec<Ustr> {
        self.unit_graph
            .read()
            .get_lesson_exercises(unit_id)
            .unwrap_or_default()
            .into_iter()
            .collect()
    }

    /// Returns the number of lessons in the given course.
    #[inline(always)]
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
    #[inline(always)]
    pub fn unit_passes_filter(
        &self,
        unit_id: &Ustr,
        metadata_filter: Option<&KeyValueFilter>,
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
                Ok(metadata_filter
                    .as_ref()
                    .unwrap()
                    .apply_to_course(&course_manifest))
            }
            UnitType::Lesson => {
                // Retrieve the lesson and course manifests and check if the lesson passes the
                // filter.
                let course_manifest =
                    self.get_course_manifest(&self.get_lesson_course(unit_id).unwrap_or_default())?;
                let lesson_manifest = self.get_lesson_manifest(unit_id)?;
                Ok(metadata_filter
                    .as_ref()
                    .unwrap()
                    .apply_to_lesson(&course_manifest, &lesson_manifest))
            }
        }
    }

    /// Increments the value in the frequency map for the given exercise ID.
    #[inline(always)]
    pub fn increment_exercise_frequency(&self, exercise_id: &Ustr) {
        let mut frequency_map = self.frequency_map.write();
        let frequency = frequency_map.entry(*exercise_id).or_insert(0.0);
        *frequency += 1.0;
    }

    /// Returns the frequency of the given exercise ID.
    #[inline(always)]
    pub fn get_exercise_frequency(&self, exercise_id: &Ustr) -> f32 {
        self.frequency_map
            .read()
            .get(exercise_id)
            .copied()
            .unwrap_or(0.0)
    }

    /// Returns the unit filter for the saved filter with the given ID. Returns an error if no
    /// filter exists with that ID exists.
    pub fn get_saved_filter(&self, filter_id: &str) -> Result<SavedFilter> {
        match self.filter_manager.read().get_filter(filter_id) {
            Some(filter) => Ok(filter),
            None => Err(anyhow!("no saved filter with ID {} exists", filter_id)),
        }
    }

    /// Returns the unit filter that should be used for the given study session.
    pub fn get_session_filter(
        &self,
        session_data: &StudySessionData,
        time: DateTime<Utc>,
    ) -> Result<Option<UnitFilter>> {
        match session_data.get_part(time) {
            SessionPart::NoFilter { .. } => Ok(None),
            SessionPart::UnitFilter { filter, .. } => Ok(Some(filter)),
            SessionPart::SavedFilter { filter_id, .. } => {
                let saved_filter = self.get_saved_filter(&filter_id)?;
                Ok(Some(saved_filter.filter))
            }
        }
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use chrono::Duration;
    use lazy_static::lazy_static;
    use parking_lot::RwLock;
    use std::{
        collections::{BTreeMap, HashMap},
        sync::Arc,
    };
    use ustr::Ustr;

    use crate::{
        data::{
            filter::{
                FilterType, KeyValueFilter, SavedFilter, SessionPart, StudySession,
                StudySessionData, UnitFilter,
            },
            UnitType,
        },
        filter_manager::LocalFilterManager,
        testutil::*,
    };

    static NUM_EXERCISES: usize = 2;

    lazy_static! {
        /// A simple set of courses to test the basic functionality of Trane.
        static ref TEST_LIBRARY: Vec<TestCourse> = vec![
            TestCourse {
                id: TestId(0, None, None),
                dependencies: vec![],
                superseded: vec![],
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
                        superseded: vec![],
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
                        superseded: vec![],
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
        let library = init_test_simulation(&temp_dir.path(), &TEST_LIBRARY)?;
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
        let library = init_test_simulation(&temp_dir.path(), &TEST_LIBRARY)?;
        let scheduler_data = library.get_scheduler_data();
        let metadata_filter = KeyValueFilter::CourseFilter {
            key: "key".into(),
            value: "value".into(),
            filter_type: FilterType::Include,
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
        let library = init_test_simulation(&temp_dir.path(), &TEST_LIBRARY)?;
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

    /// Verifies retrieving the filter for a session part.
    #[test]
    fn get_session_filter() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let library = init_test_simulation(&temp_dir.path(), &TEST_LIBRARY)?;

        // Add a saved filter to the filter manager.
        let mut scheduler_data = library.get_scheduler_data();
        scheduler_data.filter_manager = Arc::new(RwLock::new(LocalFilterManager {
            filters: HashMap::from([(
                "saved_filter".to_string(),
                SavedFilter {
                    id: "saved_filter".to_string(),
                    description: "Saved filter".to_string(),
                    filter: UnitFilter::ReviewListFilter,
                },
            )]),
        }));

        // Define the data for the study session.
        let start_time = chrono::Utc::now();
        let session_data = StudySessionData {
            start_time,
            definition: StudySession {
                id: "session".to_string(),
                description: "Session".to_string(),
                parts: vec![
                    SessionPart::UnitFilter {
                        filter: UnitFilter::ReviewListFilter,
                        duration: 1,
                    },
                    SessionPart::NoFilter { duration: 1 },
                    SessionPart::SavedFilter {
                        filter_id: "saved_filter".into(),
                        duration: 1,
                    },
                ],
            },
        };

        // Verify that the filter for each session part is correct.
        assert_eq!(
            scheduler_data.get_session_filter(&session_data, start_time)?,
            Some(UnitFilter::ReviewListFilter)
        );
        assert_eq!(
            scheduler_data.get_session_filter(&session_data, start_time + Duration::minutes(1))?,
            None
        );
        assert_eq!(
            scheduler_data.get_session_filter(&session_data, start_time + Duration::minutes(2))?,
            Some(UnitFilter::ReviewListFilter)
        );

        // Verify that trying to retrieve an unknown saved filter returns an error.
        assert!(scheduler_data
            .get_session_filter(
                &StudySessionData {
                    start_time,
                    definition: StudySession {
                        id: "session".to_string(),
                        description: "Session".to_string(),
                        parts: vec![SessionPart::SavedFilter {
                            filter_id: "unknown_filter".into(),
                            duration: 1,
                        }],
                    },
                },
                start_time
            )
            .is_err());

        Ok(())
    }
}
