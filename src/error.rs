//! Contains the errors returned by Trane.

use std::{io, path::PathBuf};

use thiserror::Error;
use ustr::Ustr;

/// Generic errors for Trane.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error("{0}")]
    Basic(String),

    #[error("{0}")]
    Error(#[from] anyhow::Error),

    #[error("{0}: {1}")]
    WithSource(String, #[source] anyhow::Error),
}

/// An error returned when dealing with the blacklist.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum BlacklistError {
    #[error("cannot add unit {0} to the blacklist: {1}")]
    AddUnit(Ustr, #[source] anyhow::Error),

    #[error("cannot get entries from the blacklist: {0}")]
    GetEntries(#[source] anyhow::Error),

    #[error("cannot remove entries with prefix {0} from the blacklist: {1}")]
    RemovePrefix(String, #[source] anyhow::Error),

    #[error("cannot remove unit {0} from the blacklist: {1}")]
    RemoveUnit(Ustr, #[source] anyhow::Error),
}

/// An error returned when dealing with the course library.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum CourseLibraryError {
    #[error("cannot process query {0}: {1}")]
    Search(String, #[source] anyhow::Error),
}

/// An error returned when dealing with the exercise scheduler.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum ExerciseSchedulerError {
    #[error("cannot retrieve exercise batch: {0}")]
    GetExerciseBatch(#[source] anyhow::Error),

    #[error("cannot score exercise: {0}")]
    ScoreExercise(#[source] anyhow::Error),
}

/// An error returned when dealing with the practice stats.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum PracticeStatsError {
    #[error("cannot get scores for unit {0}: {1}")]
    GetScores(Ustr, #[source] anyhow::Error),

    #[error("cannot record score for unit {0}: {1}")]
    RecordScore(Ustr, #[source] anyhow::Error),

    #[error("cannot trim scores: {0}")]
    TrimScores(#[source] anyhow::Error),
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

/// An error returned when dealing with the review list.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum ReviewListError {
    #[error("cannot add unit {0} to the review list: {1}")]
    AddUnit(Ustr, #[source] anyhow::Error),

    #[error("cannot retrieve the entries from the review list: {0}")]
    GetEntries(#[source] anyhow::Error),

    #[error("cannot remove unit {0} from the review list: {1}")]
    RemoveUnit(Ustr, #[source] anyhow::Error),
}
