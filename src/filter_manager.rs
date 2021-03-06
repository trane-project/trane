//! Module containing utilities to manipulate the filters saved under the .trane/filters directory.
#[cfg(test)]
mod test;

use std::{collections::HashMap, fs::File, io::BufReader};

use anyhow::Result;

use crate::data::filter::NamedFilter;

/// Contains functions to manage saved filters.
pub trait FilterManager {
    /// Gets the filter with the given ID.
    fn get_filter(&self, id: &str) -> Option<NamedFilter>;

    /// Returns a list of filter IDs and descriptions.
    fn list_filters(&self) -> Vec<(String, String)>;
}

/// An implementation of FilterManager backed by the local filesystem.
pub(crate) struct LocalFilterManager {
    /// A map of filter IDs to filters.
    filters: HashMap<String, NamedFilter>,
}

impl LocalFilterManager {
    /// Scans all NamedFilters in the given directory and returns a map of filters.
    fn scan_filters(filter_directory: &str) -> Result<HashMap<String, NamedFilter>> {
        let mut filters = HashMap::new();
        for entry in std::fs::read_dir(filter_directory)? {
            let file = File::open(entry?.path())?;
            let reader = BufReader::new(file);
            let filter: NamedFilter = serde_json::from_reader(reader)?;
            filters.insert(filter.id.clone(), filter);
        }
        Ok(filters)
    }

    /// Creates a new LocalFilterManager.
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
        let mut filters: Vec<(String, String)> = self
            .filters
            .iter()
            .map(|(id, filter)| (id.clone(), filter.description.clone()))
            .collect();
        filters.sort_by(|a, b| a.0.cmp(&b.0));
        filters
    }
}
