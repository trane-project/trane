//! Contains the errors returned by Trane.

use std::path::PathBuf;

use thiserror::Error;

/// An error returned by Trane when dealing with a repository contianing courses.
#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("failed to clone repository with URL {0}")]
    CloneRepository(String),

    #[error("failed to create repository directory under .trane/repositories")]
    InvalidRepositoryDirectory,

    #[error("another repository with ID {0} already exists")]
    DuplicateRepository(String),

    #[error("the directory to which to download repositories is invalid or cannot be accessed")]
    InvalidDownloadDirectory(PathBuf),

    #[error("the repository at {0} is invalid")]
    InvalidRepository(PathBuf),

    #[error("the repository metadata at {0} is invalid")]
    InvalidRepositoryMetadata(PathBuf),

    #[error("repository with URL {0} has an invalid URL")]
    InvalidRepositoryURL(String),

    #[error("repository with SSH URL {0} is not supported, use an HTTPS URL instead")]
    SshRepository(String),

    #[error("cannot find repository with ID {0}")]
    UnknownRepository(String),
}
