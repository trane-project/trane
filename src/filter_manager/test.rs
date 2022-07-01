use std::path::Path;

use anyhow::{Ok, Result};
use tempfile::TempDir;

use crate::{
    data::filter::{FilterOp, FilterType, KeyValueFilter, MetadataFilter, NamedFilter, UnitFilter},
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
                course_id: "course1".to_string(),
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
        let filter_path = dir.join(format!("{}.json", filter.id));
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
