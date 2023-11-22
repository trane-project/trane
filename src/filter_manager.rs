//! Contains utilities to use filters saved by the user.
//!
//! Trane's default mode for scheduling exercises is to traverse the entire graph. Sometimes,
//! students want to only schedule exercises from a subset of the graph. This module allows them to
//! re-use filters they have previously saved.

use anyhow::{bail, Context, Result};
use std::{collections::HashMap, fs::File, io::BufReader};

use crate::data::filter::SavedFilter;

/// A trait with functions to manage saved filters. Each filter is given a unique name to use as an
/// identifier and contains a `UnitFilter`.
pub trait FilterManager {
    /// Gets the filter with the given ID.
    fn get_filter(&self, id: &str) -> Option<SavedFilter>;

    /// Returns a list of filter IDs and descriptions.
    fn list_filters(&self) -> Vec<(String, String)>;
}

/// An implementation of [`FilterManager`] backed by the local file system.
pub struct LocalFilterManager {
    /// A map of filter IDs to filters.
    pub filters: HashMap<String, SavedFilter>,
}

impl LocalFilterManager {
    /// Scans all `NamedFilters` in the given directory and returns a map of filters.
    fn scan_filters(filter_directory: &str) -> Result<HashMap<String, SavedFilter>> {
        let mut filters = HashMap::new();
        for entry in std::fs::read_dir(filter_directory)
            .with_context(|| format!("Failed to read filter directory {filter_directory}"))?
        {
            // Try to read the file as a `NamedFilter`.
            let entry = entry.with_context(|| "Failed to read file entry for saved filter")?;
            let file = File::open(entry.path()).with_context(|| {
                format!(
                    "Failed to open saved filter file {}",
                    entry.path().display()
                )
            })?;
            let reader = BufReader::new(file);
            let filter: SavedFilter = serde_json::from_reader(reader).with_context(|| {
                format!(
                    "Failed to parse named filter from {}",
                    entry.path().display()
                )
            })?;

            // Check for duplicate IDs before inserting the filter.
            if filters.contains_key(&filter.id) {
                bail!("Found multiple filters with ID {}", filter.id);
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
    fn get_filter(&self, id: &str) -> Option<SavedFilter> {
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
    use std::{os::unix::prelude::PermissionsExt, path::Path};
    use tempfile::TempDir;
    use ustr::Ustr;

    use crate::{
        data::filter::{FilterOp, FilterType, KeyValueFilter, SavedFilter, UnitFilter},
        filter_manager::FilterManager,
    };

    use super::LocalFilterManager;

    /// Creates some unit filters for testing.
    fn test_filters() -> Vec<SavedFilter> {
        vec![
            SavedFilter {
                id: "filter1".to_string(),
                description: "Filter 1".to_string(),
                filter: UnitFilter::CourseFilter {
                    course_ids: vec![Ustr::from("course1")],
                },
            },
            SavedFilter {
                id: "filter2".to_string(),
                description: "Filter 2".to_string(),
                filter: UnitFilter::MetadataFilter {
                    filter: KeyValueFilter::CombinedFilter {
                        op: FilterOp::All,
                        filters: vec![
                            KeyValueFilter::LessonFilter {
                                key: "key1".to_string(),
                                value: "value1".to_string(),
                                filter_type: FilterType::Include,
                            },
                            KeyValueFilter::CombinedFilter {
                                op: FilterOp::Any,
                                filters: vec![
                                    KeyValueFilter::CourseFilter {
                                        key: "key2".to_string(),
                                        value: "value2".to_string(),
                                        filter_type: FilterType::Include,
                                    },
                                    KeyValueFilter::CourseFilter {
                                        key: "key3".to_string(),
                                        value: "value3".to_string(),
                                        filter_type: FilterType::Include,
                                    },
                                ],
                            },
                        ],
                    },
                },
            },
        ]
    }

    /// Writes the filters to the given directory.
    fn write_filters(filters: Vec<SavedFilter>, dir: &Path) -> Result<()> {
        for filter in filters {
            // Give each file a unique name.
            let timestamp_ns = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
            let filter_path = dir.join(format!("{}_{}.json", filter.id, timestamp_ns));
            let filter_json = serde_json::to_string(&filter)?;
            std::fs::write(filter_path, filter_json)?;
        }
        Ok(())
    }

    /// Verifies creating a filter manager with valid filters.
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

    /// Verifies that filters with repeated IDs cause the filter manager to fail.
    #[test]
    fn filters_repeated_ids() -> Result<()> {
        let filters = vec![
            SavedFilter {
                id: "filter1".to_string(),
                description: "Filter 1".to_string(),
                filter: UnitFilter::CourseFilter {
                    course_ids: vec![Ustr::from("course1")],
                },
            },
            SavedFilter {
                id: "filter1".to_string(),
                description: "Filter 1".to_string(),
                filter: UnitFilter::LessonFilter {
                    lesson_ids: vec![Ustr::from("lesson1")],
                },
            },
            SavedFilter {
                id: "filter1".to_string(),
                description: "Filter 1".to_string(),
                filter: UnitFilter::ReviewListFilter,
            },
        ];

        let temp_dir = TempDir::new()?;
        write_filters(filters.clone(), temp_dir.path())?;
        assert!(LocalFilterManager::new(temp_dir.path().to_str().unwrap()).is_err());
        Ok(())
    }

    /// Verifies that trying to read filters from an invalid directory fails.
    #[test]
    fn read_bad_directory() -> Result<()> {
        assert!(LocalFilterManager::new("bad_directory").is_err());
        Ok(())
    }

    /// Verifies that filters in an invalid format cause the filter manager to fail.
    #[test]
    fn read_bad_file_format() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let bad_file = temp_dir.path().join("bad_file.json");
        std::fs::write(bad_file, "bad json")?;
        assert!(LocalFilterManager::new(temp_dir.path().to_str().unwrap()).is_err());
        Ok(())
    }

    /// Verifies that filters with bad permissions cause the filter manager to fail.
    #[test]
    fn read_bad_file_permissions() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let bad_file = temp_dir.path().join("bad_file.json");
        std::fs::write(bad_file.clone(), "bad json")?;
        std::fs::set_permissions(bad_file, std::fs::Permissions::from_mode(0o000))?;
        assert!(LocalFilterManager::new(temp_dir.path().to_str().unwrap()).is_err());
        Ok(())
    }
}
