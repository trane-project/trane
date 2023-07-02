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
    /// A basic filter that matches a key value pair.
    BasicFilter {
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
        filter_type: FilterType,
    ) -> bool {
        // Check whether the key-value pair is present in the metadata.
        let contains_metadata = if !metadata.contains_key(key) {
            false
        } else {
            metadata
                .get(key)
                .unwrap_or(&Vec::new())
                .contains(&value.to_string())
        };

        // Decide whether the filter passes based on its type.
        match filter_type {
            FilterType::Include => contains_metadata,
            FilterType::Exclude => !contains_metadata,
        }
    }

    /// Applies the filter to the given manifest.
    pub fn apply(&self, manifest: &impl GetMetadata) -> bool {
        let default_metadata = BTreeMap::default();
        let metadata = manifest.get_metadata().unwrap_or(default_metadata);

        match self {
            KeyValueFilter::BasicFilter {
                key,
                value,
                filter_type,
            } => {
                // Return whether the unit passes the single key-value filter.
                KeyValueFilter::passes_filter(&metadata, key, value, filter_type.clone())
            }
            KeyValueFilter::CombinedFilter { op, filters } => {
                // Apply each filter individually and combine the results based on the logical
                // operation.
                let mut results = filters.iter().map(|f| f.apply(manifest));
                match *op {
                    FilterOp::All => results.all(|x| x),
                    FilterOp::Any => results.any(|x| x),
                }
            }
        }
    }
}

/// A filter on course and/or lesson metadata.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MetadataFilter {
    /// The filter to apply on course metadata.
    #[serde(default)]
    pub course_filter: Option<KeyValueFilter>,

    /// The filter to apply on lesson metadata.
    #[serde(default)]
    pub lesson_filter: Option<KeyValueFilter>,

    /// The logical operation used to combine the course and lesson filters.
    pub op: FilterOp,
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
        filter: MetadataFilter,
    },

    /// A filter that indicates only exercises from the review list should be scheduled.
    ReviewListFilter,

    /// A filter that schedules exercises from all the given units and its dependents.
    Dependents {
        /// The IDs of the units from which to start the search.
        unit_ids: Vec<Ustr>,
    },

    /// A filter that schedules exercies from the dependencies of the given units.
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
    pub fn passes_course_filter(&self, course_id: &Ustr) -> bool {
        match self {
            UnitFilter::CourseFilter { course_ids } => course_ids.contains(course_id),
            _ => false,
        }
    }

    /// Returns whether the lesson with the given ID passes the lesson filter.
    pub fn passes_lesson_filter(&self, lesson_id: &Ustr) -> bool {
        match self {
            UnitFilter::LessonFilter { lesson_ids } => lesson_ids.contains(lesson_id),
            _ => false,
        }
    }

    /// Returns whether the course with the given metadata passes the metadata filter.
    pub fn course_passes_metadata_filter(
        filter: &MetadataFilter,
        course_manifest: &impl GetMetadata,
    ) -> bool {
        // Apply the course filter to the metadata.
        let course_passes = filter
            .course_filter
            .as_ref()
            .map(|course_filter| course_filter.apply(course_manifest));

        // Decide how to proceed based on the values of the course and lesson filters.
        match (&filter.course_filter, &filter.lesson_filter) {
            // There's no lesson nor course filter, so the course passes the filter.
            (None, None) => true,
            // There's only a course filter, so return whether the course passed the filter.
            (Some(_), None) => course_passes.unwrap_or(false),
            // There's only a lesson filter. Return false so that the course is skipped, and the
            // decision is made based on the lesson.
            (None, Some(_)) => false,
            // There's both a lesson and course filter. The behavior depends on the logical op used
            // in the filter.
            (Some(_), Some(_)) => match filter.op {
                // If the op is All, return whether the course passed the filter.
                FilterOp::All => course_passes.unwrap_or(false),
                // If the op is Any, return false so that the course is skipped and the decision is
                // made based on the lesson.
                FilterOp::Any => false,
            },
        }
    }

    /// Returns whether the lesson with the given lesson and course metadata passes the filter.
    pub fn lesson_passes_metadata_filter(
        filter: &MetadataFilter,
        course_manifest: &impl GetMetadata,
        lesson_manifest: &impl GetMetadata,
    ) -> bool {
        // Apply the course and lesson filters to the course and lesson metadata.
        let course_passes = filter
            .course_filter
            .as_ref()
            .map(|course_filter| course_filter.apply(course_manifest));
        let lesson_passes = filter
            .lesson_filter
            .as_ref()
            .map(|lesson_filter| lesson_filter.apply(lesson_manifest));

        // Decide how to proceed based on the values of the course and lesson filters.
        match (&filter.course_filter, &filter.lesson_filter) {
            // There's no lesson nor course filter, so the lesson passes the filter.
            (None, None) => true,
            // There's only a course filter, so return whether the course passed the filter.
            (Some(_), None) => course_passes.unwrap_or(false),
            // There's only a lesson filter, so return whether the lesson passed the filter.
            (None, Some(_)) => lesson_passes.unwrap_or(false),
            // There's both a lesson and course filter. The behavior depends on the logical op used
            // in the filter.
            (Some(_), Some(_)) => match filter.op {
                // If the op is All, return whether the lesson and the course passed the filters.
                FilterOp::All => lesson_passes.unwrap_or(false) && course_passes.unwrap_or(false),
                // If the op is Any, return whether the lesson or the course passed the filter.
                FilterOp::Any => lesson_passes.unwrap_or(false) || course_passes.unwrap_or(false),
            },
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
/// [UnitFilter](crate::data::filter::UnitFilter) defined inline or a reference to a
/// [SavedFilter](crate::data::filter::SavedFilter).
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
    pub fn duration(&self) -> u32 {
        match self {
            SessionPart::UnitFilter { duration, .. } => *duration,
            SessionPart::SavedFilter { duration, .. } => *duration,
            SessionPart::NoFilter { duration, .. } => *duration,
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

/// A specific instace of a study session. It contains the start time of the session and its
/// definition so that the scheduler knows the progress of the session.
#[derive(Clone, Debug)]
pub struct StudySessionData {
    /// The start time of the session.
    pub start_time: DateTime<Utc>,

    /// The definition of the session.
    pub definition: StudySession,
}

impl StudySessionData {
    /// Returns the study session part that should be practiced at the given time.
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

#[cfg(test)]
mod test {
    use anyhow::Result;
    use chrono::{Duration, Utc};
    use std::collections::BTreeMap;
    use ustr::Ustr;

    use crate::data::{
        filter::{
            FilterOp, FilterType, KeyValueFilter, MetadataFilter, SessionPart, StudySessionData,
            UnitFilter,
        },
        GetMetadata,
    };

    use super::StudySession;

    impl GetMetadata for BTreeMap<String, Vec<String>> {
        fn get_metadata(&self) -> Option<BTreeMap<String, Vec<String>>> {
            Some(self.clone())
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

    /// Verifies that a metadata filter with no course or lesson filter passes all courses and
    /// lessons.
    #[test]
    fn passes_metadata_filter_none() {
        let filter = MetadataFilter {
            course_filter: None,
            lesson_filter: None,
            op: FilterOp::Any,
        };
        let course_metadata = BTreeMap::new();
        let lesson_metadata = BTreeMap::new();
        assert!(UnitFilter::course_passes_metadata_filter(
            &filter,
            &course_metadata
        ));
        assert!(UnitFilter::lesson_passes_metadata_filter(
            &filter,
            &course_metadata,
            &lesson_metadata
        ));
    }

    /// Verifies correctly applying a basic key-value filter.
    #[test]
    fn apply_simple_filter() -> Result<()> {
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
        let include_filter = KeyValueFilter::BasicFilter {
            key: "key1".to_string(),
            value: "value1".to_string(),
            filter_type: FilterType::Include,
        };
        assert!(include_filter.apply(&metadata));
        let exclude_filter = KeyValueFilter::BasicFilter {
            key: "key1".to_string(),
            value: "value1".to_string(),
            filter_type: FilterType::Exclude,
        };
        assert!(!exclude_filter.apply(&metadata));
        Ok(())
    }

    /// Verifies applying a basic key-value filter to metadata that doesn't contain the required
    /// keys or values.
    #[test]
    fn apply_simple_filter_no_match() -> Result<()> {
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

        // The key-value pair doesn't exist in the metadata so the filter should not apply.
        let include_filter = KeyValueFilter::BasicFilter {
            key: "key10".to_string(),
            value: "value1".to_string(),
            filter_type: FilterType::Include,
        };
        assert!(!include_filter.apply(&metadata));

        // The same key-value pair should apply to the exclude filter.
        let exclude_filter = KeyValueFilter::BasicFilter {
            key: "key10".to_string(),
            value: "value1".to_string(),
            filter_type: FilterType::Exclude,
        };
        assert!(exclude_filter.apply(&metadata));
        Ok(())
    }

    /// Verifies applying a combined key-value filter with the ALL operator.
    #[test]
    fn apply_combined_all_filter() -> Result<()> {
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
                KeyValueFilter::BasicFilter {
                    key: "key1".to_string(),
                    value: "value1".to_string(),
                    filter_type: FilterType::Include,
                },
                KeyValueFilter::BasicFilter {
                    key: "key2".to_string(),
                    value: "value4".to_string(),
                    filter_type: FilterType::Include,
                },
            ],
        };
        assert!(filter.apply(&metadata));
        Ok(())
    }

    /// Verifies applying a combined key-value filter with the ALL operator to a key-value pair that
    /// does not pass the filter.
    #[test]
    fn apply_combined_all_filter_no_match() -> Result<()> {
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
                KeyValueFilter::BasicFilter {
                    key: "key1".to_string(),
                    value: "value1".to_string(),
                    filter_type: FilterType::Include,
                },
                KeyValueFilter::BasicFilter {
                    key: "key2".to_string(),
                    value: "value5".to_string(),
                    filter_type: FilterType::Include,
                },
            ],
        };
        assert!(!filter.apply(&metadata));
        Ok(())
    }

    /// Verifies applying a combined key-value filter with the ANY operator.
    #[test]
    fn apply_combined_any_filter() -> Result<()> {
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
                KeyValueFilter::BasicFilter {
                    key: "key1".to_string(),
                    value: "value1".to_string(),
                    filter_type: FilterType::Include,
                },
                KeyValueFilter::BasicFilter {
                    key: "key2".to_string(),
                    value: "value5".to_string(),
                    filter_type: FilterType::Include,
                },
            ],
        };
        assert!(filter.apply(&metadata));
        Ok(())
    }

    /// Verifies applying a combined key-value filter with the ANY operator to a key-value pair that
    /// does not pass the filter.
    #[test]
    fn apply_combined_any_filter_no_match() -> Result<()> {
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
                KeyValueFilter::BasicFilter {
                    key: "key1".to_string(),
                    value: "value3".to_string(),
                    filter_type: FilterType::Include,
                },
                KeyValueFilter::BasicFilter {
                    key: "key2".to_string(),
                    value: "value5".to_string(),
                    filter_type: FilterType::Include,
                },
            ],
        };
        assert!(!filter.apply(&metadata));
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
            filter: MetadataFilter {
                course_filter: None,
                lesson_filter: None,
                op: FilterOp::Any,
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
