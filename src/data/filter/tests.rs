use std::collections::HashMap;

use anyhow::Result;

use super::{FilterType, KeyValueFilter};
use crate::data::{filter::FilterOp, GetMetadata};

impl GetMetadata for HashMap<String, Vec<String>> {
    fn get_metadata(&self) -> Option<&HashMap<String, Vec<String>>> {
        Some(self)
    }
}

#[test]
fn apply_simple_filter() -> Result<()> {
    let metadata = HashMap::from([
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
    let metadata = HashMap::from([
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
    let metadata = HashMap::from([
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
