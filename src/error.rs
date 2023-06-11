//! Contains the errors returned by Trane.

use std::path::PathBuf;

use thiserror::Error;

/// An error returned when dealing with git repositories contianing courses.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum RepositoryError {
    #[error("failed to clone repository with URL {0}: {1}")]
    CloneRepository(String, #[source] git2::Error),

    #[error("failed to create repository metadata directory .trane/repositories: {0}")]
    InvalidMetadataDirectory(#[source] std::io::Error),

    #[error("another repository with ID {0} already exists")]
    DuplicateRepository(String),

    #[error("the download directory {0} is invalid or cannot be accessed: {1}")]
    InvalidDownloadDirectory(PathBuf, #[source] std::io::Error),

    #[error("the repository cannot be copied to {0}: {1}")]
    CopyRepository(PathBuf, #[source] fs_extra::error::Error),

    #[error("the repository at {0} is invalid: {1}")]
    InvalidRepository(PathBuf, #[source] git2::Error),

    #[error("the repository metadata file at {0} cannot be accessed: {1}")]
    InvalidMetadataFile(PathBuf, #[source] std::io::Error),

    #[error("the repository metadata at {0} is invalid: {1}")]
    InvalidRepositoryMetadata(PathBuf, #[source] serde_json::Error),

    #[error("repository with URL {0} has an invalid URL")]
    InvalidRepositoryURL(String),

    #[error("cannot find repository with ID {0}")]
    UnknownRepository(String),
}
