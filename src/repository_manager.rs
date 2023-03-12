//! A module containing functions to download and manage courses from git repositories, which is
//! meant to simplify the process of adding new courses to Trane.

use anyhow::{bail, Result};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};
use url::Url;

use crate::{
    data::RepositoryMetadata, error::RepositoryError, DOWNLOAD_DIRECTORY, REPOSITORY_DIRECTORY,
    TRANE_CONFIG_DIR_PATH,
};

/// The prefix for HTTPS URLs. Only HTTP URLs are supported at the moment because SSH URLs require
/// Trane to have access to the user's SSH keys.
const HTTPS_PREFIX: &str = "https://";

/// A trait with function to manage git repositories of courses, with functions to add new
/// repositories, remove existing ones, and update repositories to the latest version.
pub trait RepositoryManager {
    /// Downloads the courses from the given git repository into the given directory. The ID will
    /// also be used to identify the repository in the future and as the name of the directory. If
    /// ommitted, the name of the repository will be used to generate an ID.
    fn add_repo(&mut self, url: &str, repo_id: Option<String>) -> Result<()>;

    /// Removes the repository with the given ID.
    fn remove_repo(&mut self, repo_id: &str) -> Result<()>;

    /// Attempts to pull the latest version of the given repository.
    fn update_repo(&self, repo_id: &str) -> Result<()>;

    /// Attempts to pull the latest version of all repositories.
    fn update_all_repos(&self) -> Result<()>;

    /// Returns a list of all the repositories that are currently being managed.
    fn list_repos(&self) -> Result<Vec<RepositoryMetadata>>;
}

/// An implementation of [RepositoryManager] backed by the local file system. All repositories will
/// be downloaded to the `managed_courses` directory in the root of the Trane library.
pub struct LocalRepositoryManager {
    /// A map of repository IDs to its metadata.
    repositories: HashMap<String, RepositoryMetadata>,

    /// The path to the directory where repositories will be downloaded.
    download_directory: PathBuf,

    /// The path to the directory where repository metadata will be stored.
    metadata_directory: PathBuf,
}

impl LocalRepositoryManager {
    /// Returns the default ID for the repository based on the URL.
    fn id_from_url(url: Url) -> Result<String> {
        Ok(url
            .path_segments()
            .and_then(|segments| segments.last())
            .ok_or_else(|| RepositoryError::InvalidRepositoryURL(url.to_string()))?
            .trim_end_matches(".git")
            .to_owned())
    }

    /// Reads the repository metadata from the given path.
    fn read_metadata(path: &Path) -> Result<RepositoryMetadata> {
        let repo = serde_json::from_str::<RepositoryMetadata>(
            &fs::read_to_string(path)
                .map_err(|_| RepositoryError::InvalidRepositoryMetadata(path.to_owned()))?, // grcov-excl-line
        )
        .map_err(|_| RepositoryError::InvalidRepositoryMetadata(path.to_owned()))?; // grcov-excl-line
        Ok(repo)
    }

    /// Writes the repository metadata to metadata directory.
    fn write_metadata(&self, metadata: &RepositoryMetadata) -> Result<()> {
        let path = self
            .metadata_directory
            .join(format!("{}.json", metadata.id));
        fs::write(&path, serde_json::to_string_pretty(metadata)?)
            .map_err(|_| RepositoryError::InvalidRepositoryMetadata(path))?; // grcov-excl-line
        Ok(())
    }

    /// Clones the repository at the given URL into the given directory. If the directory already
    /// exists, it will be deleted and replaced with the new repository.
    fn clone_repo(&self, url: &str, repo_id: &str) -> Result<()> {
        // The clone path must be a directory if it exists.
        let clone_dir = self.download_directory.join(repo_id);
        if clone_dir.exists() && !clone_dir.is_dir() {
            bail!(RepositoryError::InvalidDownloadDirectory(clone_dir));
        }

        // Clone the repo into a temp directory.
        let temp_dir = tempfile::tempdir()?;
        let temp_clone_path = temp_dir.path().join(repo_id);
        git2::Repository::clone(url, &temp_clone_path)
            .map_err(|_| RepositoryError::CloneRepository(url.to_string()))?;

        // Copy the repo into the download directory.
        if clone_dir.exists() {
            fs::remove_dir_all(&clone_dir)
                .map_err(|_| RepositoryError::InvalidDownloadDirectory(clone_dir.to_owned()))?;
        }
        fs::create_dir(&clone_dir)
            .map_err(|_| RepositoryError::InvalidDownloadDirectory(clone_dir.to_owned()))?;
        fs_extra::copy_items(
            &[temp_clone_path.to_str().unwrap()],
            &self.download_directory,
            &fs_extra::dir::CopyOptions::new().copy_inside(true),
        )
        .map_err(|_| RepositoryError::InvalidDownloadDirectory(clone_dir.to_owned()))?;
        Ok(())
    }

    /// Opens the download directory and tracks all the existing repositories.
    pub fn new(library_root: &Path) -> Result<LocalRepositoryManager> {
        // Craete the repository manager and the repository directory if it doesn't exist.
        let repo_dir = library_root
            .join(TRANE_CONFIG_DIR_PATH)
            .join(REPOSITORY_DIRECTORY);
        if !repo_dir.exists() {
            fs::create_dir(&repo_dir).map_err(|_| RepositoryError::InvalidMetadataDirectory)?;
        }
        let mut manager = LocalRepositoryManager {
            repositories: HashMap::new(),
            download_directory: library_root.join(DOWNLOAD_DIRECTORY),
            metadata_directory: repo_dir.clone(),
        };

        // Read the repository directory and add all the repositories to the map.
        let read_repo_dir =
            fs::read_dir(&repo_dir).map_err(|_| RepositoryError::InvalidMetadataDirectory)?;
        for entry in read_repo_dir {
            // Ignore any directories, invalid files, or files that are not JSON.
            // grcov-excl-start: These error conditions are not possible if the directory is
            // properly managed by Trane.
            if entry.is_err() {
                continue;
            }
            let entry = entry.unwrap();
            if !entry.path().is_file() || entry.path().extension().unwrap_or_default() != "json" {
                continue;
            }
            // grcov-excl-end

            // Read the repository metadata and add it to the map.
            let repo_metadata = Self::read_metadata(&entry.path())?;
            manager
                .repositories
                .insert(repo_metadata.id.clone(), repo_metadata.clone());

            // Verify that the repository exists and is a valid git repository.
            let download_directory = library_root
                .join(DOWNLOAD_DIRECTORY)
                .join(&repo_metadata.id);
            if !download_directory.exists() {
                bail!(RepositoryError::InvalidDownloadDirectory(
                    download_directory
                ));
            }
            if git2::Repository::open(&download_directory).is_err() {
                bail!(RepositoryError::InvalidRepository(download_directory));
            }
        }
        Ok(manager)
    }
}

impl RepositoryManager for LocalRepositoryManager {
    fn add_repo(&mut self, url: &str, repo_id: Option<String>) -> Result<()> {
        // Check that the repository URL is not an SSH URL.
        if !url.starts_with(HTTPS_PREFIX) {
            bail!(RepositoryError::InvalidRepositoryURL(url.to_string()));
        }

        // Extract the repository ID from the URL if it wasn't provided.
        let parsed_url = url
            .parse::<Url>()
            .map_err(|_| RepositoryError::InvalidRepositoryURL(url.to_string()))?;
        let repo_id = if let Some(repo_id) = repo_id {
            repo_id
        } else {
            Self::id_from_url(parsed_url)?
        };

        // Check that no other repository has the same ID.
        if self.repositories.contains_key(&repo_id) {
            bail!(RepositoryError::DuplicateRepository(repo_id));
        }

        // Clone the repository into the download directory.
        self.clone_repo(url, &repo_id)?;

        // Add the metadata to the repository directory and the map.
        let repo_metadata = RepositoryMetadata {
            id: repo_id.clone(),
            url: url.to_string(),
        };
        self.write_metadata(&repo_metadata)?;
        self.repositories.insert(repo_id, repo_metadata);
        Ok(())
    }

    fn remove_repo(&mut self, repo_id: &str) -> Result<()> {
        // Do nothing if no repository with the given ID exists.
        if !self.repositories.contains_key(repo_id) {
            return Ok(());
        }

        // Remove the repository from the map and delete the cloned repository and metadata.
        self.repositories.remove(repo_id);
        let clone_dir = self.download_directory.join(repo_id);
        fs::remove_dir_all(&clone_dir)
            .map_err(|_| RepositoryError::InvalidDownloadDirectory(clone_dir))?;
        let repo_metadata_path = self.metadata_directory.join(format!("{}.json", repo_id));
        fs::remove_file(&repo_metadata_path)
            .map_err(|_| RepositoryError::InvalidRepositoryMetadata(repo_metadata_path))?;
        Ok(())
    }

    fn update_repo(&self, repo_id: &str) -> Result<()> {
        let repo_metadata = self.repositories.get(repo_id);
        if repo_metadata.is_none() {
            bail!(RepositoryError::UnknownRepository(repo_id.to_string()));
        }

        // Re-clone the repository to make the logic easier. Otherwise, it would be harder to handle
        // corner cases. Users should not directly modify the cloned repositories.
        let repo_metadata = repo_metadata.unwrap();
        self.clone_repo(&repo_metadata.url, &repo_metadata.id)
    }

    fn update_all_repos(&self) -> Result<()> {
        for repo_id in self.repositories.keys() {
            self.update_repo(repo_id)?;
        }
        Ok(())
    }

    fn list_repos(&self) -> Result<Vec<RepositoryMetadata>> {
        Ok(self.repositories.values().cloned().collect())
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;

    use super::*;

    const REPO_URL: &str = "https://github.com/trane-project/trane-leetcode.git";
    const REPO_ID: &str = "trane-leetcode";

    fn setup_directories(library_root: &Path) -> Result<()> {
        let metadata_dir = library_root
            .join(TRANE_CONFIG_DIR_PATH)
            .join(REPOSITORY_DIRECTORY);
        fs::create_dir_all(&metadata_dir)?;
        let download_dir = library_root.join(DOWNLOAD_DIRECTORY);
        fs::create_dir_all(&download_dir)?;
        Ok(())
    }

    /// Verifies opening a repository manager with empty directories.
    #[test]
    fn new_empty() -> Result<()> {
        let library_root = tempfile::tempdir()?;
        setup_directories(library_root.path())?;
        let _ = LocalRepositoryManager::new(library_root.path())?;
        Ok(())
    }

    /// Verifies adding a repository.
    #[test]
    fn add() -> Result<()> {
        let library_root = tempfile::tempdir()?;
        setup_directories(library_root.path())?;
        let mut manager = LocalRepositoryManager::new(library_root.path())?;
        manager.add_repo(REPO_URL, None)?;
        assert!(manager.repositories.contains_key(REPO_ID));
        let repo_dir = library_root.path().join(DOWNLOAD_DIRECTORY).join(REPO_ID);
        assert!(repo_dir.exists());
        assert!(library_root
            .path()
            .join(TRANE_CONFIG_DIR_PATH)
            .join(REPOSITORY_DIRECTORY)
            .join(format!("{}.json", REPO_ID))
            .exists());
        Ok(())
    }

    /// Verifies adding a repository with a custom ID.
    #[test]
    fn add_with_id() -> Result<()> {
        let library_root = tempfile::tempdir()?;
        setup_directories(library_root.path())?;
        let mut manager = LocalRepositoryManager::new(library_root.path())?;
        manager.add_repo(REPO_URL, Some("custom-id".to_string()))?;
        assert!(manager.repositories.contains_key("custom-id"));
        assert!(library_root
            .path()
            .join(DOWNLOAD_DIRECTORY)
            .join("custom-id")
            .exists());
        assert!(library_root
            .path()
            .join(TRANE_CONFIG_DIR_PATH)
            .join(REPOSITORY_DIRECTORY)
            .join("custom-id.json")
            .exists());
        Ok(())
    }

    /// Verifies adding a repository with an SSH URL.
    #[test]
    fn add_ssh_repo() -> Result<()> {
        let library_root = tempfile::tempdir()?;
        setup_directories(library_root.path())?;
        let mut manager = LocalRepositoryManager::new(library_root.path())?;
        assert!(manager
            .add_repo("git@github.com:trane-project/trane-leetcode.git", None)
            .is_err());
        Ok(())
    }

    #[test]
    fn add_duplicate() -> Result<()> {
        let library_root = tempfile::tempdir()?;
        setup_directories(library_root.path())?;
        let mut manager = LocalRepositoryManager::new(library_root.path())?;
        manager.add_repo(REPO_URL, None)?;
        assert!(manager.add_repo(REPO_URL, None).is_err());
        Ok(())
    }

    /// Verifies removing a repository.
    #[test]
    fn remove() -> Result<()> {
        let library_root = tempfile::tempdir()?;
        setup_directories(library_root.path())?;
        let mut manager = LocalRepositoryManager::new(library_root.path())?;
        manager.add_repo(REPO_URL, None)?;
        manager.remove_repo(REPO_ID)?;
        assert!(!manager.repositories.contains_key(REPO_ID));
        assert!(!library_root
            .path()
            .join(DOWNLOAD_DIRECTORY)
            .join(REPO_ID)
            .exists());
        assert!(!library_root
            .path()
            .join(TRANE_CONFIG_DIR_PATH)
            .join(REPOSITORY_DIRECTORY)
            .join(format!("{}.json", REPO_ID))
            .exists());
        Ok(())
    }

    /// Verifies removing a repository that does not exist.
    #[test]
    fn remove_nonexistent() -> Result<()> {
        let library_root = tempfile::tempdir()?;
        setup_directories(library_root.path())?;
        let mut manager = LocalRepositoryManager::new(library_root.path())?;
        manager.remove_repo(REPO_ID)?;
        Ok(())
    }

    /// Verifies updating a repository.
    #[test]
    fn update() -> Result<()> {
        let library_root = tempfile::tempdir()?;
        setup_directories(library_root.path())?;
        let mut manager = LocalRepositoryManager::new(library_root.path())?;
        manager.add_repo(REPO_URL, None)?;
        manager.update_repo(REPO_ID)?;
        Ok(())
    }

    /// Verifies updating a repository that does not exist.
    #[test]
    fn update_nonexistent() -> Result<()> {
        let library_root = tempfile::tempdir()?;
        setup_directories(library_root.path())?;
        let manager = LocalRepositoryManager::new(library_root.path())?;
        assert!(manager.update_repo(REPO_ID).is_err());
        Ok(())
    }

    /// Verifies updating all repositories.
    #[test]
    fn update_all() -> Result<()> {
        let library_root = tempfile::tempdir()?;
        setup_directories(library_root.path())?;
        let mut manager = LocalRepositoryManager::new(library_root.path())?;
        manager.add_repo(REPO_URL, None)?;
        manager.update_all_repos()?;
        Ok(())
    }

    /// Verifies listing all repositories.
    #[test]
    fn list() -> Result<()> {
        let library_root = tempfile::tempdir()?;
        setup_directories(library_root.path())?;
        let mut manager = LocalRepositoryManager::new(library_root.path())?;
        manager.add_repo(REPO_URL, None)?;
        let repos = manager.list_repos()?;
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].id, REPO_ID);
        assert_eq!(repos[0].url, REPO_URL);
        Ok(())
    }

    /// Verifies opening an existing repository manager.
    #[test]
    fn new_existing() -> Result<()> {
        let library_root = tempfile::tempdir()?;
        setup_directories(library_root.path())?;
        let mut manager = LocalRepositoryManager::new(library_root.path())?;
        manager.add_repo(REPO_URL, None)?;
        let _ = LocalRepositoryManager::new(library_root.path())?;
        Ok(())
    }

    /// Verifies opening a repository manager with a missing repo.
    #[test]
    fn new_missing_repo() -> Result<()> {
        let library_root = tempfile::tempdir()?;
        setup_directories(library_root.path())?;
        let mut manager = LocalRepositoryManager::new(library_root.path())?;
        manager.add_repo(REPO_URL, None)?;
        let repo_dir = library_root.path().join(DOWNLOAD_DIRECTORY).join(REPO_ID);
        fs::remove_dir_all(&repo_dir)?;
        assert!(LocalRepositoryManager::new(library_root.path()).is_err());
        Ok(())
    }

    /// Verifies opening a repository manager with a bad repo.
    #[test]
    fn new_bad_repo() -> Result<()> {
        let library_root = tempfile::tempdir()?;
        setup_directories(library_root.path())?;
        let mut manager = LocalRepositoryManager::new(library_root.path())?;
        manager.add_repo(REPO_URL, None)?;
        let git_dir = library_root
            .path()
            .join(DOWNLOAD_DIRECTORY)
            .join(REPO_ID)
            .join(".git");
        fs::remove_dir_all(&git_dir)?;
        assert!(LocalRepositoryManager::new(library_root.path()).is_err());
        Ok(())
    }
}