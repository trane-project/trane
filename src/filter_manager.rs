//! Contains utilities to use filters saved by the user.
//!
//! Trane's default mode for scheduling exercises is to traverse the entire graph. Sometimes,
//! students want to only schedule exercises from a subset of the graph. This module allows them to
//! re-use filters they have previously saved.

use anyhow::{Context, Result};
use std::{collections::HashMap, fs::File, io::BufReader};

use crate::data::filter::NamedFilter;

/// A trait with functions to manage saved filters. Each filter is given a unique name to use as an
/// identifier and contains a `UnitFilter`.
pub trait FilterManager {
    /// Gets the filter with the given ID.
    fn get_filter(&self, id: &str) -> Option<NamedFilter>;

    /// Returns a list of filter IDs and descriptions.
    fn list_filters(&self) -> Vec<(String, String)>;
}

/// An implementation of [FilterManager] backed by the local file system.
pub(crate) struct LocalFilterManager {
    /// A map of filter IDs to filters.
    filters: HashMap<String, NamedFilter>,
}

impl LocalFilterManager {
    /// Scans all `NamedFilters` in the given directory and returns a map of filters.
    fn scan_filters(filter_directory: &str) -> Result<HashMap<String, NamedFilter>> {
        let mut filters = HashMap::new();
        for entry in std::fs::read_dir(filter_directory)
            .with_context(|| format!("Failed to read filter directory {}", filter_directory))?
        {
            // Try to read the file as a [NamedFilter].
            let entry = entry.with_context(|| "Failed to read file entry for saved filter")?;
            let file = File::open(entry.path()).with_context(|| {
                format!(
                    "Failed to open saved filter file {}",
                    entry.path().display()
                )
            })?;
            let reader = BufReader::new(file);
            let filter: NamedFilter = serde_json::from_reader(reader).with_context(|| {
                format!(
                    "Failed to parse named filter from {}",
                    entry.path().display()
                )
            })?;

            // Check for duplicate IDs before inserting the filter.
            if filters.contains_key(&filter.id) {
                return Err(anyhow::anyhow!(
                    "Found multiple filters with ID {}",
                    filter.id
                ));
            }
            filters.insert(filter.id.clone(), filter);
        }
        Ok(filters)
    }

    /// Creates a new `LocalFilterManager`.
    pub fn new(filter_directory: &str) -> Result<LocalFilterManager> {
        Ok(LocalFilterManager {
            filters: LocalFilterManager::scan_filters(filter_directory)?,
        })
    }
}

impl FilterManager for LocalFilterManager {
    fn get_filter(&self, id: &str) -> Option<NamedFilter> {
        self.filters.get(id).cloned()
    }

    fn list_filters(&self) -> Vec<(String, String)> {
        // Create a list of (ID, description) pairs.
        let mut filters: Vec<(String, String)> = self
            .filters
            .iter()
            .map(|(id, filter)| (id.clone(), filter.description.clone()))
            .collect();

        // Sort the filters by their IDs.
        filters.sort_by(|a, b| a.0.cmp(&b.0));
        filters
    }
}

#[cfg(test)]
mod test {
    use anyhow::{Ok, Result};
    use std::path::Path;
    use tempfile::TempDir;
    use ustr::Ustr;

    use crate::{
        data::filter::{
            FilterOp, FilterType, KeyValueFilter, MetadataFilter, NamedFilter, UnitFilter,
        },
        filter_manager::FilterManager,
    };

    use super::LocalFilterManager;

    /// Creates some unit filters for testing.
    fn test_filters() -> Vec<NamedFilter> {
        vec![
            NamedFilter {
                id: "filter1".to_string(),
                description: "Filter 1".to_string(),
                filter: UnitFilter::CourseFilter {
                    course_ids: vec![Ustr::from("course1")],
                },
            },
            NamedFilter {
                id: "filter2".to_string(),
                description: "Filter 2".to_string(),
                filter: UnitFilter::MetadataFilter {
                    filter: MetadataFilter {
                        op: FilterOp::All,
                        lesson_filter: Some(KeyValueFilter::BasicFilter {
                            key: "key1".to_string(),
                            value: "value1".to_string(),
                            filter_type: FilterType::Include,
                        }),
                        course_filter: Some(KeyValueFilter::BasicFilter {
                            key: "key2".to_string(),
                            value: "value2".to_string(),
                            filter_type: FilterType::Include,
                        }),
                    },
                },
            },
        ]
    }

    /// Writes the filters to the given directory.
    fn write_filters(filters: Vec<NamedFilter>, dir: &Path) -> Result<()> {
        for filter in filters {
            // Give each file a unique name.
            let timestamp_ns = chrono::Utc::now().timestamp_nanos();
            let filter_path = dir.join(format!("{}_{}.json", filter.id, timestamp_ns));
            let filter_json = serde_json::to_string(&filter)?;
            std::fs::write(filter_path, filter_json)?;
        }
        Ok(())
    }

    #[test]
    fn filter_manager() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let filters = test_filters();
        write_filters(filters.clone(), temp_dir.path())?;
        let manager = LocalFilterManager::new(temp_dir.path().to_str().unwrap())?;

        let filter_list = manager.list_filters();
        assert_eq!(
            filter_list,
            vec![
                ("filter1".to_string(), "Filter 1".to_string()),
                ("filter2".to_string(), "Filter 2".to_string())
            ]
        );

        for (index, (id, _)) in filter_list.iter().enumerate() {
            let filter = manager.get_filter(&id);
            assert!(filter.is_some());
            let filter = filter.unwrap();
            assert_eq!(filters[index], filter);
        }
        Ok(())
    }

    #[test]
    fn filters_repeated_ids() -> Result<()> {
        let filters = vec![
            NamedFilter {
                id: "filter1".to_string(),
                description: "Filter 1".to_string(),
                filter: UnitFilter::CourseFilter {
                    course_ids: vec![Ustr::from("course1")],
                },
            },
            NamedFilter {
                id: "filter1".to_string(),
                description: "Filter 1".to_string(),
                filter: UnitFilter::CourseFilter {
                    course_ids: vec![Ustr::from("course1")],
                },
            },
        ];

        let temp_dir = TempDir::new()?;
        write_filters(filters.clone(), temp_dir.path())?;
        assert!(LocalFilterManager::new(temp_dir.path().to_str().unwrap()).is_err());
        Ok(())
    }
}
