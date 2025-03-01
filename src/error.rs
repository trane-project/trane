//! Contains the errors returned by Trane.

use thiserror::Error;
use ustr::Ustr;

use crate::data::UnitType;

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
pub enum PracticeRewardsError {
    #[error("cannot get rewards for unit {0}: {1}")]
    GetRewards(Ustr, #[source] anyhow::Error),

    #[error("cannot record reward for unit {0}: {1}")]
    RecordReward(Ustr, #[source] anyhow::Error),

    #[error("cannot trim rewards: {0}")]
    TrimReward(#[source] anyhow::Error),

    #[error("cannot remove rewards from units matching prefix {0}: {1}")]
    RemovePrefix(String, #[source] anyhow::Error),
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

    #[error("cannot remove scores from units matching prefix {0}: {1}")]
    RemovePrefix(String, #[source] anyhow::Error),
}

/// An error returned when dealing with user preferences.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum PreferencesManagerError {
    #[error("cannot get user preferences: {0}")]
    GetUserPreferences(#[source] anyhow::Error),

    #[error("cannot set user preferences: {0}")]
    SetUserPreferences(#[source] anyhow::Error),
}

/// An error returned when dealing with git repositories containing courses.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum RepositoryManagerError {
    #[error("cannot add repository with URL {0}: {1}")]
    AddRepo(String, #[source] anyhow::Error),

    #[error("cannot list repositories: {0}")]
    ListRepos(#[source] anyhow::Error),

    #[error("cannot get repository with ID {0}: {1}")]
    RemoveRepo(String, #[source] anyhow::Error),

    #[error("cannot update repository with ID {0}: {1}")]
    UpdateRepo(String, #[source] anyhow::Error),

    #[error("cannot update repositories: {0}")]
    UpdateRepos(#[source] anyhow::Error),
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

/// An error returned when downloading transcription assets.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum TranscriptionDownloaderError {
    #[error("cannot download asset for exercise {0}: {1}")]
    DownloadAsset(Ustr, #[source] anyhow::Error),
}

/// An error returned when dealing with the unit graph.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum UnitGraphError {
    #[error("cannot add dependencies for unit {0} of type {1} to the unit graph: {2}")]
    AddDependencies(Ustr, UnitType, #[source] anyhow::Error),

    #[error("cannot add unit {0} of type {1} to the unit graph: {2}")]
    AddUnit(Ustr, UnitType, #[source] anyhow::Error),

    #[error("checking for cycles in the unit graph failed: {0}")]
    CheckCycles(#[source] anyhow::Error),
}
