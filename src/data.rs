pub mod filter;

use std::{collections::HashMap, path::Path};

use anyhow::Result;
use derive_builder::Builder;
use serde::{Deserialize, Serialize};

/// Score used by students to evaluate their mastery of a particular exercise after a trial. More
/// detailed descriptions of the levels are provided using the example of an exercise that requires
/// the student to learn a musical passage.
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
    /// material slowly with barely any mistakes, and can has begun to learn it at higher tempos.
    Three,

    /// Four signifies the student has gained mastery of the exercise, requiring almost not
    /// conscious thought to complete it. For a musical passage, this level of mastery represents
    /// the stage at which the student can perform the material at the desired tempo with all
    /// elements (rhythm, dynamics, etc) completely integrated into the performance.
    Four,

    /// Five signifies the student has gained total masterey of the material and can apply it in
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
    /// Assigns a f32 value to each of the values of MasteryScore.
    pub fn float_score(&self) -> f32 {
        match self {
            &Self::One => 1.0,
            &Self::Two => 2.0,
            &Self::Three => 3.0,
            &Self::Four => 4.0,
            &Self::Five => 5.0,
        }
    }
}

/// Options to configure a mastery window.
#[derive(Clone, Debug)]
pub struct MasteryWindowOpts {
    /// The percentage of total candidates which fall on this window.
    pub percentage: f32,

    /// The range of scores which fall on this window. Scores whose values are in the range [start,
    /// end) fall within this window.
    pub range: (f32, f32),
}

impl MasteryWindowOpts {
    pub fn in_window(&self, score: f32) -> bool {
        self.range.0 <= score && score < self.range.1
    }
}

/// The result of a single trial.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExerciseTrial {
    /// The score assigned to the exercise after the trial.
    pub score: f32,

    /// The timestamp at which the trial happened.
    pub timestamp: i64,
}

/// The type of a unit.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum UnitType {
    /// A single task, which the student is meant to perform and assess.
    Exercise,

    /// A set of related exercises. There are no dependencies between the exercises
    /// in a single lesson, so students could see them in any order.
    Lesson,

    /// A set of related lessons around one or more similar topics. Lessons in the same course can
    /// have dependency relationships among each other. The course itself implicitly depends on all
    /// lessons. That is to say, in order to achieve mastery of the material in a course, mastery of
    /// the material in all the lessons is required.
    Course,
}

pub trait NormalizePaths
where
    Self: Sized,
{
    /// Converts all relative paths in the object to absolute paths.
    fn normalize_paths(&self, dir: &Path) -> Result<Self>;
}

pub trait VerifyPaths
where
    Self: Sized,
{
    /// Checks that all the paths mentioned in the object exist in disk.
    fn verify_paths(&self, dir: &Path) -> Result<bool>;
}

pub trait GetMetadata {
    /// Returns the object's metadata.
    fn get_metadata(&self) -> Option<&HashMap<String, Vec<String>>>;
}

pub trait GetUnitType {
    /// Returns the type of the unit associated with the manifest.
    fn get_unit_type(&self) -> UnitType;
}

/// An asset attached to a unit, which could be used to store instructions, or present the material
/// introduced by a lesson.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum BasicAsset {
    /// An asset containing the path to a markdown file.
    MarkdownAsset { path: String },
}

impl NormalizePaths for BasicAsset {
    fn normalize_paths(&self, dir: &Path) -> Result<Self> {
        match &self {
            BasicAsset::MarkdownAsset { path } => {
                let abs_path = dir
                    .join(Path::new(path))
                    .canonicalize()?
                    .to_str()
                    .unwrap_or(path)
                    .to_string();
                Ok(BasicAsset::MarkdownAsset { path: abs_path })
            }
        }
    }
}

impl VerifyPaths for BasicAsset {
    fn verify_paths(&self, dir: &Path) -> Result<bool> {
        match &self {
            BasicAsset::MarkdownAsset { path } => {
                let abs_path = dir.join(Path::new(path));
                Ok(abs_path.exists())
            }
        }
    }
}

/// A manifest describing the contents of a course.
#[derive(Builder, Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CourseManifest {
    /// The ID assigned to this course.
    /// For example, "music::instrument::guitar::basic_jazz_chords".
    pub id: String,

    /// The name of the course to be presented to the user.
    /// For example, "Basic Jazz Chords on Guitar".
    pub name: String,

    /// The IDs of all dependencies of this course.
    pub dependencies: Vec<String>,

    /// An optional description of the course.
    #[builder(default)]
    pub description: Option<String>,

    /// An optional list of the course's authors.
    #[builder(default)]
    pub authors: Option<Vec<String>>,

    //// A mapping of String keys to a list of String values. For example, ("genre", ["jazz"]) could
    /// be attached to a course named "Basic Jazz Chords on Guitar".
    #[builder(default)]
    pub metadata: Option<HashMap<String, Vec<String>>>,

    /// An optional asset, which presents the material covered in the course.
    #[builder(default)]
    pub course_material: Option<BasicAsset>,

    /// An optional asset, which presents instructions common to all exercises in the course.
    #[builder(default)]
    pub course_instructions: Option<BasicAsset>,
}

impl NormalizePaths for CourseManifest {
    fn normalize_paths(&self, dir: &Path) -> Result<Self> {
        let mut clone = self.clone();
        match &self.course_material {
            None => (),
            Some(asset) => clone.course_material = Some(asset.normalize_paths(dir)?),
        }
        Ok(clone)
    }
}

impl VerifyPaths for CourseManifest {
    fn verify_paths(&self, dir: &Path) -> Result<bool> {
        match &self.course_material {
            None => Ok(true),
            Some(asset) => asset.verify_paths(dir),
        }
    }
}

impl GetMetadata for CourseManifest {
    fn get_metadata(&self) -> Option<&HashMap<String, Vec<String>>> {
        self.metadata.as_ref()
    }
}

impl GetUnitType for CourseManifest {
    fn get_unit_type(&self) -> UnitType {
        UnitType::Course
    }
}

/// A manifest describing the contents of a lesson.
#[derive(Builder, Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LessonManifest {
    /// The ID assigned to this lesson. For example,
    /// "music::instrument::guitar::basic_jazz_chords::major_chords".
    pub id: String,

    /// The IDs of all dependencies of this lesson.
    pub dependencies: Vec<String>,

    /// The ID of the course to which the lesson belongs.
    pub course_id: String,

    /// The name of the lesson to be presented to the user. For example, "Basic Jazz Major Chords".
    pub name: String,

    /// An optional description of the lesson.
    #[builder(default)]
    pub description: Option<String>,

    //// A mapping of String keys to a list of String values. For example, ("key", ["C"]) could
    /// be attached to a lesson named "C Major Scale".
    #[builder(default)]
    pub metadata: Option<HashMap<String, Vec<String>>>,

    /// An optional asset, which presents the material covered in the lesson.
    #[builder(default)]
    pub lesson_material: Option<BasicAsset>,

    /// An optional asset, which presents instructions common to all exercises in the lesson.
    #[builder(default)]
    pub lesson_instructions: Option<BasicAsset>,
}

impl NormalizePaths for LessonManifest {
    fn normalize_paths(&self, dir: &Path) -> Result<Self> {
        let mut clone = self.clone();
        match &self.lesson_instructions {
            None => (),
            Some(asset) => clone.lesson_instructions = Some(asset.normalize_paths(dir)?),
        }
        match &self.lesson_material {
            None => (),
            Some(asset) => clone.lesson_material = Some(asset.normalize_paths(dir)?),
        }
        Ok(clone)
    }
}

impl VerifyPaths for LessonManifest {
    fn verify_paths(&self, dir: &Path) -> Result<bool> {
        let instruction_exists = match &self.lesson_instructions {
            None => true,
            Some(asset) => asset.verify_paths(dir)?,
        };
        let lesson_exists = match &self.lesson_material {
            None => true,
            Some(asset) => asset.verify_paths(dir)?,
        };
        Ok(instruction_exists && lesson_exists)
    }
}

impl GetMetadata for LessonManifest {
    fn get_metadata(&self) -> Option<&HashMap<String, Vec<String>>> {
        self.metadata.as_ref()
    }
}

impl GetUnitType for LessonManifest {
    fn get_unit_type(&self) -> UnitType {
        UnitType::Lesson
    }
}

/// The type of knowledge tested by an exercise.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ExerciseType {
    /// Represents an exercise that tests mastery of factual knowledge. For example, an exercise
    /// asking students to name the notes in a D Major chord.
    Declarative,

    /// Represents an exercises that requires more complex actions to be performed. For example, an
    /// exercise asking students to play a D Major chords in a piano.
    Procedural,
}

/// The asset storing the material of a particular exercise.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub enum ExerciseAsset {
    /// An asset which stores a link to a SoundSlice.
    SoundSliceAsset { link: String },

    /// An asset storing two paths to two markdown files. The first file stores the front (question)
    /// of the flashcard while the second file stores the back (answer).
    FlashcardAsset {
        front_path: String,
        back_path: String,
    },
}

impl NormalizePaths for ExerciseAsset {
    fn normalize_paths(&self, dir: &Path) -> Result<Self> {
        match &self {
            ExerciseAsset::FlashcardAsset {
                front_path,
                back_path,
            } => {
                let abs_front_path = dir.join(Path::new(front_path));
                let abs_back_path = dir.join(Path::new(back_path));

                Ok(ExerciseAsset::FlashcardAsset {
                    front_path: abs_front_path
                        .canonicalize()?
                        .to_str()
                        .unwrap_or(front_path)
                        .to_string(),
                    back_path: abs_back_path
                        .canonicalize()?
                        .to_str()
                        .unwrap_or(back_path)
                        .to_string(),
                })
            }
            ExerciseAsset::SoundSliceAsset { .. } => Ok(self.clone()),
        }
    }
}

impl VerifyPaths for ExerciseAsset {
    fn verify_paths(&self, dir: &Path) -> Result<bool> {
        match &self {
            ExerciseAsset::FlashcardAsset {
                front_path,
                back_path,
            } => {
                let front_abs_path = dir.join(Path::new(front_path));
                let back_abs_path = dir.join(Path::new(back_path));
                Ok(front_abs_path.exists() && back_abs_path.exists())
            }
            ExerciseAsset::SoundSliceAsset { .. } => Ok(true),
        }
    }
}

/// Manifest describing a single exercise.
#[derive(Builder, Clone, Debug, Deserialize, Serialize)]
pub struct ExerciseManifest {
    /// The ID assigned to this exercise. For example,
    /// "music::instrument::guitar::basic_jazz_chords::major_chords::ex_1".
    pub id: String,

    /// The ID of the lesson to which this exercise belongs.
    pub lesson_id: String,

    /// The ID of the course to which this exercise belongs.
    pub course_id: String,

    /// The name of the lesson to be presented to the user.
    /// For example, "Exercise 1".
    pub name: String,

    /// An optional description of the exercise.
    #[builder(default)]
    pub description: Option<String>,

    /// The type of the exercised categorized by the type of knowledge it tests.
    pub exercise_type: ExerciseType,

    /// The asset containing the exercise itself.
    pub exercise_asset: ExerciseAsset,
}

impl NormalizePaths for ExerciseManifest {
    fn normalize_paths(&self, dir: &Path) -> Result<Self> {
        let mut clone = self.clone();
        clone.exercise_asset = clone.exercise_asset.normalize_paths(dir)?;
        Ok(clone)
    }
}

impl VerifyPaths for ExerciseManifest {
    fn verify_paths(&self, dir: &Path) -> Result<bool> {
        self.exercise_asset.verify_paths(dir)
    }
}

impl GetUnitType for ExerciseManifest {
    fn get_unit_type(&self) -> UnitType {
        UnitType::Exercise
    }
}

/// Options to control how the scheduler selects exercises.
#[derive(Clone, Debug)]
pub struct SchedulerOptions {
    /// The maximum number of candidates to return each time the scheduler is worked.
    pub batch_size: usize,

    /// The options of the target mastery window.
    pub target_window_opts: MasteryWindowOpts,

    /// The options of the current mastery window.
    pub current_window_opts: MasteryWindowOpts,

    /// The options of the easy mastery window.
    pub easy_window_opts: MasteryWindowOpts,

    /// The minimum average score of a unit required to move on to its dependents.
    pub passing_score: f32,

    /// The maximum number of scores to lookup in the practice stats.
    pub num_scores: usize,
}

impl Default for SchedulerOptions {
    /// Returns scheduler options with sensible default values.
    fn default() -> Self {
        SchedulerOptions {
            batch_size: 50,
            target_window_opts: MasteryWindowOpts {
                percentage: 0.25,
                range: (0.0, 2.5),
            },
            current_window_opts: MasteryWindowOpts {
                percentage: 0.5,
                range: (2.5, 3.9),
            },
            easy_window_opts: MasteryWindowOpts {
                percentage: 0.25,
                range: (3.9, 5.0),
            },
            passing_score: 3.9,
            num_scores: 25,
        }
    }
}
