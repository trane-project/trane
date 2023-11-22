//! Defines the data structures used to select from which units exercises should be selected during
//! scheduling.
//!
//! Trane's default mode of operation is to select exercises from all units. The filters in this
//! module define the following additional modes of operation:
//! 1. Selecting exercises from a list of courses.
//! 2. Selecting exercises from a list of lessons.
//! 3. Selecting exercises from the courses and lessons which match the given criteria based on the
//!    course and lesson metadata.
//! 4. Selecting exercises from the units in the review list.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ustr::Ustr;

use crate::data::GetMetadata;

/// The logical operation used to combine multiple filters.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FilterOp {
    /// A filter returns true if all its sub-filters pass.
    All,

    /// A filter returns true if at least one of its sub-filters pass.
    Any,
}

/// The type of filter according to whether the units which match are included or excluded.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FilterType {
    /// A filter which includes the units that match it.
    Include,

    /// A filter which excludes the units that match it.
    Exclude,
}

/// A filter on course or lesson metadata.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum KeyValueFilter {
    /// A basic filter that matches a key value pair in the course's metadata.
    CourseFilter {
        /// The key to filter.
        key: String,

        /// The value to filter.
        value: String,

        /// Whether units which match the filter should be included or excluded.
        filter_type: FilterType,
    },

    /// A basic filter that matches a key value pair in the lesson's metadata.
    LessonFilter {
        /// The key to filter.
        key: String,

        /// The value to filter.
        value: String,

        /// Whether units which match the filter should be included or excluded.
        filter_type: FilterType,
    },

    /// A combination of simpler filters on course or lesson metadata.
    CombinedFilter {
        /// The logical operation used to combine multiple filters.
        op: FilterOp,

        /// The filters to combine.
        filters: Vec<KeyValueFilter>,
    },
}

impl KeyValueFilter {
    /// Returns whether the given key-value pair passes the filter given a filter type and the
    /// unit's metadata.
    fn passes_filter(
        metadata: &BTreeMap<String, Vec<String>>,
        key: &str,
        value: &str,
        filter_type: &FilterType,
    ) -> bool {
        // Check whether the key-value pair is present in the metadata.
        let contains_metadata = if metadata.contains_key(key) {
            metadata
                .get(key)
                .unwrap_or(&Vec::new())
                .contains(&value.to_string())
        } else {
            false
        };

        // Decide whether the filter passes based on its type.
        match filter_type {
            FilterType::Include => contains_metadata,
            FilterType::Exclude => !contains_metadata,
        }
    }

    /// Applies the filter to the course with the given manifest.
    pub fn apply_to_course(&self, course_manifest: &impl GetMetadata) -> bool {
        let default_metadata = BTreeMap::default();
        let course_metadata = course_manifest.get_metadata().unwrap_or(&default_metadata);

        match self {
            KeyValueFilter::CourseFilter {
                key,
                value,
                filter_type,
            } => {
                // Compare the course's metadata against the filter.
                KeyValueFilter::passes_filter(course_metadata, key, value, filter_type)
            }
            KeyValueFilter::LessonFilter { .. } => {
                // Return false because this filter is not applicable to courses. The course will be
                // skipped, and the decision will be made based on the lesson's metadata.
                false
            }
            KeyValueFilter::CombinedFilter { op, filters } => {
                // Separate the course filters from the list of filters.
                let course_filters = filters
                    .iter()
                    .filter(|f| matches!(f, KeyValueFilter::CourseFilter { .. }))
                    .collect::<Vec<_>>();
                let other_filters = filters
                    .iter()
                    .filter(|f| !matches!(f, KeyValueFilter::CourseFilter { .. }))
                    .collect::<Vec<_>>();

                // Apply each course filter individually and combine the results based on the
                // logical operation. Do the same for the other filters.
                let course_result = match *op {
                    FilterOp::All => course_filters
                        .iter()
                        .map(|f| f.apply_to_course(course_manifest))
                        .all(|x| x),
                    FilterOp::Any => course_filters
                        .iter()
                        .map(|f| f.apply_to_course(course_manifest))
                        .any(|x| x),
                };
                let other_result = match *op {
                    FilterOp::All => other_filters
                        .iter()
                        .map(|f| f.apply_to_course(course_manifest))
                        .all(|x| x),
                    FilterOp::Any => other_filters
                        .iter()
                        .map(|f| f.apply_to_course(course_manifest))
                        .any(|x| x),
                };

                // If there were only course filters, return that result as is.
                if other_filters.is_empty() {
                    return course_result;
                }

                // Otherwise, return false if the operation is `Any` so that the course is skipped
                // and the decision is made based on the lesson.
                match *op {
                    FilterOp::All => course_result && other_result,
                    FilterOp::Any => false,
                }
            }
        }
    }

    /// Applies the filter to the lesson with the given manifest. The function also takes the
    /// manifest of the lesson's course to exclude lessons whose course do not match the filter.
    pub fn apply_to_lesson(
        &self,
        course_manifest: &impl GetMetadata,
        lesson_manifest: &impl GetMetadata,
    ) -> bool {
        let default_metadata = BTreeMap::default();
        let course_metadata = course_manifest.get_metadata().unwrap_or(&default_metadata);
        let lesson_metadata = lesson_manifest.get_metadata().unwrap_or(&default_metadata);

        match self {
            KeyValueFilter::CourseFilter {
                key,
                value,
                filter_type,
            } => {
                // Compare the course's metadata against the filter.
                KeyValueFilter::passes_filter(course_metadata, key, value, filter_type)
            }
            KeyValueFilter::LessonFilter {
                key,
                value,
                filter_type,
            } => {
                // Compare the lesson's metadata against the filter.
                KeyValueFilter::passes_filter(lesson_metadata, key, value, filter_type)
            }
            KeyValueFilter::CombinedFilter { op, filters } => {
                // Combine the filters using the given logical operation.
                match *op {
                    FilterOp::All => filters
                        .iter()
                        .map(|f| f.apply_to_lesson(course_manifest, lesson_manifest))
                        .all(|x| x),
                    FilterOp::Any => filters
                        .iter()
                        .map(|f| f.apply_to_lesson(course_manifest, lesson_manifest))
                        .any(|x| x),
                }
            }
        }
    }
}

// grcov-excl-start: Code coverage for this struct is flaky for some unknown reason.
/// A filter on a course or lesson manifest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum UnitFilter {
    /// A filter to show exercises belonging to the given courses.
    CourseFilter {
        /// The IDs of the courses to filter.
        course_ids: Vec<Ustr>,
    },

    /// A filter to show exercises belonging to the given lessons.
    LessonFilter {
        /// The IDs of the lessons to filter.
        lesson_ids: Vec<Ustr>,
    },

    /// A filter on the metadata of a course or lesson.
    MetadataFilter {
        /// The filter to apply to the course or lesson metadata.
        filter: KeyValueFilter,
    },

    /// A filter that indicates only exercises from the review list should be scheduled.
    ReviewListFilter,

    /// A filter that schedules exercises from all the given units and its dependents.
    Dependents {
        /// The IDs of the units from which to start the search.
        unit_ids: Vec<Ustr>,
    },

    /// A filter that schedules exercises from the dependencies of the given units.
    Dependencies {
        /// The IDs from which to look up the dependencies.
        unit_ids: Vec<Ustr>,

        /// The depth of the dependency tree to search.
        depth: usize,
    },
}
// grcov-excl-stop

impl UnitFilter {
    /// Returns whether the course with the given ID passes the course filter.
    #[must_use]
    pub fn passes_course_filter(&self, course_id: &Ustr) -> bool {
        match self {
            UnitFilter::CourseFilter { course_ids } => course_ids.contains(course_id),
            _ => false,
        }
    }

    /// Returns whether the lesson with the given ID passes the lesson filter.
    #[must_use]
    pub fn passes_lesson_filter(&self, lesson_id: &Ustr) -> bool {
        match self {
            UnitFilter::LessonFilter { lesson_ids } => lesson_ids.contains(lesson_id),
            _ => false,
        }
    }
}

//@<saved-filter
/// A saved filter for easy reference.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SavedFilter {
    /// A unique ID for the filter.
    pub id: String,

    /// A human-readable description of the filter.
    pub description: String,

    /// The filter to apply.
    pub filter: UnitFilter,
}
//>@saved-filter

/// A part of a study session. Contains the criteria used to filter the exercises during a section
/// of the study session along with the duration in minutes. The filter can either be a
/// [`UnitFilter`] defined inline or a reference to a [`SavedFilter`].
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum SessionPart {
    /// A part of the study session that uses a filter defined inline.
    UnitFilter {
        /// The filter to use.
        filter: UnitFilter,

        /// The duration of this section of the study session, in minutes.
        duration: u32,
    },
    /// A part of the study session that references a saved filter.
    SavedFilter {
        /// The ID of the saved filter to use.
        filter_id: String,

        /// The duration of this section of the study session, in minutes.
        duration: u32,
    },
    /// A part of the study session that does not have a filter. The scheduler will use exercises
    /// from the entire unit graph.
    NoFilter {
        /// The duration of this section of the study session, in minutes.
        duration: u32,
    },
}

impl SessionPart {
    /// Returns the duration of the study part.
    #[must_use]
    pub fn duration(&self) -> u32 {
        match self {
            SessionPart::UnitFilter { duration, .. }
            | SessionPart::SavedFilter { duration, .. }
            | SessionPart::NoFilter { duration, .. } => *duration,
        }
    }
}

/// A study session is a list of parts, each of which define the exercises to study and for how
/// long. For example, a student learning to play piano and guitar could define a session that
/// spends 30 minutes on exercises for piano, and 30 minutes on exercises for guitar.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StudySession {
    /// A unique identifier for the study session.
    pub id: String,

    /// A human-readable description for the study session.
    #[serde(default)]
    pub description: String,

    /// The parts of the study session.
    #[serde(default)]
    pub parts: Vec<SessionPart>,
}

/// A specific instance of a study session. It contains the start time of the session and its
/// definition so that the scheduler knows the progress of the session.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StudySessionData {
    /// The start time of the session.
    pub start_time: DateTime<Utc>,

    /// The definition of the session.
    pub definition: StudySession,
}

impl StudySessionData {
    /// Returns the study session part that should be practiced at the given time.
    #[must_use]
    pub fn get_part(&self, time: DateTime<Utc>) -> SessionPart {
        // Return a dummy part with no filter if the session has no parts.
        if self.definition.parts.is_empty() {
            return SessionPart::NoFilter { duration: 0 };
        }

        // Get the number of minutes since the start of the session. Return the first part if the
        // value is negative.
        let minutes_since_start = (time - self.start_time).num_minutes();
        if minutes_since_start < 0 {
            return self.definition.parts[0].clone();
        }
        let minutes_since_start = minutes_since_start as u32;

        // Find the first part that has not been completed yet. If all parts have been completed,
        // return the last part.
        let mut session_length = 0;
        for part in &self.definition.parts {
            session_length += part.duration();
            if minutes_since_start < session_length {
                return part.clone();
            }
        }
        self.definition.parts.last().unwrap().clone()
    }
}

/// A set of options to control which exercises should be considered to be included in the final
/// batch.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum ExerciseFilter {
    /// Select exercises based on a unit filter.
    UnitFilter(UnitFilter),

    /// Select exercises based on a study session.
    StudySession(StudySessionData),
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use chrono::{Duration, Utc};
    use std::collections::BTreeMap;
    use ustr::Ustr;

    use crate::data::{
        filter::{FilterOp, FilterType, KeyValueFilter, SessionPart, StudySessionData, UnitFilter},
        GetMetadata,
    };

    use super::StudySession;

    impl GetMetadata for BTreeMap<String, Vec<String>> {
        fn get_metadata(&self) -> Option<&BTreeMap<String, Vec<String>>> {
            Some(self)
        }
    }

    /// Verifies that the correct courses pass the course filter.
    #[test]
    fn passes_course_filter() {
        let filter = UnitFilter::CourseFilter {
            course_ids: vec!["course1".into()],
        };
        assert!(filter.passes_course_filter(&"course1".into()));
        assert!(!filter.passes_course_filter(&"course2".into()));
        assert!(!filter.passes_lesson_filter(&"lesson1".into()));
    }

    /// Verifies that the correct lessons pass the lesson filter.
    #[test]
    fn passes_lesson_filter() {
        let filter = UnitFilter::LessonFilter {
            lesson_ids: vec!["lesson1".into()],
        };
        assert!(filter.passes_lesson_filter(&"lesson1".into()));
        assert!(!filter.passes_lesson_filter(&"lesson2".into()));
        assert!(!filter.passes_course_filter(&"course1".into()));
    }

    /// Verifies correctly applying a course filter to a course.
    #[test]
    fn apply_course_filter_to_course() -> Result<()> {
        let metadata = BTreeMap::from([
            (
                "key1".to_string(),
                vec!["value1".to_string(), "value2".to_string()],
            ),
            (
                "key2".to_string(),
                vec!["value3".to_string(), "value4".to_string()],
            ),
        ]);
        let include_filter = KeyValueFilter::CourseFilter {
            key: "key1".to_string(),
            value: "value1".to_string(),
            filter_type: FilterType::Include,
        };
        assert!(include_filter.apply_to_course(&metadata));
        let exclude_filter = KeyValueFilter::CourseFilter {
            key: "key1".to_string(),
            value: "value1".to_string(),
            filter_type: FilterType::Exclude,
        };
        assert!(!exclude_filter.apply_to_course(&metadata));
        Ok(())
    }

    /// Verifies applying a lesson filter to a course.
    #[test]
    fn apply_lesson_filter_to_course() -> Result<()> {
        let metadata = BTreeMap::from([
            (
                "key1".to_string(),
                vec!["value1".to_string(), "value2".to_string()],
            ),
            (
                "key2".to_string(),
                vec!["value3".to_string(), "value4".to_string()],
            ),
        ]);
        let include_filter = KeyValueFilter::LessonFilter {
            key: "key1".to_string(),
            value: "value1".to_string(),
            filter_type: FilterType::Include,
        };
        assert!(!include_filter.apply_to_course(&metadata));
        let exclude_filter = KeyValueFilter::LessonFilter {
            key: "key1".to_string(),
            value: "value1".to_string(),
            filter_type: FilterType::Exclude,
        };
        assert!(!exclude_filter.apply_to_course(&metadata));
        Ok(())
    }

    /// Verifies applying a course filter to a course with metadata that doesn't contain the
    /// required keys or values.
    #[test]
    fn apply_course_filter_to_course_no_match() -> Result<()> {
        let metadata = BTreeMap::from([
            (
                "key1".to_string(),
                vec!["value1".to_string(), "value2".to_string()],
            ),
            (
                "key2".to_string(),
                vec!["value3".to_string(), "value4".to_string()],
            ),
        ]);

        // The key-value pair doesn't exist in the metadata, so the filter should not apply.
        let include_filter = KeyValueFilter::CourseFilter {
            key: "key10".to_string(),
            value: "value1".to_string(),
            filter_type: FilterType::Include,
        };
        assert!(!include_filter.apply_to_course(&metadata));

        // The same key-value pair should apply to the exclude filter.
        let exclude_filter = KeyValueFilter::CourseFilter {
            key: "key10".to_string(),
            value: "value1".to_string(),
            filter_type: FilterType::Exclude,
        };
        assert!(exclude_filter.apply_to_course(&metadata));
        Ok(())
    }

    /// Verifies correctly applying a course filter to a lesson.
    #[test]
    fn apply_course_filter_to_lesson() -> Result<()> {
        let course_metadata = BTreeMap::from([
            (
                "key1".to_string(),
                vec!["value1".to_string(), "value2".to_string()],
            ),
            (
                "key2".to_string(),
                vec!["value3".to_string(), "value4".to_string()],
            ),
        ]);
        let lesson_metadata = BTreeMap::from([
            (
                "key3".to_string(),
                vec!["value5".to_string(), "value6".to_string()],
            ),
            (
                "key4".to_string(),
                vec!["value7".to_string(), "value8".to_string()],
            ),
        ]);

        let include_filter = KeyValueFilter::CourseFilter {
            key: "key1".to_string(),
            value: "value1".to_string(),
            filter_type: FilterType::Include,
        };
        assert!(include_filter.apply_to_lesson(&course_metadata, &lesson_metadata));
        let exclude_filter = KeyValueFilter::CourseFilter {
            key: "key1".to_string(),
            value: "value1".to_string(),
            filter_type: FilterType::Exclude,
        };
        assert!(!exclude_filter.apply_to_lesson(&course_metadata, &lesson_metadata));
        Ok(())
    }

    /// Verifies correctly applying a lesson filter to a lesson.
    #[test]
    fn apply_lesson_filter_to_lesson() -> Result<()> {
        let course_metadata = BTreeMap::from([
            (
                "key1".to_string(),
                vec!["value1".to_string(), "value2".to_string()],
            ),
            (
                "key2".to_string(),
                vec!["value3".to_string(), "value4".to_string()],
            ),
        ]);
        let lesson_metadata = BTreeMap::from([
            (
                "key3".to_string(),
                vec!["value5".to_string(), "value6".to_string()],
            ),
            (
                "key4".to_string(),
                vec!["value7".to_string(), "value8".to_string()],
            ),
        ]);

        let include_filter = KeyValueFilter::LessonFilter {
            key: "key3".to_string(),
            value: "value5".to_string(),
            filter_type: FilterType::Include,
        };
        assert!(include_filter.apply_to_lesson(&course_metadata, &lesson_metadata));
        let exclude_filter = KeyValueFilter::LessonFilter {
            key: "key3".to_string(),
            value: "value5".to_string(),
            filter_type: FilterType::Exclude,
        };
        assert!(!exclude_filter.apply_to_lesson(&course_metadata, &lesson_metadata));
        Ok(())
    }

    /// Verifies correctly applying a course filter to a lesson that does not match the filter.
    #[test]
    fn apply_course_filter_to_lesson_no_match() -> Result<()> {
        let course_metadata = BTreeMap::from([
            (
                "key1".to_string(),
                vec!["value1".to_string(), "value2".to_string()],
            ),
            (
                "key2".to_string(),
                vec!["value3".to_string(), "value4".to_string()],
            ),
        ]);
        let lesson_metadata = BTreeMap::from([
            (
                "key3".to_string(),
                vec!["value5".to_string(), "value6".to_string()],
            ),
            (
                "key4".to_string(),
                vec!["value7".to_string(), "value8".to_string()],
            ),
        ]);

        let include_filter = KeyValueFilter::CourseFilter {
            key: "key3".to_string(),
            value: "value1".to_string(),
            filter_type: FilterType::Include,
        };
        assert!(!include_filter.apply_to_lesson(&course_metadata, &lesson_metadata));
        let exclude_filter = KeyValueFilter::CourseFilter {
            key: "key3".to_string(),
            value: "value1".to_string(),
            filter_type: FilterType::Exclude,
        };
        assert!(exclude_filter.apply_to_lesson(&course_metadata, &lesson_metadata));
        Ok(())
    }

    /// Verifies correctly applying a lesson filter to a lesson that does not match the filter.
    #[test]
    fn apply_lesson_filter_to_lesson_no_match() -> Result<()> {
        let course_metadata = BTreeMap::from([
            (
                "key1".to_string(),
                vec!["value1".to_string(), "value2".to_string()],
            ),
            (
                "key2".to_string(),
                vec!["value3".to_string(), "value4".to_string()],
            ),
        ]);
        let lesson_metadata = BTreeMap::from([
            (
                "key3".to_string(),
                vec!["value5".to_string(), "value6".to_string()],
            ),
            (
                "key4".to_string(),
                vec!["value7".to_string(), "value8".to_string()],
            ),
        ]);

        let include_filter = KeyValueFilter::LessonFilter {
            key: "key2".to_string(),
            value: "value5".to_string(),
            filter_type: FilterType::Include,
        };
        assert!(!include_filter.apply_to_lesson(&course_metadata, &lesson_metadata));
        let exclude_filter = KeyValueFilter::LessonFilter {
            key: "key2".to_string(),
            value: "value5".to_string(),
            filter_type: FilterType::Exclude,
        };
        assert!(exclude_filter.apply_to_lesson(&course_metadata, &lesson_metadata));
        Ok(())
    }

    /// Verifies applying a combined key-value filter with the `All` operator to a course.
    #[test]
    fn apply_combined_all_filter_to_course() -> Result<()> {
        let metadata = BTreeMap::from([
            (
                "key1".to_string(),
                vec!["value1".to_string(), "value2".to_string()],
            ),
            (
                "key2".to_string(),
                vec!["value3".to_string(), "value4".to_string()],
            ),
        ]);
        let filter = KeyValueFilter::CombinedFilter {
            op: FilterOp::All,
            filters: vec![
                KeyValueFilter::CourseFilter {
                    key: "key1".to_string(),
                    value: "value1".to_string(),
                    filter_type: FilterType::Include,
                },
                KeyValueFilter::CourseFilter {
                    key: "key2".to_string(),
                    value: "value4".to_string(),
                    filter_type: FilterType::Include,
                },
            ],
        };
        assert!(filter.apply_to_course(&metadata));
        Ok(())
    }

    /// Verifies applying a combined key-value filter containing a lesson filter with the `All`
    /// operator to a course.
    #[test]
    fn apply_combined_all_filter_with_lesson_filter_to_course() -> Result<()> {
        let metadata = BTreeMap::from([
            (
                "key1".to_string(),
                vec!["value1".to_string(), "value2".to_string()],
            ),
            (
                "key2".to_string(),
                vec!["value3".to_string(), "value4".to_string()],
            ),
        ]);
        let filter = KeyValueFilter::CombinedFilter {
            op: FilterOp::All,
            filters: vec![
                KeyValueFilter::CourseFilter {
                    key: "key1".to_string(),
                    value: "value1".to_string(),
                    filter_type: FilterType::Include,
                },
                KeyValueFilter::CourseFilter {
                    key: "key2".to_string(),
                    value: "value4".to_string(),
                    filter_type: FilterType::Include,
                },
                KeyValueFilter::LessonFilter {
                    key: "key3".to_string(),
                    value: "value5".to_string(),
                    filter_type: FilterType::Include,
                },
            ],
        };
        assert!(!filter.apply_to_course(&metadata));
        Ok(())
    }

    /// Verifies applying a combined key-value filter containing a nested combined filter with the
    /// `All` operator to a course.
    #[test]
    fn apply_combined_all_filter_with_combined_filter_to_course() -> Result<()> {
        let metadata = BTreeMap::from([
            (
                "key1".to_string(),
                vec!["value1".to_string(), "value2".to_string()],
            ),
            (
                "key2".to_string(),
                vec!["value3".to_string(), "value4".to_string()],
            ),
        ]);
        let filter = KeyValueFilter::CombinedFilter {
            op: FilterOp::All,
            filters: vec![
                KeyValueFilter::CourseFilter {
                    key: "key1".to_string(),
                    value: "value1".to_string(),
                    filter_type: FilterType::Include,
                },
                KeyValueFilter::CourseFilter {
                    key: "key2".to_string(),
                    value: "value4".to_string(),
                    filter_type: FilterType::Include,
                },
                KeyValueFilter::CombinedFilter {
                    op: FilterOp::All,
                    filters: vec![KeyValueFilter::CourseFilter {
                        key: "key1".to_string(),
                        value: "value2".to_string(),
                        filter_type: FilterType::Include,
                    }],
                },
            ],
        };
        assert!(filter.apply_to_course(&metadata));
        Ok(())
    }

    /// Verifies applying a combined key-value filter with the `All` operator to a key-value pair that
    /// does not pass the filter.
    #[test]
    fn apply_combined_all_filter_to_course_no_match() -> Result<()> {
        let metadata = BTreeMap::from([
            (
                "key1".to_string(),
                vec!["value1".to_string(), "value2".to_string()],
            ),
            (
                "key2".to_string(),
                vec!["value3".to_string(), "value4".to_string()],
            ),
        ]);
        let filter = KeyValueFilter::CombinedFilter {
            op: FilterOp::All,
            filters: vec![
                KeyValueFilter::CourseFilter {
                    key: "key1".to_string(),
                    value: "value1".to_string(),
                    filter_type: FilterType::Include,
                },
                KeyValueFilter::CourseFilter {
                    key: "key2".to_string(),
                    value: "value5".to_string(),
                    filter_type: FilterType::Include,
                },
            ],
        };
        assert!(!filter.apply_to_course(&metadata));
        Ok(())
    }

    /// Verifies applying a combined key-value filter with the `Any` operator to a course.
    #[test]
    fn apply_combined_any_filter_to_course() -> Result<()> {
        let metadata = BTreeMap::from([
            (
                "key1".to_string(),
                vec!["value1".to_string(), "value2".to_string()],
            ),
            (
                "key2".to_string(),
                vec!["value3".to_string(), "value4".to_string()],
            ),
        ]);
        let filter = KeyValueFilter::CombinedFilter {
            op: FilterOp::Any,
            filters: vec![
                KeyValueFilter::CourseFilter {
                    key: "key1".to_string(),
                    value: "value1".to_string(),
                    filter_type: FilterType::Include,
                },
                KeyValueFilter::CourseFilter {
                    key: "key2".to_string(),
                    value: "value5".to_string(),
                    filter_type: FilterType::Include,
                },
            ],
        };
        assert!(filter.apply_to_course(&metadata));
        Ok(())
    }

    /// Verifies applying a combined key-value filter with the `Any` operator to a course that
    /// does not pass the filter.
    #[test]
    fn apply_combined_any_filter_to_course_no_match() -> Result<()> {
        let metadata = BTreeMap::from([
            (
                "key1".to_string(),
                vec!["value1".to_string(), "value2".to_string()],
            ),
            (
                "key2".to_string(),
                vec!["value3".to_string(), "value4".to_string()],
            ),
        ]);
        let filter = KeyValueFilter::CombinedFilter {
            op: FilterOp::Any,
            filters: vec![
                KeyValueFilter::CourseFilter {
                    key: "key1".to_string(),
                    value: "value3".to_string(),
                    filter_type: FilterType::Include,
                },
                KeyValueFilter::CourseFilter {
                    key: "key2".to_string(),
                    value: "value5".to_string(),
                    filter_type: FilterType::Include,
                },
            ],
        };
        assert!(!filter.apply_to_course(&metadata));
        Ok(())
    }

    /// Verifies applying a combined key-value filter containing a nested combined filter with the
    /// `Any` operator to a course.
    #[test]
    fn apply_combined_any_filter_with_combined_filter_to_course() -> Result<()> {
        let metadata = BTreeMap::from([
            (
                "key1".to_string(),
                vec!["value1".to_string(), "value2".to_string()],
            ),
            (
                "key2".to_string(),
                vec!["value3".to_string(), "value4".to_string()],
            ),
        ]);
        let filter = KeyValueFilter::CombinedFilter {
            op: FilterOp::Any,
            filters: vec![
                KeyValueFilter::CourseFilter {
                    key: "key1".to_string(),
                    value: "value1".to_string(),
                    filter_type: FilterType::Include,
                },
                KeyValueFilter::CourseFilter {
                    key: "key2".to_string(),
                    value: "value4".to_string(),
                    filter_type: FilterType::Include,
                },
                KeyValueFilter::CombinedFilter {
                    op: FilterOp::All,
                    filters: vec![KeyValueFilter::CourseFilter {
                        key: "key1".to_string(),
                        value: "value2".to_string(),
                        filter_type: FilterType::Include,
                    }],
                },
            ],
        };
        assert!(!filter.apply_to_course(&metadata));
        Ok(())
    }

    /// Verifies applying a combined key-value filter with the `All` operator to a lesson.
    #[test]
    fn apply_combined_all_filter_to_lesson() -> Result<()> {
        let course_metadata = BTreeMap::from([
            (
                "key1".to_string(),
                vec!["value1".to_string(), "value2".to_string()],
            ),
            (
                "key2".to_string(),
                vec!["value3".to_string(), "value4".to_string()],
            ),
        ]);
        let lesson_metadata = BTreeMap::from([
            (
                "key3".to_string(),
                vec!["value5".to_string(), "value6".to_string()],
            ),
            (
                "key4".to_string(),
                vec!["value7".to_string(), "value8".to_string()],
            ),
        ]);

        let filter = KeyValueFilter::CombinedFilter {
            op: FilterOp::All,
            filters: vec![
                KeyValueFilter::CourseFilter {
                    key: "key1".to_string(),
                    value: "value1".to_string(),
                    filter_type: FilterType::Include,
                },
                KeyValueFilter::LessonFilter {
                    key: "key3".to_string(),
                    value: "value6".to_string(),
                    filter_type: FilterType::Include,
                },
            ],
        };
        assert!(filter.apply_to_lesson(&course_metadata, &lesson_metadata));
        Ok(())
    }

    /// Verifies applying a combined key-value filter with the `All` operator to a lesson that does
    /// not match.
    #[test]
    fn apply_combined_all_filter_to_lesson_no_match() -> Result<()> {
        let course_metadata = BTreeMap::from([
            (
                "key1".to_string(),
                vec!["value1".to_string(), "value2".to_string()],
            ),
            (
                "key2".to_string(),
                vec!["value3".to_string(), "value4".to_string()],
            ),
        ]);
        let lesson_metadata = BTreeMap::from([
            (
                "key3".to_string(),
                vec!["value5".to_string(), "value6".to_string()],
            ),
            (
                "key4".to_string(),
                vec!["value7".to_string(), "value8".to_string()],
            ),
        ]);

        let filter = KeyValueFilter::CombinedFilter {
            op: FilterOp::All,
            filters: vec![
                KeyValueFilter::CourseFilter {
                    key: "key1".to_string(),
                    value: "value8".to_string(),
                    filter_type: FilterType::Include,
                },
                KeyValueFilter::LessonFilter {
                    key: "key3".to_string(),
                    value: "value6".to_string(),
                    filter_type: FilterType::Include,
                },
            ],
        };
        assert!(!filter.apply_to_lesson(&course_metadata, &lesson_metadata));
        Ok(())
    }

    /// Verifies applying a combined key-value filter with the `Any` operator to a lesson.
    #[test]
    fn apply_combined_any_filter_to_lesson() -> Result<()> {
        let course_metadata = BTreeMap::from([
            (
                "key1".to_string(),
                vec!["value1".to_string(), "value2".to_string()],
            ),
            (
                "key2".to_string(),
                vec!["value3".to_string(), "value4".to_string()],
            ),
        ]);
        let lesson_metadata = BTreeMap::from([
            (
                "key3".to_string(),
                vec!["value5".to_string(), "value6".to_string()],
            ),
            (
                "key4".to_string(),
                vec!["value7".to_string(), "value8".to_string()],
            ),
        ]);

        let filter = KeyValueFilter::CombinedFilter {
            op: FilterOp::Any,
            filters: vec![
                KeyValueFilter::CourseFilter {
                    key: "key1".to_string(),
                    value: "value1".to_string(),
                    filter_type: FilterType::Include,
                },
                KeyValueFilter::LessonFilter {
                    key: "key3".to_string(),
                    value: "value1".to_string(),
                    filter_type: FilterType::Include,
                },
            ],
        };
        assert!(filter.apply_to_lesson(&course_metadata, &lesson_metadata));
        Ok(())
    }

    /// Verifies applying a combined key-value filter with the `Any` operator to a lesson that does
    /// not match.
    #[test]
    fn apply_combined_any_filter_to_lesson_no_match() -> Result<()> {
        let course_metadata = BTreeMap::from([
            (
                "key1".to_string(),
                vec!["value1".to_string(), "value2".to_string()],
            ),
            (
                "key2".to_string(),
                vec!["value3".to_string(), "value4".to_string()],
            ),
        ]);
        let lesson_metadata = BTreeMap::from([
            (
                "key3".to_string(),
                vec!["value5".to_string(), "value6".to_string()],
            ),
            (
                "key4".to_string(),
                vec!["value7".to_string(), "value8".to_string()],
            ),
        ]);

        let filter = KeyValueFilter::CombinedFilter {
            op: FilterOp::Any,
            filters: vec![
                KeyValueFilter::CourseFilter {
                    key: "key1".to_string(),
                    value: "value8".to_string(),
                    filter_type: FilterType::Include,
                },
                KeyValueFilter::LessonFilter {
                    key: "key3".to_string(),
                    value: "value2".to_string(),
                    filter_type: FilterType::Include,
                },
            ],
        };
        assert!(!filter.apply_to_lesson(&course_metadata, &lesson_metadata));
        Ok(())
    }

    /// Verifies cloning a unit filter. Done so that the auto-generated trait implementation is
    /// included in the code coverage reports.
    #[test]
    fn unit_filter_clone() {
        let filter = UnitFilter::CourseFilter {
            course_ids: vec![Ustr::from("course1")],
        };
        assert_eq!(filter.clone(), filter);

        let filter = UnitFilter::LessonFilter {
            lesson_ids: vec![Ustr::from("lesson1")],
        };
        assert_eq!(filter.clone(), filter);

        let filter = UnitFilter::MetadataFilter {
            filter: KeyValueFilter::CourseFilter {
                key: "key".into(),
                value: "value".into(),
                filter_type: FilterType::Include,
            },
        };
        assert_eq!(filter.clone(), filter);

        let filter = UnitFilter::ReviewListFilter;
        assert_eq!(filter.clone(), filter);
    }

    /// Verifies selecting the right session part based on the time.
    #[test]
    fn get_session_part() {
        // Get the part from an empty study session.
        let session_data = StudySessionData {
            definition: StudySession {
                id: "session".into(),
                description: "session".into(),
                parts: vec![],
            },
            start_time: Utc::now(),
        };
        assert_eq!(
            session_data.get_part(Utc::now()),
            SessionPart::NoFilter { duration: 0 }
        );

        // Get the first part if the number of minutes since the start time is negative.
        let start_time = Utc::now();
        let session_data = StudySessionData {
            definition: StudySession {
                id: "session".into(),
                description: "session".into(),
                parts: vec![
                    SessionPart::SavedFilter {
                        filter_id: "1".into(),
                        duration: 1,
                    },
                    SessionPart::SavedFilter {
                        filter_id: "2".into(),
                        duration: 1,
                    },
                    SessionPart::SavedFilter {
                        filter_id: "3".into(),
                        duration: 1,
                    },
                ],
            },
            start_time,
        };
        assert_eq!(
            session_data.get_part(start_time - Duration::minutes(1)),
            SessionPart::SavedFilter {
                filter_id: "1".into(),
                duration: 1
            }
        );

        // Get each of the parts of the study session.
        let start_time = Utc::now();
        let session_data = StudySessionData {
            definition: StudySession {
                id: "session".into(),
                description: "session".into(),
                parts: vec![
                    SessionPart::SavedFilter {
                        filter_id: "1".into(),
                        duration: 1,
                    },
                    SessionPart::SavedFilter {
                        filter_id: "2".into(),
                        duration: 1,
                    },
                    SessionPart::SavedFilter {
                        filter_id: "3".into(),
                        duration: 1,
                    },
                ],
            },
            start_time,
        };
        assert_eq!(
            session_data.get_part(start_time),
            SessionPart::SavedFilter {
                filter_id: "1".into(),
                duration: 1
            }
        );
        assert_eq!(
            session_data.get_part(start_time + Duration::minutes(1)),
            SessionPart::SavedFilter {
                filter_id: "2".into(),
                duration: 1
            }
        );
        assert_eq!(
            session_data.get_part(start_time + Duration::minutes(2)),
            SessionPart::SavedFilter {
                filter_id: "3".into(),
                duration: 1
            }
        );

        // Get the last part when the time is past the end of the session.
        assert_eq!(
            session_data.get_part(start_time + Duration::minutes(30)),
            SessionPart::SavedFilter {
                filter_id: "3".into(),
                duration: 1
            }
        );
    }
}
