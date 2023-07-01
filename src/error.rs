//! Contains the errors returned by Trane.

use std::{io, path::PathBuf};

use thiserror::Error;
use ustr::Ustr;

/// An error returned when dealing with the blacklist.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum BlacklistError {
    #[error("cannot add unit {0} to the blacklist: {1}")]
    AddEntry(Ustr, #[source] rusqlite::Error),

    #[error("cannot retrieve connection from pool: {0}")]
    Connection(#[source] r2d2::Error),

    #[error("the migrations for the blacklist DB cannot be applied: {0}")]
    Migration(#[source] rusqlite_migration::Error),

    #[error("the sql statement cannot be prepared: {0}")]
    PrepareSqlStatement(#[source] rusqlite::Error),

    #[error("cannot query entries from the blacklist: {0}")]
    Query(#[source] rusqlite::Error),

    #[error("cannot remove unit {0} from the blacklist: {1}")]
    RemoveEntry(Ustr, #[source] rusqlite::Error),
}

/// An error returned when dealing with the course library.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum CourseLibraryError {
    #[error("cannot query the course library: {0}")]
    QueryError(#[from] tantivy::error::TantivyError),

    #[error("cannot retrieve schema for field {0}: {1}")]
    SchemaFieldError(String, #[source] tantivy::error::TantivyError),
}

/// An error returned when dealing with git repositories contianing courses.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum RepositoryError {
    #[error("failed to clone repository with URL {0}: {1}")]
    CloneRepository(String, #[source] git2::Error),

    #[error("the repository cannot be copied to {0}: {1}")]
    CopyRepository(PathBuf, #[source] fs_extra::error::Error),

    #[error("another repository with ID {0} already exists")]
    DuplicateRepository(String),

    #[error("the download directory {0} is invalid or cannot be accessed: {1}")]
    InvalidDownloadDirectory(PathBuf, #[source] std::io::Error),

    #[error("failed to create repository metadata directory .trane/repositories: {0}")]
    InvalidMetadataDirectory(#[source] std::io::Error),

    #[error("the repository metadata file at {0} cannot be accessed: {1}")]
    InvalidMetadataFile(PathBuf, #[source] std::io::Error),

    #[error("the repository at {0} is invalid: {1}")]
    InvalidRepository(PathBuf, #[source] git2::Error),

    #[error("the repository metadata at {0} is invalid: {1}")]
    InvalidRepositoryMetadata(PathBuf, #[source] serde_json::Error),

    #[error("repository with URL {0} has an invalid URL")]
    InvalidRepositoryURL(String),

    #[error("error creating temp directory: {0}")]
    TempDir(#[source] io::Error),

    #[error("cannot find repository with ID {0}")]
    UnknownRepository(String),
}
