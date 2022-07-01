#[cfg(test)]
mod tests;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::data::GetMetadata;

/// The logical operation used to combine multiple filters.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum FilterOp {
    /// A filter returns true if all its filters pass.
    All,

    /// A filter returns true if at least one of its filters pass.
    Any,
}

/// The type of filter according to how they treat the items which match the filter.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum FilterType {
    /// A filter which includes the items that match it.
    Include,

    /// A filter which excludes the items that match it.
    Exclude,
}

/// A filter on the metadata of a course.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum KeyValueFilter {
    BasicFilter {
        key: String,
        value: String,
        filter_type: FilterType,
    },

    /// A combination of filters on the metadata.
    CombinedFilter {
        op: FilterOp,
        filters: Vec<KeyValueFilter>,
    },
}

impl KeyValueFilter {
    /// Applies the filter to the given map of keys and values.
    fn apply_metadata(
        metadata: &HashMap<String, Vec<String>>,
        key: &str,
        value: &str,
        filter_type: FilterType,
    ) -> bool {
        let contains_metadata = if !metadata.contains_key(key) {
            if filter_type == FilterType::Include {
                false
            } else {
                true
            }
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
        let default_metadata = HashMap::default();
        let metadata = manifest.get_metadata().unwrap_or(&default_metadata);
        match self {
            KeyValueFilter::BasicFilter {
                key,
                value,
                filter_type,
            } => KeyValueFilter::apply_metadata(metadata, key, value, filter_type.clone()),
            KeyValueFilter::CombinedFilter { op, filters } => {
                let mut results = filters.iter().map(|f| f.apply(manifest));
                match op {
                    &FilterOp::All => {
                        return results.all(|x| x);
                    }
                    &FilterOp::Any => {
                        return results.any(|x| x);
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MetadataFilter {
    pub course_filter: Option<KeyValueFilter>,
    pub lesson_filter: Option<KeyValueFilter>,
    pub op: FilterOp,
}

/// A filter on a course or lesson manifest.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum UnitFilter {
    /// A filter on a course ID.
    CourseFilter { course_id: String },

    /// A filter on a lesson ID.
    LessonFilter { lesson_id: String },

    /// A filter on the metadata of a course or lesson.
    MetadataFilter { filter: MetadataFilter },
}

/// A named filter for easy reference.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct NamedFilter {
    /// A unique ID for the filter.
    pub id: String,

    /// A human-readable description of the filter.
    pub description: String,

    /// The filter to apply.
    pub filter: UnitFilter,
}
