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
    /// Applies the filter to the given map of keys and values.
    fn apply_metadata(
        metadata: &BTreeMap<String, Vec<String>>,
        key: &str,
        value: &str,
        filter_type: FilterType,
    ) -> bool {
        let contains_metadata = if !metadata.contains_key(key) {
            filter_type != FilterType::Include
        } else {
            metadata
                .get(key)
                .unwrap_or(&Vec::new())
                .contains(&value.to_string())
        };
        match filter_type {
            FilterType::Include => contains_metadata,
            FilterType::Exclude => !contains_metadata,
        }
    }

    /// Applies the filter to the given manifest.
    pub fn apply(&self, manifest: &impl GetMetadata) -> bool {
        let default_metadata = BTreeMap::default();
        let metadata = manifest.get_metadata().unwrap_or(&default_metadata);
        match self {
            KeyValueFilter::BasicFilter {
                key,
                value,
                filter_type,
            } => KeyValueFilter::apply_metadata(metadata, key, value, filter_type.clone()),
            KeyValueFilter::CombinedFilter { op, filters } => {
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
    pub course_filter: Option<KeyValueFilter>,

    /// The filter to apply on lesson metadata.
    pub lesson_filter: Option<KeyValueFilter>,

    /// The logical operation used to combine the course and lesson filters.
    pub op: FilterOp,
}

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

    /// A filter to indicate that only exercises from the review list should be scheduled.
    ReviewListFilter,
}

impl UnitFilter {
    /// Applies the course filter to the course with the given ID.
    pub fn apply_course_id(&self, course_id: &Ustr) -> bool {
        match self {
            UnitFilter::CourseFilter { course_ids } => course_ids.contains(course_id),
            _ => false,
        }
    }

    /// Applies the lesson filter to the lesson with the given ID.
    pub fn apply_lesson_id(&self, lesson_id: &Ustr) -> bool {
        match self {
            UnitFilter::LessonFilter { lesson_ids } => lesson_ids.contains(lesson_id),
            _ => false,
        }
    }

    /// Applies the metadata filter to the course with the given manifest.
    pub fn apply_course_metadata(&self, manifest: &impl GetMetadata) -> bool {
        match self {
            UnitFilter::MetadataFilter { filter } => match &filter.course_filter {
                Some(course_filter) => course_filter.apply(manifest),
                // A value of `None` returns true so that the final determination is based on the
                // lesson metadata.
                None => true,
            },
            _ => false,
        }
    }

    /// Applies the metadata filter to the lesson with the given lesson and course manifests.
    pub fn apply_lesson_metadata(
        &self,
        lesson_manifest: &impl GetMetadata,
        course_manifest: &impl GetMetadata,
    ) -> bool {
        match self {
            UnitFilter::MetadataFilter { filter } => {
                match (&filter.course_filter, &filter.lesson_filter) {
                    // None of the filters are set, so every lesson passes the filter.
                    (None, None) => true,
                    // Only the course filter is set, so the lesson passes the filter if the course
                    // passes the filter.
                    (Some(course_filter), None) => course_filter.apply(course_manifest),
                    // Only the lesson filter is set, so the lesson passes the filter if the lesson
                    // passes the filter.
                    (None, Some(lesson_filter)) => lesson_filter.apply(lesson_manifest),
                    // Both filters are set, so the result depends on the logical operation used in
                    // the filter.
                    (Some(course_filter), Some(lesson_filter)) => match filter.op {
                        // If the op is `All`, the lesson passes the filter if both the course and
                        // lesson filters pass.
                        FilterOp::All => {
                            course_filter.apply(course_manifest)
                                && lesson_filter.apply(lesson_manifest)
                        }
                        // If the op is `Any`, the lesson passes the filter if either the course or
                        // the lesson filters pass.
                        FilterOp::Any => {
                            course_filter.apply(course_manifest)
                                || lesson_filter.apply(lesson_manifest)
                        }
                    },
                }
            }
            _ => false,
        }
    }
}

/// A named filter for easy reference.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NamedFilter {
    /// A unique ID for the filter.
    pub id: String,

    /// A human-readable description of the filter.
    pub description: String,

    /// The filter to apply.
    pub filter: UnitFilter,
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use std::collections::BTreeMap;

    use crate::data::{
        filter::{FilterOp, FilterType, KeyValueFilter},
        GetMetadata,
    };

    impl GetMetadata for BTreeMap<String, Vec<String>> {
        fn get_metadata(&self) -> Option<&BTreeMap<String, Vec<String>>> {
            Some(self)
        }
    }

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
        let filter = KeyValueFilter::BasicFilter {
            key: "key1".to_string(),
            value: "value1".to_string(),
            filter_type: FilterType::Include,
        };
        assert!(filter.apply(&metadata));
        Ok(())
    }

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
                    value: "value5".to_string(),
                    filter_type: FilterType::Include,
                },
            ],
        };
        assert!(!filter.apply(&metadata));
        Ok(())
    }

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
}
