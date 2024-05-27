//! A module containing methods to read and write user preferences.

use anyhow::{anyhow, Context, Result};
use std::{fs::File, io::BufReader, path::PathBuf};

use crate::{data::UserPreferences, PreferencesManagerError};

/// A trait for managing user preferences.
pub trait PreferencesManager {
    /// Gets the current user preferences.
    fn get_user_preferences(&self) -> Result<UserPreferences, PreferencesManagerError>;

    /// Sets the user preferences to the given value.
    fn set_user_preferences(
        &mut self,
        preferences: UserPreferences,
    ) -> Result<(), PreferencesManagerError>;
}

/// A preferences manager backed by a local file containing a serialized `UserPreferences` object.
pub struct LocalPreferencesManager {
    /// The path to the user preferences file.
    pub path: PathBuf,
}

impl LocalPreferencesManager {
    /// Helper function to get the current user preferences.
    fn get_user_preferences_helper(&self) -> Result<UserPreferences> {
        let file = File::open(self.path.clone()).with_context(|| {
            anyhow!("cannot open user preferences file {}", self.path.display())
        })?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader)
            .with_context(|| anyhow!("cannot parse user preferences file {}", self.path.display()))
    }

    /// Helper function to set the user preferences to the given value.
    fn set_user_preferences_helper(&self, preferences: &UserPreferences) -> Result<()> {
        let file = File::create(self.path.clone()).with_context(|| {
            anyhow!(
                "cannot create user preferences file {}",
                self.path.display()
            )
        })?;
        serde_json::to_writer(file, &preferences).with_context(|| {
            anyhow!(
                "cannot serialize user preferences to file at {}",
                self.path.display()
            )
        })
    }
}

impl PreferencesManager for LocalPreferencesManager {
    fn get_user_preferences(&self) -> Result<UserPreferences, PreferencesManagerError> {
        self.get_user_preferences_helper()
            .map_err(PreferencesManagerError::GetUserPreferences)
    }

    fn set_user_preferences(
        &mut self,
        preferences: UserPreferences,
    ) -> Result<(), PreferencesManagerError> {
        self.set_user_preferences_helper(&preferences)
            .map_err(PreferencesManagerError::SetUserPreferences)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use tempfile::tempdir;

    use crate::{
        data::UserPreferences,
        preferences_manager::{LocalPreferencesManager, PreferencesManager},
        USER_PREFERENCES_PATH,
    };

    /// Verifies setting and getting user preferences using the local filesystem.
    #[test]
    fn local_preferences_manager() -> Result<()> {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join(USER_PREFERENCES_PATH);

        let mut manager = LocalPreferencesManager { path };

        // Set and get the default user preferences.
        let preferences = UserPreferences::default();
        assert!(manager.get_user_preferences().is_err());
        manager.set_user_preferences(preferences.clone())?;
        assert_eq!(manager.get_user_preferences()?, preferences);

        // Set and get modified user preferences.
        let new_preferences = UserPreferences {
            ignored_paths: vec!["foo".to_string(), "bar".to_string()],
            ..Default::default()
        };
        manager.set_user_preferences(new_preferences.clone())?;
        assert_eq!(manager.get_user_preferences()?, new_preferences);
        Ok(())
    }
}
