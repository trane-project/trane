//! Defines the basic data structures used by Trane to describe courses, lessons, and exercises,
//! store the results of a student's attempt at mastering an exercise, the options avaialble to
//! control the behavior of the scheduler, among other things.

pub mod course_generator;
pub mod filter;
pub mod music;

use anyhow::{bail, Result};
use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
use ustr::Ustr;

use self::course_generator::{
    improvisation::{ImprovisationConfig, ImprovisationPreferences},
    knowledge_base::KnowledgeBaseConfig,
    music_piece::MusicPieceConfig,
    transcription::{TranscriptionConfig, TranscriptionPreferences},
};

/// The score used by students to evaluate their mastery of a particular exercise after a trial.
/// More detailed descriptions of the levels are provided using the example of an exercise that
/// requires the student to learn a musical passage.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum MasteryScore {
    /// One signifies the student has barely any mastery of the exercise. For a musical passage,
    /// this level of mastery represents the initial attempts at hearing and reading the music, and
    /// figuring out the movements required to perform it.
    One,

    /// Two signifies the student has achieved some mastery of the exercise. For a musical passage,
    /// this level of mastery represents the stage at which the student knows the music, the
    /// required movements, and can perform the passage slowly with some mistakes.
    Two,

    /// Three signifies the student has achieved significant mastery of the exercise. For a musical
    /// passage, this level of mastery represents the stage at which the student can perform the
    /// material slowly with barely any mistakes, and has begun to learn it at higher tempos.
    Three,

    /// Four signifies the student has gained mastery of the exercise, requiring almost not
    /// conscious thought to complete it. For a musical passage, this level of mastery represents
    /// the stage at which the student can perform the material at the desired tempo with all
    /// elements (rhythm, dynamics, etc.) completely integrated into the performance.
    Four,

    /// Five signifies the student has gained total mastery of the material and can apply it in
    /// novel situations and come up with new variations. For exercises that test declarative
    /// knowledge or that do not easily lend themselves for variations (e.g., a question on some
    /// programming language's feature), the difference between the fourth and fifth level is just a
    /// matter of increased speed and accuracy. For a musical passage, this level of mastery
    /// represents the stage at which the student can perform the material without making mistakes.
    /// In addition, they can also play their own variations of the material by modifying the
    /// melody, harmony, dynamics, rhythm, etc., and do so effortlessly.
    Five,
}

impl MasteryScore {
    /// Assigns a float value to each of the values of `MasteryScore`.
    pub fn float_score(&self) -> f32 {
        match *self {
            Self::One => 1.0,
            Self::Two => 2.0,
            Self::Three => 3.0,
            Self::Four => 4.0,
            Self::Five => 5.0,
        }
    }
}

//@<lp-example-4
/// The result of a single trial.
pub struct ExerciseTrial {
    /// The score assigned to the exercise after the trial.
    pub score: f32,

    /// The timestamp at which the trial happened.
    pub timestamp: i64,
}
//>@lp-example-4

/// A mastery window consists a range of scores and the percentage of the total exercises in the
/// batch returned by the scheduler that will fall within that range.
///
/// Mastery windows are used by the scheduler to control the amount of exercises for a given range
/// of difficulty given to the student to try to keep an optimal balance. For example, exercises
/// that are already fully mastered should not be shown very often lest the student becomes bored.
/// Very difficult exercises should not be shown too often either lest the student becomes
/// frustrated.
#[derive(Clone, Debug)]
pub struct MasteryWindow {
    /// The percentage of the exercises in each batch returned by the scheduler whose scores should
    /// fall within this window.
    pub percentage: f32,

    /// The range of scores which fall on this window. Scores whose values are in the range
    /// `[range.0, range.1)` fall within this window. If `range.1` is equal to 5.0 (the float
    /// representation of the maximum possible score), then the range becomes inclusive.
    pub range: (f32, f32),
}

impl MasteryWindow {
    /// Returns whether the given score falls within this window.
    pub fn in_window(&self, score: f32) -> bool {
        // Handle the special case of the window containing the maximum score.
        if self.range.1 >= 5.0 && score == 5.0 {
            return true;
        }

        // Return true if the score falls within the range `[range.0, range.1)`.
        self.range.0 <= score && score < self.range.1
    }
}

/// The type of the units stored in the dependency graph.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum UnitType {
    /// A single task, which the student is meant to perform and assess.
    Exercise,

    /// A set of related exercises. There are no dependencies between the exercises in a single
    /// lesson, so students could see them in any order. Lessons themselves can depend on other
    /// lessons or courses. There is also an implicit dependency between a lesson and the course to
    /// which it belongs.
    Lesson,

    /// A set of related lessons around one or more similar topics. Courses can depend on other
    /// lessons or courses.
    Course,
}

impl std::fmt::Display for UnitType {
    /// Implements the [Display](std::fmt::Display) trait for [UnitType].
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exercise => "Exercise".fmt(f),
            Self::Lesson => "Lesson".fmt(f),
            Self::Course => "Course".fmt(f),
        }
    }
}

/// Trait to convert relative paths to absolute paths so that objects stored in memory contain the
/// full path to all their assets.
pub trait NormalizePaths
where
    Self: Sized,
{
    /// Converts all relative paths in the object to absolute paths.
    fn normalize_paths(&self, working_dir: &Path) -> Result<Self>;
}

/// Converts a relative to an absolute path given a path and a working directory.
fn normalize_path(working_dir: &Path, path_str: &str) -> Result<String> {
    let path = Path::new(path_str);
    if path.is_absolute() {
        return Ok(path_str.to_string());
    }

    Ok(working_dir
        .join(path)
        .canonicalize()?
        .to_str()
        .unwrap_or(path_str)
        .to_string())
}

/// Trait to verify that the paths in the object are valid.
pub trait VerifyPaths
where
    Self: Sized,
{
    /// Checks that all the paths mentioned in the object exist in disk.
    fn verify_paths(&self, working_dir: &Path) -> Result<bool>;
}

/// Trait to get the metadata from a lesson or course manifest.
pub trait GetMetadata {
    /// Returns the manifest's metadata.
    fn get_metadata(&self) -> Option<&BTreeMap<String, Vec<String>>>;
}

/// Trait to get the unit type from a manifest.
pub trait GetUnitType {
    /// Returns the type of the unit associated with the manifest.
    fn get_unit_type(&self) -> UnitType;
}

/// An asset attached to a unit, which could be used to store instructions, or present the material
/// introduced by a course or lesson.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum BasicAsset {
    /// An asset containing the path to a markdown file.
    MarkdownAsset {
        /// The path to the markdown file.
        path: String,
    },

    /// An asset containing its content as a string.
    InlinedAsset {
        /// The content of the asset.
        content: String,
    },

    /// An asset containing its content as a unique string. Useful for generating assets that are
    /// replicated across many units.
    InlinedUniqueAsset {
        /// The content of the asset.
        content: Ustr,
    },
}

impl NormalizePaths for BasicAsset {
    fn normalize_paths(&self, working_dir: &Path) -> Result<Self> {
        match &self {
            BasicAsset::MarkdownAsset { path } => {
                let abs_path = normalize_path(working_dir, path)?;
                Ok(BasicAsset::MarkdownAsset { path: abs_path })
            }
            BasicAsset::InlinedAsset { .. } => Ok(self.clone()),
            BasicAsset::InlinedUniqueAsset { .. } => Ok(self.clone()),
        }
    }
}

impl VerifyPaths for BasicAsset {
    fn verify_paths(&self, working_dir: &Path) -> Result<bool> {
        match &self {
            BasicAsset::MarkdownAsset { path } => {
                let abs_path = working_dir.join(Path::new(path));
                Ok(abs_path.exists())
            }
            BasicAsset::InlinedAsset { .. } => Ok(true),
            BasicAsset::InlinedUniqueAsset { .. } => Ok(true),
        }
    }
}

//@<course-generator
/// A configuration used for generating special types of courses on the fly.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum CourseGenerator {
    /// The configuration for generating an improvisation course.
    Improvisation(ImprovisationConfig),

    /// The configuration for generating a knowledge base course. Currently, there are no
    /// configuration options, but the struct was added to implement the [GenerateManifests] trait
    /// and for future extensibility.
    KnowledgeBase(KnowledgeBaseConfig),

    /// The configuration for generating a music piece course.
    MusicPiece(MusicPieceConfig),

    /// The configuration for generating a transcription course.
    Transcription(TranscriptionConfig),
}
//>@course-generator

//@<user-preferences
/// The user-specific configuration
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct UserPreferences {
    /// The preferences for generating improvisation courses.
    pub improvisation: Option<ImprovisationPreferences>,

    /// The preferences for generating transcription courses.
    pub transcription: Option<TranscriptionPreferences>,
}
//>@user-preferences

/// A struct holding the results from running a course generator.
pub struct GeneratedCourse {
    /// The lessons and exercise manifests generated for the course.
    pub lessons: Vec<(LessonManifest, Vec<ExerciseManifest>)>,

    /// Updated course metadata. If None, the existing metadata is used.
    pub updated_metadata: Option<BTreeMap<String, Vec<String>>>,

    /// Updated course instructions. If None, the existing instructions are used.
    pub updated_instructions: Option<BasicAsset>,
}

/// The trait to return all the generated lesson and exercise manifests for a course.
pub trait GenerateManifests {
    /// Returns all the generated lesson and exercise manifests for a course.
    fn generate_manifests(
        &self,
        course_root: &Path,
        course_manifest: &CourseManifest,
        preferences: &UserPreferences,
    ) -> Result<GeneratedCourse>;
}

impl GenerateManifests for CourseGenerator {
    fn generate_manifests(
        &self,
        course_root: &Path,
        course_manifest: &CourseManifest,
        preferences: &UserPreferences,
    ) -> Result<GeneratedCourse> {
        match self {
            CourseGenerator::Improvisation(config) => {
                config.generate_manifests(course_root, course_manifest, preferences)
            }
            CourseGenerator::KnowledgeBase(config) => {
                config.generate_manifests(course_root, course_manifest, preferences)
            }
            CourseGenerator::MusicPiece(config) => {
                config.generate_manifests(course_root, course_manifest, preferences)
            }
            CourseGenerator::Transcription(config) => {
                config.generate_manifests(course_root, course_manifest, preferences)
            }
        }
    }
}

/// A manifest describing the contents of a course.
#[derive(Builder, Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CourseManifest {
    /// The ID assigned to this course.
    ///
    /// For example, `music::instrument::guitar::basic_jazz_chords`.
    #[builder(setter(into))]
    pub id: Ustr,

    /// The name of the course to be presented to the user.
    ///
    /// For example, "Basic Jazz Chords on Guitar".
    #[builder(default)]
    #[serde(default)]
    pub name: String,

    /// The IDs of all dependencies of this course.
    #[builder(default)]
    #[serde(default)]
    pub dependencies: Vec<Ustr>,

    /// An optional description of the course.
    #[builder(default)]
    #[serde(default)]
    pub description: Option<String>,

    /// An optional list of the course's authors.
    #[builder(default)]
    #[serde(default)]
    pub authors: Option<Vec<String>>,

    //@<lp-example-5
    //// A mapping of String keys to a list of String values. For example, ("genre", ["jazz"]) could
    /// be attached to a course named "Basic Jazz Chords on Guitar".
    ///
    /// The purpose of this metadata is to allow students to focus on more specific material during
    /// a study session which does not belong to a single lesson or course. For example, a student
    /// might want to only focus on guitar scales or ear training.
    #[builder(default)]
    #[serde(default)]
    pub metadata: Option<BTreeMap<String, Vec<String>>>,

    //>@lp-example-5
    /// An optional asset, which presents the material covered in the course.
    #[builder(default)]
    #[serde(default)]
    pub course_material: Option<BasicAsset>,

    /// An optional asset, which presents instructions common to all exercises in the course.
    #[builder(default)]
    #[serde(default)]
    pub course_instructions: Option<BasicAsset>,

    /// An optional configuration to generate material for this course. Generated courses allow
    /// easier creation of courses for specific purposes without requiring the manual creation of
    /// all the files a normal course would need.
    #[builder(default)]
    #[serde(default)]
    pub generator_config: Option<CourseGenerator>,
}

impl NormalizePaths for CourseManifest {
    fn normalize_paths(&self, working_directory: &Path) -> Result<Self> {
        let mut clone = self.clone();
        match &self.course_instructions {
            None => (),
            Some(asset) => {
                clone.course_instructions = Some(asset.normalize_paths(working_directory)?)
            }
        }
        match &self.course_material {
            None => (),
            Some(asset) => clone.course_material = Some(asset.normalize_paths(working_directory)?),
        }
        Ok(clone)
    }
}

impl VerifyPaths for CourseManifest {
    fn verify_paths(&self, working_dir: &Path) -> Result<bool> {
        // The paths mentioned in the instructions and material must both exist.
        let instructions_exist = match &self.course_instructions {
            None => true,
            Some(asset) => asset.verify_paths(working_dir)?,
        };
        let material_exists = match &self.course_material {
            None => true,
            Some(asset) => asset.verify_paths(working_dir)?,
        };
        Ok(instructions_exist && material_exists)
    }
}

impl GetMetadata for CourseManifest {
    fn get_metadata(&self) -> Option<&BTreeMap<String, Vec<String>>> {
        self.metadata.as_ref()
    }
}

impl GetUnitType for CourseManifest {
    fn get_unit_type(&self) -> UnitType {
        UnitType::Course
    }
}

/// A manifest describing the contents of a lesson.
#[derive(Builder, Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LessonManifest {
    /// The ID assigned to this lesson.
    ///
    /// For example, `music::instrument::guitar::basic_jazz_chords::major_chords`.
    #[builder(setter(into))]
    pub id: Ustr,

    /// The IDs of all dependencies of this lesson.
    #[builder(default)]
    #[serde(default)]
    pub dependencies: Vec<Ustr>,

    /// The ID of the course to which the lesson belongs.
    #[builder(setter(into))]
    pub course_id: Ustr,

    /// The name of the lesson to be presented to the user.
    ///
    /// For example, "Basic Jazz Major Chords".
    #[builder(default)]
    #[serde(default)]
    pub name: String,

    /// An optional description of the lesson.
    #[builder(default)]
    #[serde(default)]
    pub description: Option<String>,

    //// A mapping of String keys to a list of String values. For example, ("key", ["C"]) could
    /// be attached to a lesson named "C Major Scale". The purpose is the same as the metadata
    /// stored in the course manifest but allows finer control over which lessons are selected.
    #[builder(default)]
    #[serde(default)]
    pub metadata: Option<BTreeMap<String, Vec<String>>>,

    /// An optional asset, which presents the material covered in the lesson.
    #[builder(default)]
    #[serde(default)]
    pub lesson_material: Option<BasicAsset>,

    /// An optional asset, which presents instructions common to all exercises in the lesson.
    #[builder(default)]
    #[serde(default)]
    pub lesson_instructions: Option<BasicAsset>,
}

impl NormalizePaths for LessonManifest {
    fn normalize_paths(&self, working_dir: &Path) -> Result<Self> {
        let mut clone = self.clone();
        match &self.lesson_instructions {
            None => (),
            Some(asset) => clone.lesson_instructions = Some(asset.normalize_paths(working_dir)?),
        }
        match &self.lesson_material {
            None => (),
            Some(asset) => clone.lesson_material = Some(asset.normalize_paths(working_dir)?),
        }
        Ok(clone)
    }
}

impl VerifyPaths for LessonManifest {
    fn verify_paths(&self, working_dir: &Path) -> Result<bool> {
        // The paths mentioned in the instructions and material must both exist.
        let instruction_exists = match &self.lesson_instructions {
            None => true,
            Some(asset) => asset.verify_paths(working_dir)?,
        };
        let material_exists = match &self.lesson_material {
            None => true,
            Some(asset) => asset.verify_paths(working_dir)?,
        };
        Ok(instruction_exists && material_exists)
    }
}

impl GetMetadata for LessonManifest {
    fn get_metadata(&self) -> Option<&BTreeMap<String, Vec<String>>> {
        self.metadata.as_ref()
    }
}

impl GetUnitType for LessonManifest {
    fn get_unit_type(&self) -> UnitType {
        UnitType::Lesson
    }
}

/// The type of knowledge tested by an exercise.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub enum ExerciseType {
    /// Represents an exercise that tests mastery of factual knowledge. For example, an exercise
    /// asking students to name the notes in a D Major chord.
    Declarative,

    /// Represents an exercises that requires more complex actions to be performed. For example, an
    /// exercise asking students to play a D Major chords in a piano.
    #[default]
    Procedural,
}

/// The asset storing the material of a particular exercise.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum ExerciseAsset {
    /// An asset which stores a link to a SoundSlice.
    SoundSliceAsset {
        /// The link to the SoundSlice asset.
        link: String,

        /// An optional description of the exercise tied to this asset. For example, "Play this
        /// slice in the key of D Major" or "Practice measures 1 through 4". A missing description
        /// implies the entire slice should be practiced as is.
        #[serde(default)]
        description: Option<String>,

        /// An optional path to a MusicXML file containing the sheet music for the exercise.
        #[serde(default)]
        backup: Option<String>,
    },

    /// An asset representing a flashcard with a front and back each stored in a markdown file. The
    /// first file stores the front (question) of the flashcard while the second file stores the
    /// back (answer).
    FlashcardAsset {
        /// The path to the file containing the front of the flashcard.
        front_path: String,

        /// The path to the file containing the back of the flashcard. This path is optional,
        /// because a flashcard is not required to provide an answer. For example, the exercise is
        /// open-ended, or it is referring to an external resource which contains the exercise and
        /// possibly the answer.
        #[serde(default)]
        back_path: Option<String>,
    },

    /// A basic asset storing the material of the exercise.
    BasicAsset(BasicAsset),
}

impl NormalizePaths for ExerciseAsset {
    fn normalize_paths(&self, working_dir: &Path) -> Result<Self> {
        match &self {
            ExerciseAsset::FlashcardAsset {
                front_path,
                back_path,
            } => {
                let abs_front_path = normalize_path(working_dir, front_path)?;
                let abs_back_path = if let Some(back_path) = back_path {
                    Some(normalize_path(working_dir, back_path)?)
                } else {
                    None
                };
                Ok(ExerciseAsset::FlashcardAsset {
                    front_path: abs_front_path,
                    back_path: abs_back_path,
                })
            }
            ExerciseAsset::SoundSliceAsset {
                link,
                description,
                backup,
            } => match backup {
                None => Ok(self.clone()),
                Some(path) => {
                    let abs_path = normalize_path(working_dir, path)?;
                    Ok(ExerciseAsset::SoundSliceAsset {
                        link: link.clone(),
                        description: description.clone(),
                        backup: Some(abs_path),
                    })
                }
            },
            ExerciseAsset::BasicAsset(asset) => Ok(ExerciseAsset::BasicAsset(
                asset.normalize_paths(working_dir)?,
            )),
        }
    }
}

impl VerifyPaths for ExerciseAsset {
    fn verify_paths(&self, working_dir: &Path) -> Result<bool> {
        match &self {
            ExerciseAsset::FlashcardAsset {
                front_path,
                back_path,
            } => {
                let front_abs_path = working_dir.join(Path::new(front_path));
                if let Some(back_path) = back_path {
                    // The paths to the front and back of the flashcard must both exist.
                    let back_abs_path = working_dir.join(Path::new(back_path));
                    Ok(front_abs_path.exists() && back_abs_path.exists())
                } else {
                    // If the back of the flashcard is missing, then the front must exist.
                    Ok(front_abs_path.exists())
                }
            }
            ExerciseAsset::SoundSliceAsset { backup, .. } => match backup {
                None => Ok(true),
                Some(path) => {
                    // The backup path must exist.
                    let abs_path = working_dir.join(Path::new(path));
                    Ok(abs_path.exists())
                }
            },
            ExerciseAsset::BasicAsset(asset) => asset.verify_paths(working_dir),
        }
    }
}

/// Manifest describing a single exercise.
#[derive(Builder, Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ExerciseManifest {
    /// The ID assigned to this exercise.
    ///
    /// For example, `music::instrument::guitar::basic_jazz_chords::major_chords::exercise_1`.
    #[builder(setter(into))]
    pub id: Ustr,

    /// The ID of the lesson to which this exercise belongs.
    #[builder(setter(into))]
    pub lesson_id: Ustr,

    /// The ID of the course to which this exercise belongs.
    #[builder(setter(into))]
    pub course_id: Ustr,

    /// The name of the exercise to be presented to the user.
    ///
    /// For example, "Exercise 1".
    #[builder(default)]
    #[serde(default)]
    pub name: String,

    /// An optional description of the exercise.
    #[builder(default)]
    #[serde(default)]
    pub description: Option<String>,

    /// The type of knowledge the exercise tests.
    #[builder(default)]
    #[serde(default)]
    pub exercise_type: ExerciseType,

    /// The asset containing the exercise itself.
    pub exercise_asset: ExerciseAsset,
}

impl NormalizePaths for ExerciseManifest {
    fn normalize_paths(&self, working_dir: &Path) -> Result<Self> {
        let mut clone = self.clone();
        clone.exercise_asset = clone.exercise_asset.normalize_paths(working_dir)?;
        Ok(clone)
    }
}

impl VerifyPaths for ExerciseManifest {
    fn verify_paths(&self, working_dir: &Path) -> Result<bool> {
        self.exercise_asset.verify_paths(working_dir)
    }
}

impl GetUnitType for ExerciseManifest {
    fn get_unit_type(&self) -> UnitType {
        UnitType::Exercise
    }
}

/// Options to compute the passing score for a unit.
#[derive(Clone, Debug, PartialEq)]
pub enum PassingScoreOptions {
    /// The score will be a fixed value.
    ConstantScore(f32),

    /// The score will start at a fixed value and increase by a fixed amount based on the depth of
    /// the unit relative to the starting unit. This is useful for allowing users to make faster
    /// progress at the beginning, so to avoid boredom. Once enough of the graph has been mastered,
    /// the passing score will settle to a fixed value.
    IncreasingScore {
        /// The initial score. The units at the starting depth will use this value as their passing
        /// score.
        starting_score: f32,

        /// The amount by which the score will increase for each additional depth. For example, if
        /// the unit is at depth 2, then the passing score will increase by `step_size * 2`.
        step_size: f32,

        /// The maximum number of steps that increase the passing score. Units that are deeper than
        /// this will have a passing score of `starting_score + step_size * max_steps`.
        max_steps: usize,
    },
}

impl Default for PassingScoreOptions {
    fn default() -> Self {
        PassingScoreOptions::IncreasingScore {
            starting_score: 3.50,
            step_size: 0.05,
            max_steps: 5,
        }
    }
}

impl PassingScoreOptions {
    /// Computes the passing score for a unit at the given depth.
    pub fn compute_score(&self, depth: usize) -> f32 {
        match self {
            PassingScoreOptions::ConstantScore(score) => score.min(5.0),
            PassingScoreOptions::IncreasingScore {
                starting_score,
                step_size,
                max_steps,
            } => {
                let steps = depth.min(*max_steps);
                (starting_score + step_size * steps as f32).min(5.0)
            }
        }
    }

    /// Verifies that the options are valid.
    pub fn verify(&self) -> Result<()> {
        match self {
            PassingScoreOptions::ConstantScore(score) => {
                if *score < 0.0 || *score > 5.0 {
                    bail!("Invalid score: {}", score);
                }
                Ok(())
            }
            PassingScoreOptions::IncreasingScore {
                starting_score,
                step_size,
                ..
            } => {
                if *starting_score < 0.0 || *starting_score > 5.0 {
                    bail!("Invalid starting score: {}", starting_score);
                }
                if *step_size < 0.0 {
                    bail!("Invalid step size: {}", step_size);
                }
                Ok(())
            }
        }
    }
}

/// Options to control how the scheduler selects exercises.
#[derive(Clone, Debug)]
pub struct SchedulerOptions {
    /// The maximum number of candidates to return each time the scheduler is worked.
    pub batch_size: usize,

    /// The options of the target mastery window. That is, the window of exercises that lie outside
    /// the user's current abilities.
    pub target_window_opts: MasteryWindow,

    /// The options of the current mastery window. That is, the window of exercises that lie
    /// slightly outside the user's current abilities.
    pub current_window_opts: MasteryWindow,

    /// The options of the easy mastery window. That is, the window of exercises that lie well
    /// within the user's current abilities.
    pub easy_window_opts: MasteryWindow,

    /// The options for the mastered mastery window. That is, the window of exercises that the user
    /// has properly mastered.
    pub mastered_window_opts: MasteryWindow,

    /// The minimum average score of a unit required to move on to its dependents.
    pub passing_score: PassingScoreOptions,

    /// The number of trials to retrieve from the practice stats to compute an exercise's score.
    pub num_trials: usize,
}

impl SchedulerOptions {
    /// Verifies that the scheduler options are valid.
    pub fn verify(&self) -> Result<()> {
        // The batch size must be greater than 0.
        if self.batch_size == 0 {
            bail!("invalid scheduler options: batch_size must be greater than 0");
        }

        // The sum of the percentages of the mastery windows must be 1.0.
        if self.mastered_window_opts.percentage
            + self.easy_window_opts.percentage
            + self.current_window_opts.percentage
            + self.target_window_opts.percentage
            != 1.0
        {
            bail!(
                "invalid scheduler options: the sum of the percentages of the mastery windows \
                must be 1.0"
            );
        }

        // The target window's range must start at 0.0.
        if self.target_window_opts.range.0 != 0.0 {
            bail!("invalid scheduler options: the target window's range must start at 0.0");
        }

        // The mastered window's range must end at 5.0.
        if self.mastered_window_opts.range.1 != 5.0 {
            bail!("invalid scheduler options: the mastered window's range must end at 5.0");
        }

        // There must be no gaps in the mastery windows.
        if self.target_window_opts.range.1 != self.current_window_opts.range.0
            || self.current_window_opts.range.1 != self.easy_window_opts.range.0
            || self.easy_window_opts.range.1 != self.mastered_window_opts.range.0
        {
            bail!("invalid scheduler options: there must be no gaps in the mastery windows");
        }

        Ok(())
    }
}

impl Default for SchedulerOptions {
    /// Returns the default scheduler options.
    fn default() -> Self {
        SchedulerOptions {
            batch_size: 50,
            target_window_opts: MasteryWindow {
                percentage: 0.2,
                range: (0.0, 2.5),
            },
            current_window_opts: MasteryWindow {
                percentage: 0.5,
                range: (2.5, 3.75),
            },
            easy_window_opts: MasteryWindow {
                percentage: 0.2,
                range: (3.75, 4.7),
            },
            mastered_window_opts: MasteryWindow {
                percentage: 0.1,
                range: (4.7, 5.0),
            },
            passing_score: PassingScoreOptions::default(),
            num_trials: 20,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::data::*;

    #[test]
    fn float_score() {
        assert_eq!(1.0, MasteryScore::One.float_score());
        assert_eq!(2.0, MasteryScore::Two.float_score());
        assert_eq!(3.0, MasteryScore::Three.float_score());
        assert_eq!(4.0, MasteryScore::Four.float_score());
        assert_eq!(5.0, MasteryScore::Five.float_score());
    }

    /// Verifies that each type of manifest returns the correct unit type.
    #[test]
    fn get_unit_type() {
        assert_eq!(
            UnitType::Course,
            CourseManifestBuilder::default()
                .id("test")
                .name("Test".to_string())
                .dependencies(vec![])
                .build()
                .unwrap()
                .get_unit_type()
        );
        assert_eq!(
            UnitType::Lesson,
            LessonManifestBuilder::default()
                .id("test")
                .course_id("test")
                .name("Test".to_string())
                .dependencies(vec![])
                .build()
                .unwrap()
                .get_unit_type()
        );
        assert_eq!(
            UnitType::Exercise,
            ExerciseManifestBuilder::default()
                .id("test")
                .course_id("test")
                .lesson_id("test")
                .name("Test".to_string())
                .exercise_type(ExerciseType::Procedural)
                .exercise_asset(ExerciseAsset::FlashcardAsset {
                    front_path: "front.png".to_string(),
                    back_path: Some("back.png".to_string()),
                })
                .build()
                .unwrap()
                .get_unit_type()
        );
    }

    /// Verifies that checking the paths of a manifest works if there are no paths to check.
    #[test]
    fn verify_paths_none() -> Result<()> {
        let lesson_manifest = LessonManifestBuilder::default()
            .id("test")
            .course_id("test")
            .name("Test".to_string())
            .dependencies(vec![])
            .build()
            .unwrap();
        lesson_manifest.verify_paths(Path::new("./"))?;

        let course_manifest = CourseManifestBuilder::default()
            .id("test")
            .name("Test".to_string())
            .dependencies(vec![])
            .build()
            .unwrap();
        course_manifest.verify_paths(Path::new("./"))?;
        Ok(())
    }

    /// Verifies the `NormalizePaths` trait works for a SoundSlice asset.
    #[test]
    fn soundslice_normalize_paths() -> Result<()> {
        let soundslice = ExerciseAsset::SoundSliceAsset {
            link: "https://www.soundslice.com/slices/QfZcc/".to_string(),
            description: Some("Test".to_string()),
            backup: None,
        };
        soundslice.normalize_paths(Path::new("./"))?;

        let temp_dir = tempfile::tempdir()?;
        let temp_file = tempfile::NamedTempFile::new_in(temp_dir.path())?;
        let soundslice = ExerciseAsset::SoundSliceAsset {
            link: "https://www.soundslice.com/slices/QfZcc/".to_string(),
            description: Some("Test".to_string()),
            backup: Some(temp_file.path().as_os_str().to_str().unwrap().to_string()),
        };
        soundslice.normalize_paths(temp_dir.path())?;
        Ok(())
    }

    /// Verifies the `VerifyPaths` trait works for a SoundSlice asset.
    #[test]
    fn soundslice_verify_paths() -> Result<()> {
        let soundslice = ExerciseAsset::SoundSliceAsset {
            link: "https://www.soundslice.com/slices/QfZcc/".to_string(),
            description: Some("Test".to_string()),
            backup: None,
        };
        assert!(soundslice.verify_paths(Path::new("./"))?);

        let soundslice = ExerciseAsset::SoundSliceAsset {
            link: "https://www.soundslice.com/slices/QfZcc/".to_string(),
            description: Some("Test".to_string()),
            backup: Some("./bad_file".to_string()),
        };
        assert!(!soundslice.verify_paths(Path::new("./"))?);
        Ok(())
    }

    /// Verifies the `NormalizePaths` trait works for an inlined asset.
    #[test]
    fn normalize_inlined_assets() -> Result<()> {
        let inlined_asset = BasicAsset::InlinedAsset {
            content: "Test".to_string(),
        };
        inlined_asset.normalize_paths(Path::new("./"))?;

        let inlined_asset = BasicAsset::InlinedUniqueAsset {
            content: Ustr::from("Test"),
        };
        inlined_asset.normalize_paths(Path::new("./"))?;
        Ok(())
    }

    /// Verifies the `VerifyPaths` trait works for an inlined asset.
    #[test]
    fn verify_inlined_assets() -> Result<()> {
        let inlined_asset = BasicAsset::InlinedAsset {
            content: "Test".to_string(),
        };
        assert!(inlined_asset.verify_paths(Path::new("./"))?);

        let inlined_asset = BasicAsset::InlinedUniqueAsset {
            content: Ustr::from("Test"),
        };
        assert!(inlined_asset.verify_paths(Path::new("./"))?);
        Ok(())
    }

    /// Verifies the `VerifyPaths` trait works for a flashcard asset.
    #[test]
    fn verify_flashcard_assets() -> Result<()> {
        // Verify a flashcard with no back.
        let temp_dir = tempfile::tempdir()?;
        let front_file = tempfile::NamedTempFile::new_in(temp_dir.path())?;
        let flashcard_asset = ExerciseAsset::FlashcardAsset {
            front_path: front_file.path().as_os_str().to_str().unwrap().to_string(),
            back_path: None,
        };
        assert!(flashcard_asset.verify_paths(temp_dir.path())?);

        // Verify a flashcard with front and back.
        let back_file = tempfile::NamedTempFile::new_in(temp_dir.path())?;
        let flashcard_asset = ExerciseAsset::FlashcardAsset {
            front_path: front_file.path().as_os_str().to_str().unwrap().to_string(),
            back_path: Some(back_file.path().as_os_str().to_str().unwrap().to_string()),
        };
        assert!(flashcard_asset.verify_paths(temp_dir.path())?);
        Ok(())
    }

    /// Verifies the `Display` trait for each unit type.
    #[test]
    fn unit_type_display() {
        assert_eq!("Course", UnitType::Course.to_string());
        assert_eq!("Lesson", UnitType::Lesson.to_string());
        assert_eq!("Exercise", UnitType::Exercise.to_string());
    }

    /// Verifies that normalizing a path works with the path to a valid file.
    #[test]
    fn normalize_good_path() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let temp_file = tempfile::NamedTempFile::new_in(temp_dir.path())?;
        let temp_file_path = temp_file.path().to_str().unwrap();
        let normalized_path = normalize_path(temp_dir.path(), temp_file_path)?;
        assert_eq!(
            temp_dir.path().join(temp_file_path).to_str().unwrap(),
            normalized_path
        );
        Ok(())
    }

    /// Verifies that normalizing an absolute path returns the original path.
    #[test]
    fn normalize_absolute_path() {
        let normalized_path = normalize_path(Path::new("/working/dir"), "/absolute/path").unwrap();
        assert_eq!("/absolute/path", normalized_path,);
    }

    /// Verifies that normalizing a path fails with the path to a missing file.
    #[test]
    fn normalize_bad_path() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let temp_file_path = "missing_file";
        assert!(normalize_path(temp_dir.path(), temp_file_path).is_err());
        Ok(())
    }

    /// Verifies the `VerifyPaths` trait works for a basic exercise asset.
    #[test]
    fn exercise_basic_asset_verify_paths() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let basic_asset = ExerciseAsset::BasicAsset(BasicAsset::InlinedAsset {
            content: "my content".to_string(),
        });
        assert!(basic_asset.verify_paths(temp_dir.path())?);
        Ok(())
    }

    /// Verifies the `NormalizePaths` trait works for a basic exercise asset.
    #[test]
    fn exercise_basic_asset_normalize_paths() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let basic_asset = ExerciseAsset::BasicAsset(BasicAsset::InlinedAsset {
            content: "my content".to_string(),
        });
        basic_asset.normalize_paths(temp_dir.path())?;
        Ok(())
    }

    /// Verifies the default scheduler options are valid.
    #[test]
    fn valid_default_scheduler_options() {
        let options = SchedulerOptions::default();
        assert!(options.verify().is_ok());
    }

    /// Verifies scheduler options with a batch size of 0 are invalid.
    #[test]
    fn scheduler_options_invalid_batch_size() {
        let mut options = SchedulerOptions::default();
        options.batch_size = 0;
        assert!(options.verify().is_err());
    }

    /// Verifies scheduler options with an invalid mastered window range are invalid.
    #[test]
    fn scheduler_options_invalid_mastered_window() {
        let mut options = SchedulerOptions::default();
        options.mastered_window_opts.range.1 = 4.9;
        assert!(options.verify().is_err());
    }

    /// Verifies scheduler options with an invalid target window range are invalid.
    #[test]
    fn scheduler_options_invalid_target_window() {
        let mut options = SchedulerOptions::default();
        options.target_window_opts.range.0 = 0.1;
        assert!(options.verify().is_err());
    }

    /// Verifies that scheduler options with a gap in the windows are invalid.
    #[test]
    fn scheduler_options_gap_in_windows() {
        let mut options = SchedulerOptions::default();
        options.target_window_opts.range.1 -= 0.1;
        assert!(options.verify().is_err());

        let mut options = SchedulerOptions::default();
        options.current_window_opts.range.1 -= 0.1;
        assert!(options.verify().is_err());

        let mut options = SchedulerOptions::default();
        options.easy_window_opts.range.1 -= 0.1;
        assert!(options.verify().is_err());
    }

    /// Verifies that scheduler options with a percentage sum other than 1 are invalid.
    #[test]
    fn scheduler_options_invalid_percentage_sum() {
        let mut options = SchedulerOptions::default();
        options.target_window_opts.percentage -= 0.1;
        assert!(options.verify().is_err());
    }

    /// Verifies that valid passing score options are recognized as such.
    #[test]
    fn verify_passing_score_options() {
        let options = PassingScoreOptions::default();
        assert!(options.verify().is_ok());

        let options = PassingScoreOptions::ConstantScore(3.50);
        assert!(options.verify().is_ok());
    }

    /// Verifies that invalid passing score options are recognized as such.
    #[test]
    fn verify_passing_score_options_invalid() {
        let options = PassingScoreOptions::ConstantScore(-1.0);
        assert!(options.verify().is_err());

        let options = PassingScoreOptions::ConstantScore(6.0);
        assert!(options.verify().is_err());

        let options = PassingScoreOptions::IncreasingScore {
            starting_score: -1.0,
            step_size: 0.0,
            max_steps: 0,
        };
        assert!(options.verify().is_err());

        let options = PassingScoreOptions::IncreasingScore {
            starting_score: 6.0,
            step_size: 0.0,
            max_steps: 0,
        };
        assert!(options.verify().is_err());

        let options = PassingScoreOptions::IncreasingScore {
            starting_score: 3.50,
            step_size: -1.0,
            max_steps: 0,
        };
        assert!(options.verify().is_err());
    }

    /// Verifies that the passing score is computed correctly.
    #[test]
    fn compute_passing_score() {
        let options = PassingScoreOptions::ConstantScore(3.50);
        assert_eq!(options.compute_score(0), 3.50);
        assert_eq!(options.compute_score(1), 3.50);
        assert_eq!(options.compute_score(2), 3.50);
        // Clone the score for code coverage.
        assert_eq!(options, options.clone());

        let options = PassingScoreOptions::default();
        assert_eq!(options.compute_score(0), 3.50);
        assert_eq!(options.compute_score(1), 3.55);
        assert_eq!(options.compute_score(2), 3.60);
        assert_eq!(options.compute_score(5), 3.75);
        assert_eq!(options.compute_score(10), 3.75);
        // Clone the score for code coverage.
        assert_eq!(options, options.clone());
    }
}
