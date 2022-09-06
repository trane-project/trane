//! Defines the basic data structures used by Trane to describe courses, lessons, and exercises,
//! store the results of a student's attempt at mastering an exercise, the options avaialble to
//! control the behavior of the scheduler, among other things.

pub mod filter;

use std::{collections::BTreeMap, path::Path};

use anyhow::Result;
use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use ustr::Ustr;

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

/// The result of a single trial.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExerciseTrial {
    /// The score assigned to the exercise after the trial.
    pub score: f32,

    /// The timestamp at which the trial happened.
    pub timestamp: i64,
}

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
        if self.range.1 >= 5.0 && score == 5.0 {
            // Handle the special case of the window containing the maximum score.
            return true;
        }
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

/// Trait to convert relative paths to absolute paths so that objects stored in memory contain the
/// full path to all their assets.
pub trait NormalizePaths
where
    Self: Sized,
{
    /// Converts all relative paths in the object to absolute paths.
    fn normalize_paths(&self, dir: &Path) -> Result<Self>;
}

/// Trait to verify that the paths in the object are valid.
pub trait VerifyPaths
where
    Self: Sized,
{
    /// Checks that all the paths mentioned in the object exist in disk.
    fn verify_paths(&self, dir: &Path) -> Result<bool>;
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
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum BasicAsset {
    /// An asset containing the path to a markdown file.
    MarkdownAsset {
        /// The path to the markdown file.
        path: String,
    },
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
    /// For example, `music::instrument::guitar::basic_jazz_chords`.
    #[builder(setter(into))]
    pub id: Ustr,

    /// The name of the course to be presented to the user.
    /// For example, "Basic Jazz Chords on Guitar".
    pub name: String,

    /// The IDs of all dependencies of this course.
    pub dependencies: Vec<Ustr>,

    /// An optional description of the course.
    #[builder(default)]
    pub description: Option<String>,

    /// An optional list of the course's authors.
    #[builder(default)]
    pub authors: Option<Vec<String>>,

    //// A mapping of String keys to a list of String values. For example, ("genre", ["jazz"]) could
    /// be attached to a course named "Basic Jazz Chords on Guitar".
    ///
    /// The purpose of this metadata is to allow students to focus on more specific material during
    /// a study session which does not belong to a single lesson or course. For example, a student
    /// might want to only focus on guitar scales or ear training.
    #[builder(default)]
    pub metadata: Option<BTreeMap<String, Vec<String>>>,

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
        match &self.course_instructions {
            None => Ok(true),
            Some(asset) => asset.verify_paths(dir),
        }?;
        match &self.course_material {
            None => Ok(true),
            Some(asset) => asset.verify_paths(dir),
        }
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
#[derive(Builder, Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LessonManifest {
    /// The ID assigned to this lesson. For example,
    /// `music::instrument::guitar::basic_jazz_chords::major_chords`.
    #[builder(setter(into))]
    pub id: Ustr,

    /// The IDs of all dependencies of this lesson.
    pub dependencies: Vec<Ustr>,

    /// The ID of the course to which the lesson belongs.
    #[builder(setter(into))]
    pub course_id: Ustr,

    /// The name of the lesson to be presented to the user. For example, "Basic Jazz Major Chords".
    pub name: String,

    /// An optional description of the lesson.
    #[builder(default)]
    pub description: Option<String>,

    //// A mapping of String keys to a list of String values. For example, ("key", ["C"]) could
    /// be attached to a lesson named "C Major Scale". The purpose is the same as the metadata
    /// stored in the course manifest but allows finer control over which lessons are selected.
    #[builder(default)]
    pub metadata: Option<BTreeMap<String, Vec<String>>>,

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
        let material_exists = match &self.lesson_material {
            None => true,
            Some(asset) => asset.verify_paths(dir)?,
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
    SoundSliceAsset {
        /// The link to the SoundSlice asset.
        link: String,

        /// An optional description of the exercise tied to this asset. For example, "Play this
        /// slice in the key of D Major" or "Practice measures 1 through 4". A missing description
        /// implies the entire slice should be practiced as is.
        description: Option<String>,
    },

    /// An asset representing a flashcard with a front and back each stored in a markdown file. The
    /// first file stores the front (question) of the flashcard while the second file stores the
    /// back (answer).
    FlashcardAsset {
        /// The path to the file containing the front of the flashcard.
        front_path: String,

        /// The path to the file containing the back of the flashcard.
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
    /// `music::instrument::guitar::basic_jazz_chords::major_chords::exercise_1`.
    #[builder(setter(into))]
    pub id: Ustr,

    /// The ID of the lesson to which this exercise belongs.
    #[builder(setter(into))]
    pub lesson_id: Ustr,

    /// The ID of the course to which this exercise belongs.
    #[builder(setter(into))]
    pub course_id: Ustr,

    /// The name of the exercise to be presented to the user. For example, "Exercise 1".
    pub name: String,

    /// An optional description of the exercise.
    #[builder(default)]
    pub description: Option<String>,

    /// The type of knowledge the exercise tests.
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
    pub passing_score: f32,

    /// The maximum number of scores to lookup in the practice stats.
    pub num_scores: usize,
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
                range: (2.5, 3.9),
            },
            easy_window_opts: MasteryWindow {
                percentage: 0.2,
                range: (3.9, 4.7),
            },
            mastered_window_opts: MasteryWindow {
                percentage: 0.1,
                range: (4.7, 5.0),
            },
            passing_score: 3.9,
            num_scores: 25,
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
                    back_path: "back.png".to_string(),
                })
                .build()
                .unwrap()
                .get_unit_type()
        );
    }

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
}
