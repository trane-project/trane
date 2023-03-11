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

/// The prefix for SSH URLs. Only HTTP URLs are supported at the moment because SSH URLs require
/// Trane to have access to the user's SSH keys.
const SSH_PREFIX: &str = "ssh://";

/// A trait with function to manage git repositories of courses, with functions to add new
/// repositories, remove existing ones, and update repositories to the latest version.
pub trait RepositoryManager {
    /// Downloads the courses from the given git repository into the given directory. The ID will
    /// also be used to identify the repository in the future and as the name of the directory. If
    /// ommitted, the name of the repository will be used to generate an ID.
    fn add(&mut self, url: &str, id: Option<&str>) -> Result<()>;

    /// Removes the repository with the given ID.
    fn remove(&mut self, id: &str) -> Result<()>;

    /// Attempts to pull the latest version of the given repository.
    fn update(&self, id: &str) -> Result<()>;

    /// Attempts to pull the latest version of all repositories.
    fn update_all(&self) -> Result<()>;
}

/// An implementation of [RepositoryManager] backed by the local file system. All repositories will
/// be downloaded to the `managed_courses` directory in the root of the Trane library.
struct LocalRepositoryManager {
    /// A map of repository IDs to the path of the repository.
    repositories: HashMap<String, PathBuf>,

    /// The path to the directory where repositories will be downloaded.
    download_directory: PathBuf,

    /// The path to the directory where repository metadata will be stored.
    repository_directory: PathBuf,
}

impl LocalRepositoryManager {
    /// Reads the repository metadata from the given path.
    fn read_managed_repo(path: &Path) -> Result<RepositoryMetadata> {
        let repo = serde_json::from_str::<RepositoryMetadata>(&fs::read_to_string(path)?)
            .map_err(|_| RepositoryError::InvalidRepository(path.to_owned()))?;
        Ok(repo)
    }

    /// Clones the repository at the given URL into the given directory. If the directory already
    /// exists, it will be deleted and replaced with the new repository.
    fn clone_repo(url: &str, clone_dir: &Path) -> Result<()> {
        // The path must be a directory.
        if clone_dir.exists() && !clone_dir.is_dir() {
            bail!(RepositoryError::InvalidDownloadDirectory(
                clone_dir.to_owned()
            ));
        }

        // Clone the repo into a temp directory and then move it to the download directory.
        let temp_dir = tempfile::tempdir()?;
        git2::Repository::clone(url, &temp_dir.path())
            .map_err(|_| RepositoryError::CloneRepository(url.to_string()))?;
        fs::remove_dir_all(clone_dir)
            .map_err(|_| RepositoryError::InvalidDownloadDirectory(clone_dir.to_owned()))?;
        fs_extra::move_items(
            &[temp_dir.path().to_str().unwrap()],
            clone_dir,
            &fs_extra::dir::CopyOptions::new(),
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
            fs::create_dir(&repo_dir).map_err(|_| RepositoryError::InvalidRepositoryDirectory)?;
        }
        let mut manager = LocalRepositoryManager {
            repositories: HashMap::new(),
            download_directory: library_root.join(DOWNLOAD_DIRECTORY),
            repository_directory: repo_dir.clone(),
        };

        // Read the repository directory and add all the repositories to the map.
        let read_repo_dir =
            fs::read_dir(&repo_dir).map_err(|_| RepositoryError::InvalidRepositoryDirectory)?;
        for entry in read_repo_dir {
            // Ignore any directories, invalid files, or files that are not JSON.
            if entry.is_err() {
                continue;
            }
            let entry = entry.unwrap();
            if entry.path().is_dir() {
                continue;
            }
            if entry.path().extension().unwrap_or_default() != "json" {
                continue;
            }

            // Read the repository metadata and add it to the map.
            let managed_repo =
                serde_json::from_str::<RepositoryMetadata>(&fs::read_to_string(entry.path())?)
                    .map_err(|_| RepositoryError::InvalidRepository(entry.path()))?;

            let download_directory = library_root.join(DOWNLOAD_DIRECTORY).join(&managed_repo.id);
            manager
                .repositories
                .insert(managed_repo.id, download_directory.clone());

            // Verify that the repository exists and is a valid git repository.
            if !download_directory.exists() {
                bail!(RepositoryError::InvalidDownloadDirectory(
                    download_directory
                ));
            }
            if git2::Repository::open(&download_directory).is_err() {
                bail!(RepositoryError::InvalidDownloadDirectory(
                    download_directory.clone()
                ));
            }
        }

        Ok(manager)
    }
}

impl RepositoryManager for LocalRepositoryManager {
    fn add(&mut self, url: &str, repo_id: Option<&str>) -> Result<()> {
        // Check that the repository URL is not an SSH URL.
        if url.starts_with(SSH_PREFIX) {
            bail!(RepositoryError::SshRepository(url.to_string()));
        }

        // Extract the repository ID from the URL if it wasn't provided.
        let parsed_url = url
            .parse::<Url>()
            .map_err(|_| RepositoryError::InvalidRepositoryURL(url.to_string()))?;
        let valid_repo_id = if let Some(repo_id) = repo_id {
            repo_id
        } else {
            parsed_url
                .path_segments()
                .and_then(|segments| segments.last())
                .ok_or_else(|| RepositoryError::InvalidRepositoryURL(url.to_string()))?
        };

        // Check that no other repository has the same ID.
        if self.repositories.contains_key(valid_repo_id) {
            bail!(RepositoryError::DuplicateRepository(
                valid_repo_id.to_string()
            ));
        }

        // Clone the repository into the download directory.
        // add it to the map, and save the repository information to the repository directory.
        let repo_download_dir = self.download_directory.join(valid_repo_id);
        git2::Repository::clone(url, &repo_download_dir)
            .map_err(|_| RepositoryError::CloneRepository(url.to_string()))?;
        self.repositories
            .insert(valid_repo_id.to_string(), repo_download_dir);
        let managed_repo = RepositoryMetadata {
            id: valid_repo_id.to_string(),
            url: url.to_string(),
        };
        let repo_dir = self
            .repository_directory
            .join(format!("{}.json", valid_repo_id));
        fs::write(&repo_dir, serde_json::to_string_pretty(&managed_repo)?)
            .map_err(|_| RepositoryError::InvalidRepositoryDirectory)?;

        Ok(())
    }

    fn remove(&mut self, repo_id: &str) -> Result<()> {
        // Do nothing if no repository with the given ID exists.
        if !self.repositories.contains_key(repo_id) {
            return Ok(());
        }

        // Remove the repository from the map and delete the directory.
        self.repositories.remove(repo_id);
        fs::remove_dir_all(PathBuf::from(DOWNLOAD_DIRECTORY).join(repo_id))?;
        todo!()
    }

    fn update(&self, repo_id: &str) -> Result<()> {
        // Check that the repository exists and is a valid git repository.
        let repo_path = self
            .repositories
            .get(repo_id)
            .ok_or_else(|| RepositoryError::UnknownRepository(repo_id.to_string()))?;
        let repo = git2::Repository::open(repo_path)
            .map_err(|_| RepositoryError::InvalidRepository(repo_path.clone()))?;

        Ok(())
    }

    fn update_all(&self) -> Result<()> {
        todo!()
    }
}
