//! Contains the FFI types used to interact with Trane from other languages.
//!
//! The FFI types are generated from the Rust types using the `typeshare` library. Since `typeshare`
//! requires that enums are serialized differently than the existing serialization format, all the
//! types are replicated in this module. Bidirectional implementations of the `From` trait are
//! provided to convert between the Rust types and the FFI types and ensure that the types are
//! equivalent at compile time.
//! Some considerations when translating between the native and FFI types:
//! - Serialize `Ustr` values as `String`.
//! - Serialize `BTreeMap` values as `HashMap`.
//! - Serialize dates, given as either a timestamp or a `DateTime`, as an RFC 3339 string. This is
//!   because `typeshare` does not support serializing `chrono` types not 64-bit integers.

pub mod course_generator;
pub mod filter;

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use typeshare::typeshare;
use ustr::Ustr;

use crate::data;
use crate::data::ffi::course_generator::*;

// grcov-excl-start: The FFI types are not tested since the implementations of the `From` trait
// should be sufficient to ensure that the types are equivalent at compile time.

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum MasteryScore {
    One,
    Two,
    Three,
    Four,
    Five,
}

impl From<MasteryScore> for data::MasteryScore {
    fn from(mastery_score: MasteryScore) -> Self {
        match mastery_score {
            MasteryScore::One => data::MasteryScore::One,
            MasteryScore::Two => data::MasteryScore::Two,
            MasteryScore::Three => data::MasteryScore::Three,
            MasteryScore::Four => data::MasteryScore::Four,
            MasteryScore::Five => data::MasteryScore::Five,
        }
    }
}

impl From<data::MasteryScore> for MasteryScore {
    fn from(mastery_score: data::MasteryScore) -> Self {
        match mastery_score {
            data::MasteryScore::One => MasteryScore::One,
            data::MasteryScore::Two => MasteryScore::Two,
            data::MasteryScore::Three => MasteryScore::Three,
            data::MasteryScore::Four => MasteryScore::Four,
            data::MasteryScore::Five => MasteryScore::Five,
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ExerciseTrial {
    pub score: f32,
    pub timestamp: String,
}

impl From<ExerciseTrial> for data::ExerciseTrial {
    fn from(exercise_trial: ExerciseTrial) -> Self {
        Self {
            score: exercise_trial.score,
            timestamp: DateTime::parse_from_rfc3339(&exercise_trial.timestamp)
                .unwrap_or_else(|_| Utc::now().fixed_offset())
                .with_timezone(&Utc)
                .timestamp(),
        }
    }
}

impl From<data::ExerciseTrial> for ExerciseTrial {
    fn from(exercise_trial: data::ExerciseTrial) -> Self {
        Self {
            score: exercise_trial.score,
            timestamp: Utc
                .timestamp_opt(exercise_trial.timestamp, 0)
                .earliest()
                .unwrap_or_default()
                .to_rfc3339(),
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum UnitType {
    Exercise,
    Lesson,
    Course,
}

impl From<UnitType> for data::UnitType {
    fn from(unit_type: UnitType) -> Self {
        match unit_type {
            UnitType::Exercise => data::UnitType::Exercise,
            UnitType::Lesson => data::UnitType::Lesson,
            UnitType::Course => data::UnitType::Course,
        }
    }
}

impl From<data::UnitType> for UnitType {
    fn from(unit_type: data::UnitType) -> Self {
        match unit_type {
            data::UnitType::Exercise => UnitType::Exercise,
            data::UnitType::Lesson => UnitType::Lesson,
            data::UnitType::Course => UnitType::Course,
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "content")]
pub enum BasicAsset {
    MarkdownAsset {
        path: String,
    },
    InlinedAsset {
        content: String,
    },
    InlinedUniqueAsset {
        #[typeshare(serialized_as = "String")]
        content: Ustr,
    },
}

impl From<BasicAsset> for data::BasicAsset {
    fn from(basic_asset: BasicAsset) -> Self {
        match basic_asset {
            BasicAsset::MarkdownAsset { path } => Self::MarkdownAsset { path },
            BasicAsset::InlinedAsset { content } => Self::InlinedAsset { content },
            BasicAsset::InlinedUniqueAsset { content } => Self::InlinedUniqueAsset { content },
        }
    }
}

impl From<data::BasicAsset> for BasicAsset {
    fn from(basic_asset: data::BasicAsset) -> Self {
        match basic_asset {
            data::BasicAsset::MarkdownAsset { path } => Self::MarkdownAsset { path },
            data::BasicAsset::InlinedAsset { content } => Self::InlinedAsset { content },
            data::BasicAsset::InlinedUniqueAsset { content } => {
                Self::InlinedUniqueAsset { content }
            }
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "content")]
pub enum CourseGenerator {
    KnowledgeBase(KnowledgeBaseConfig),
    MusicPiece(MusicPieceConfig),
    Transcription(TranscriptionConfig),
}

impl From<CourseGenerator> for data::CourseGenerator {
    fn from(generator: CourseGenerator) -> Self {
        match generator {
            CourseGenerator::KnowledgeBase(config) => Self::KnowledgeBase(config.into()),
            CourseGenerator::MusicPiece(config) => Self::MusicPiece(config.into()),
            CourseGenerator::Transcription(config) => Self::Transcription(config.into()),
        }
    }
}

impl From<data::CourseGenerator> for CourseGenerator {
    fn from(generator: data::CourseGenerator) -> Self {
        match generator {
            data::CourseGenerator::KnowledgeBase(config) => Self::KnowledgeBase(config.into()),
            data::CourseGenerator::MusicPiece(config) => Self::MusicPiece(config.into()),
            data::CourseGenerator::Transcription(config) => Self::Transcription(config.into()),
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CourseManifest {
    #[typeshare(serialized_as = "String")]
    pub id: Ustr,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    #[typeshare(serialized_as = "Vec<String>")]
    pub dependencies: Vec<Ustr>,
    #[serde(default)]
    #[typeshare(serialized_as = "Vec<String>")]
    pub superseded: Vec<Ustr>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub authors: Option<Vec<String>>,
    #[serde(default)]
    #[typeshare(serialized_as = "Option<HashMap<String, Vec<String>>>")]
    pub metadata: Option<BTreeMap<String, Vec<String>>>,
    #[serde(default)]
    pub course_material: Option<BasicAsset>,
    #[serde(default)]
    pub course_instructions: Option<BasicAsset>,
    #[serde(default)]
    pub generator_config: Option<CourseGenerator>,
}

impl From<CourseManifest> for data::CourseManifest {
    fn from(manifest: CourseManifest) -> Self {
        Self {
            id: manifest.id,
            name: manifest.name,
            dependencies: manifest.dependencies,
            superseded: manifest.superseded,
            description: manifest.description,
            authors: manifest.authors,
            metadata: manifest.metadata,
            course_material: manifest.course_material.map(Into::into),
            course_instructions: manifest.course_instructions.map(Into::into),
            generator_config: manifest.generator_config.map(Into::into),
        }
    }
}

impl From<data::CourseManifest> for CourseManifest {
    fn from(manifest: data::CourseManifest) -> Self {
        Self {
            id: manifest.id,
            name: manifest.name,
            dependencies: manifest.dependencies,
            superseded: manifest.superseded,
            description: manifest.description,
            authors: manifest.authors,
            metadata: manifest.metadata,
            course_material: manifest.course_material.map(Into::into),
            course_instructions: manifest.course_instructions.map(Into::into),
            generator_config: manifest.generator_config.map(Into::into),
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LessonManifest {
    #[typeshare(serialized_as = "String")]
    pub id: Ustr,
    #[serde(default)]
    #[typeshare(serialized_as = "Vec<String>")]
    pub dependencies: Vec<Ustr>,
    #[serde(default)]
    #[typeshare(serialized_as = "Vec<String>")]
    pub superseded: Vec<Ustr>,
    #[typeshare(serialized_as = "String")]
    pub course_id: Ustr,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    #[typeshare(serialized_as = "Option<HashMap<String, Vec<String>>>")]
    pub metadata: Option<BTreeMap<String, Vec<String>>>,
    #[serde(default)]
    pub lesson_material: Option<BasicAsset>,
    #[serde(default)]
    pub lesson_instructions: Option<BasicAsset>,
}

impl From<LessonManifest> for data::LessonManifest {
    fn from(manifest: LessonManifest) -> Self {
        Self {
            id: manifest.id,
            dependencies: manifest.dependencies,
            superseded: manifest.superseded,
            course_id: manifest.course_id,
            name: manifest.name,
            description: manifest.description,
            metadata: manifest.metadata,
            lesson_material: manifest.lesson_material.map(Into::into),
            lesson_instructions: manifest.lesson_instructions.map(Into::into),
        }
    }
}

impl From<data::LessonManifest> for LessonManifest {
    fn from(manifest: data::LessonManifest) -> Self {
        Self {
            id: manifest.id,
            dependencies: manifest.dependencies,
            superseded: manifest.superseded,
            course_id: manifest.course_id,
            name: manifest.name,
            description: manifest.description,
            metadata: manifest.metadata,
            lesson_material: manifest.lesson_material.map(Into::into),
            lesson_instructions: manifest.lesson_instructions.map(Into::into),
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub enum ExerciseType {
    Declarative,
    #[default]
    Procedural,
}

impl From<ExerciseType> for data::ExerciseType {
    fn from(exercise_type: ExerciseType) -> Self {
        match exercise_type {
            ExerciseType::Declarative => Self::Declarative,
            ExerciseType::Procedural => Self::Procedural,
        }
    }
}

impl From<data::ExerciseType> for ExerciseType {
    fn from(exercise_type: data::ExerciseType) -> Self {
        match exercise_type {
            data::ExerciseType::Declarative => Self::Declarative,
            data::ExerciseType::Procedural => Self::Procedural,
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "content")]
pub enum ExerciseAsset {
    SoundSliceAsset {
        link: String,
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        backup: Option<String>,
    },
    FlashcardAsset {
        front_path: String,
        #[serde(default)]
        back_path: Option<String>,
    },
    BasicAsset(BasicAsset),
}

impl From<ExerciseAsset> for data::ExerciseAsset {
    fn from(asset: ExerciseAsset) -> Self {
        match asset {
            ExerciseAsset::SoundSliceAsset {
                link,
                description,
                backup,
            } => Self::SoundSliceAsset {
                link,
                description,
                backup,
            },
            ExerciseAsset::FlashcardAsset {
                front_path,
                back_path,
            } => Self::FlashcardAsset {
                front_path,
                back_path,
            },
            ExerciseAsset::BasicAsset(asset) => Self::BasicAsset(asset.into()),
        }
    }
}

impl From<data::ExerciseAsset> for ExerciseAsset {
    fn from(asset: data::ExerciseAsset) -> Self {
        match asset {
            data::ExerciseAsset::SoundSliceAsset {
                link,
                description,
                backup,
            } => Self::SoundSliceAsset {
                link,
                description,
                backup,
            },
            data::ExerciseAsset::FlashcardAsset {
                front_path,
                back_path,
            } => Self::FlashcardAsset {
                front_path,
                back_path,
            },
            data::ExerciseAsset::BasicAsset(asset) => Self::BasicAsset(asset.into()),
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ExerciseManifest {
    #[typeshare(serialized_as = "String")]
    pub id: Ustr,
    #[typeshare(serialized_as = "String")]
    pub lesson_id: Ustr,
    #[typeshare(serialized_as = "String")]
    pub course_id: Ustr,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub exercise_type: ExerciseType,
    pub exercise_asset: ExerciseAsset,
}

impl From<ExerciseManifest> for data::ExerciseManifest {
    fn from(manifest: ExerciseManifest) -> Self {
        Self {
            id: manifest.id,
            lesson_id: manifest.lesson_id,
            course_id: manifest.course_id,
            name: manifest.name,
            description: manifest.description,
            exercise_type: manifest.exercise_type.into(),
            exercise_asset: manifest.exercise_asset.into(),
        }
    }
}

impl From<data::ExerciseManifest> for ExerciseManifest {
    fn from(manifest: data::ExerciseManifest) -> Self {
        Self {
            id: manifest.id,
            lesson_id: manifest.lesson_id,
            course_id: manifest.course_id,
            name: manifest.name,
            description: manifest.description,
            exercise_type: manifest.exercise_type.into(),
            exercise_asset: manifest.exercise_asset.into(),
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "content")]
pub enum PassingScoreOptions {
    ConstantScore(f32),
    IncreasingScore {
        starting_score: f32,
        step_size: f32,
        #[typeshare(serialized_as = "u32")]
        max_steps: usize,
    },
}

impl From<PassingScoreOptions> for data::PassingScoreOptions {
    fn from(options: PassingScoreOptions) -> Self {
        match options {
            PassingScoreOptions::ConstantScore(score) => Self::ConstantScore(score),
            PassingScoreOptions::IncreasingScore {
                starting_score,
                step_size,
                max_steps,
            } => Self::IncreasingScore {
                starting_score,
                step_size,
                max_steps,
            },
        }
    }
}

impl From<data::PassingScoreOptions> for PassingScoreOptions {
    fn from(options: data::PassingScoreOptions) -> Self {
        match options {
            data::PassingScoreOptions::ConstantScore(score) => Self::ConstantScore(score),
            data::PassingScoreOptions::IncreasingScore {
                starting_score,
                step_size,
                max_steps,
            } => Self::IncreasingScore {
                starting_score,
                step_size,
                max_steps,
            },
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MasteryWindow {
    pub percentage: f32,
    #[typeshare(serialized_as = "Vec<f32>")]
    pub range: (f32, f32),
}

impl From<MasteryWindow> for data::MasteryWindow {
    fn from(window: MasteryWindow) -> Self {
        Self {
            percentage: window.percentage,
            range: window.range,
        }
    }
}

impl From<data::MasteryWindow> for MasteryWindow {
    fn from(window: data::MasteryWindow) -> Self {
        Self {
            percentage: window.percentage,
            range: window.range,
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SchedulerOptions {
    #[typeshare(serialized_as = "u32")]
    pub batch_size: usize,
    pub new_window_opts: MasteryWindow,
    pub target_window_opts: MasteryWindow,
    pub current_window_opts: MasteryWindow,
    pub easy_window_opts: MasteryWindow,
    pub mastered_window_opts: MasteryWindow,
    pub passing_score: PassingScoreOptions,
    pub superseding_score: f32,
    #[typeshare(serialized_as = "u32")]
    pub num_trials: usize,
}

impl From<SchedulerOptions> for data::SchedulerOptions {
    fn from(options: SchedulerOptions) -> Self {
        Self {
            batch_size: options.batch_size,
            new_window_opts: options.new_window_opts.into(),
            target_window_opts: options.target_window_opts.into(),
            current_window_opts: options.current_window_opts.into(),
            easy_window_opts: options.easy_window_opts.into(),
            mastered_window_opts: options.mastered_window_opts.into(),
            passing_score: options.passing_score.into(),
            superseding_score: options.superseding_score,
            num_trials: options.num_trials,
        }
    }
}

impl From<data::SchedulerOptions> for SchedulerOptions {
    fn from(options: data::SchedulerOptions) -> Self {
        Self {
            batch_size: options.batch_size,
            new_window_opts: options.new_window_opts.into(),
            target_window_opts: options.target_window_opts.into(),
            current_window_opts: options.current_window_opts.into(),
            easy_window_opts: options.easy_window_opts.into(),
            mastered_window_opts: options.mastered_window_opts.into(),
            passing_score: options.passing_score.into(),
            superseding_score: options.superseding_score,
            num_trials: options.num_trials,
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct SchedulerPreferences {
    #[serde(default)]
    #[typeshare(serialized_as = "Option<u32>")]
    pub batch_size: Option<usize>,
}

impl From<SchedulerPreferences> for data::SchedulerPreferences {
    fn from(preferences: SchedulerPreferences) -> Self {
        Self {
            batch_size: preferences.batch_size,
        }
    }
}

impl From<data::SchedulerPreferences> for SchedulerPreferences {
    fn from(preferences: data::SchedulerPreferences) -> Self {
        Self {
            batch_size: preferences.batch_size,
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RepositoryMetadata {
    pub id: String,
    pub url: String,
}

impl From<RepositoryMetadata> for data::RepositoryMetadata {
    fn from(metadata: RepositoryMetadata) -> Self {
        Self {
            id: metadata.id,
            url: metadata.url,
        }
    }
}

impl From<data::RepositoryMetadata> for RepositoryMetadata {
    fn from(metadata: data::RepositoryMetadata) -> Self {
        Self {
            id: metadata.id,
            url: metadata.url,
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct UserPreferences {
    #[serde(default)]
    pub transcription: Option<TranscriptionPreferences>,
    #[serde(default)]
    pub scheduler: Option<SchedulerPreferences>,
    #[serde(default)]
    pub ignored_paths: Vec<String>,
}

impl From<UserPreferences> for data::UserPreferences {
    fn from(preferences: UserPreferences) -> Self {
        Self {
            transcription: preferences.transcription.map(Into::into),
            scheduler: preferences.scheduler.map(Into::into),
            ignored_paths: preferences.ignored_paths,
        }
    }
}

impl From<data::UserPreferences> for UserPreferences {
    fn from(preferences: data::UserPreferences) -> Self {
        Self {
            transcription: preferences.transcription.map(Into::into),
            scheduler: preferences.scheduler.map(Into::into),
            ignored_paths: preferences.ignored_paths,
        }
    }
}
