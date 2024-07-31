use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};
use ts_rs::TS;
use ustr::Ustr;

use crate::data::{
    CourseGenerator, CourseManifest, ExerciseAsset, ExerciseManifest, ExerciseType,
    GenerateManifests, GeneratedCourse, LessonManifest, UserPreferences,
};

/// The metadata key indicating this is a literacy course. Its value should be set to "true".
pub const COURSE_METADATA: &str = "literacy_course";

/// The metadata indicating the type of literacy lesson.
pub const LESSON_METADATA: &str = "literacy_lesson";

/// The extension of files containing examples.
pub const EXAMPLE_SUFFIX: &str = ".example.md";

/// The extension of files containing exceptions.
pub const EXCEPTION_SUFFIX: &str = ".exception.md";

/// The types of literacy lessons that can be generated.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[ts(export)]
pub enum LiteracyLesson {
    /// A lesson that takes examples and exceptions and asks the student to read them.
    Reading,

    /// A lesson that takes examples and exceptions and asks the student to write them based on the
    /// tutor's dictation.
    Dictation,
}

/// The configuration to create a course that teaches literacy based on the provided material.
/// Material can be of two types.
///
/// 1. Examples. For example, they can be words that share the same spelling and pronunciation (e.g.
///    "cat", "bat", "hat"), sentences that share similar words, or sentences from the same book or
///    article (for more advanced courses).
/// 2. Exceptions. For example, they can be words that share the same spelling but have different
///    pronunciations (e.g. "cow" and "crow").
///
/// All examples and exceptions accept markdown syntax. Examples and exceptions can be declared in
/// the configuration or in separate files in the course's directory. Files that end with the
/// extensions ".examples.md" and ".exceptions.md" will be considered as examples and exceptions,
/// respectively.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct LiteracyConfig {
    /// The dependencies on other literacy courses. Specifying these dependencies here instead of
    /// the [CourseManifest] allows Trane to generate more fine-grained dependencies.
    #[serde(default)]
    #[ts(as = "Vec<String>")]
    pub literacy_dependencies: Vec<Ustr>,

    /// Inlined examples to use in the course.
    #[serde(default)]
    inlined_examples: Vec<String>,

    /// Inlined exceptions to use in the course.
    #[serde(default)]
    inlined_exceptions: Vec<String>,

    /// Whether to generate an optional lesson that asks the student to write the material based on
    /// the tutor's dictation.
    #[serde(default)]
    pub generate_dictation: bool,
}

impl LiteracyConfig {
    fn generate_reading_lesson(
        course_manifest: &CourseManifest,
        examples: &[String],
        exceptions: &[String],
    ) -> (LessonManifest, Vec<ExerciseManifest>) {
        // Create the lesson manifest.
        let lesson_manifest = LessonManifest {
            id: format!("{}::reading", course_manifest.id).into(),
            dependencies: vec![],
            superseded: vec![],
            course_id: course_manifest.id,
            name: format!("{} - Reading", course_manifest.name),
            description: None,
            metadata: Some(BTreeMap::from([(
                LESSON_METADATA.to_string(),
                vec!["reading".to_string()],
            )])),
            lesson_material: None,
            lesson_instructions: None,
        };

        // Create the exercise manifest.
        let exercise_manifest = ExerciseManifest {
            id: format!("{}::reading::exercise", course_manifest.id).into(),
            lesson_id: lesson_manifest.id,
            course_id: course_manifest.id,
            name: format!("{} - Reading", course_manifest.name),
            description: None,
            exercise_type: ExerciseType::Procedural,
            exercise_asset: ExerciseAsset::LiteracyAsset {
                lesson_type: LiteracyLesson::Reading,
                examples: examples.to_vec(),
                exceptions: exceptions.to_vec(),
            },
        };
        (lesson_manifest, vec![exercise_manifest])
    }

    fn generate_dictation_lesson(
        course_manifest: &CourseManifest,
        examples: &[String],
        exceptions: &[String],
    ) -> Option<(LessonManifest, Vec<ExerciseManifest>)> {
        // Exit early if the dictation lesson should not be generated.
        let generate_dictation =
            if let Some(CourseGenerator::Literacy(config)) = &course_manifest.generator_config {
                config.generate_dictation
            } else {
                false
            };
        if !generate_dictation {
            return None;
        }

        // Create the lesson manifest.
        let lesson_manifest = LessonManifest {
            id: format!("{}::dictation", course_manifest.id).into(),
            dependencies: vec![format!("{}::dictation", course_manifest.id).into()],
            superseded: vec![],
            course_id: course_manifest.id,
            name: format!("{} - Dictation", course_manifest.name),
            description: None,
            metadata: Some(BTreeMap::from([(
                LESSON_METADATA.to_string(),
                vec!["dictation".to_string()],
            )])),
            lesson_material: None,
            lesson_instructions: None,
        };

        // Create the exercise manifest.
        let exercise_manifest = ExerciseManifest {
            id: format!("{}::dictation::exercise", course_manifest.id).into(),
            lesson_id: lesson_manifest.id,
            course_id: course_manifest.id,
            name: format!("{} - Dictation", course_manifest.name),
            description: None,
            exercise_type: ExerciseType::Procedural,
            exercise_asset: ExerciseAsset::LiteracyAsset {
                lesson_type: LiteracyLesson::Reading,
                examples: examples.to_vec(),
                exceptions: exceptions.to_vec(),
            },
        };
        Some((lesson_manifest, vec![exercise_manifest]))
    }

    /// Generates the reading lesson and the optional dictation lesson.
    fn generate_lessons(
        course_manifest: &CourseManifest,
        examples: &[String],
        exceptions: &[String],
    ) -> Vec<(LessonManifest, Vec<ExerciseManifest>)> {
        if let Some(lesson) = Self::generate_dictation_lesson(course_manifest, examples, exceptions)
        {
            vec![
                Self::generate_reading_lesson(course_manifest, examples, exceptions),
                lesson,
            ]
        } else {
            vec![Self::generate_reading_lesson(
                course_manifest,
                examples,
                exceptions,
            )]
        }
    }
}

impl GenerateManifests for LiteracyConfig {
    fn generate_manifests(
        &self,
        course_root: &Path,
        course_manifest: &CourseManifest,
        _preferences: &UserPreferences,
    ) -> Result<GeneratedCourse> {
        // Collect all the examples and exceptions. First, gather the inlined ones. Then, gather the
        // examples and exceptions from the files in the courses's root directory.
        let mut examples = self.inlined_examples.clone();
        let mut exceptions = self.inlined_exceptions.clone();
        for entry in fs::read_dir(course_root)? {
            // Ignore entries that are not a file.
            let entry = entry.context("Failed to read entry when generating literacy course")?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            // Check that the file name ends is either an example or exception file.
            let file_name = path.file_name().unwrap_or_default().to_str().unwrap();
            if file_name.ends_with(EXAMPLE_SUFFIX) {
                let example = fs::read_to_string(&path).context("Failed to read example file")?;
                examples.push(example);
            } else if file_name.ends_with(EXCEPTION_SUFFIX) {
                let exception =
                    fs::read_to_string(&path).context("Failed to read exception file")?;
                exceptions.push(exception);
            }
        }

        // Generate the manifests for all the lessons and exercises and metadata to indicate this is
        // a literacy course.
        let lessons = Self::generate_lessons(course_manifest, &examples, &exceptions);
        let mut metadata = course_manifest.metadata.clone().unwrap_or_default();
        metadata.insert(COURSE_METADATA.to_string(), vec!["true".to_string()]);
        Ok(GeneratedCourse {
            lessons,
            updated_metadata: Some(metadata),
            updated_instructions: None,
        })
    }
}
