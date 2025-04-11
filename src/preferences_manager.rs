//! A module containing methods to read and write user preferences.

use anyhow::{Context, Result};
use std::{fs, path::PathBuf};

use crate::{PreferencesManagerError, data::UserPreferences};

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
        let raw_preferences =
            &fs::read_to_string(&self.path).context("failed to read user preferences")?;
        let preferences = serde_json::from_str::<UserPreferences>(raw_preferences)
            .context("invalid user preferences")?;
        Ok(preferences)
    }

    /// Helper function to set the user preferences to the given value.
    fn set_user_preferences_helper(&self, preferences: &UserPreferences) -> Result<()> {
        let pretty_json =
            serde_json::to_string_pretty(preferences).context("invalid user preferences")?;
        fs::write(&self.path, pretty_json).context("failed to write user preferences")
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
    use std::{fs, os::unix::fs::PermissionsExt};

    use anyhow::Result;
    use tempfile::tempdir;

    use crate::{
        USER_PREFERENCES_PATH,
        data::UserPreferences,
        preferences_manager::{LocalPreferencesManager, PreferencesManager},
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

    /// Verifies that an error is returned when the preferences file is missing.
    #[test]
    fn missing_preferences_file() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join(USER_PREFERENCES_PATH);
        let manager = LocalPreferencesManager { path };
        assert!(manager.get_user_preferences().is_err());
    }

    /// Verifies that an error is returned when the preferences file cannot be written.
    #[test]
    fn unwritable_preferences_file() -> Result<()> {
        let temp_dir = tempdir().unwrap();
        fs::set_permissions(temp_dir.path(), fs::Permissions::from_mode(0o0))?;
        let path = temp_dir.path().join(USER_PREFERENCES_PATH);
        let mut manager = LocalPreferencesManager { path };
        let preferences = UserPreferences::default();
        assert!(manager.set_user_preferences(preferences).is_err());
        Ok(())
    }
}
