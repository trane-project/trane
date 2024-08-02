//! Defines a special course to teach literacy skills.
//!
//! The student is presented with examples and exceptions that match a certain spelling rule or type
//! of reading material. They are asked to read the example and exceptions and are scored based on
//! how many they get right. Optionally, a dictation lesson can be generated where the student is
//! asked to write the examples and exceptions based on the tutor's dictation.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};
use ts_rs::TS;

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
/// All examples and exceptions accept Markdown syntax. Examples and exceptions can be declared in
/// the configuration or in separate files in the course's directory. Files that end with the
/// extensions ".examples.md" and ".exceptions.md" will be considered as examples and exceptions,
/// respectively.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct LiteracyConfig {
    /// Inlined examples to use in the course.
    #[serde(default)]
    inlined_examples: Vec<String>,

    /// Inlined exceptions to use in the course.
    #[serde(default)]
    inlined_exceptions: Vec<String>,

    /// Indicates whether to generate an optional lesson that asks the student to write the material
    /// based on the tutor's dictation.
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
                false // grcov-excl-line
            };
        if !generate_dictation {
            return None;
        }

        // Create the lesson manifest.
        let lesson_manifest = LessonManifest {
            id: format!("{}::dictation", course_manifest.id).into(),
            dependencies: vec![format!("{}::reading", course_manifest.id).into()],
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

        // Sort the lists to have predictable outputs.
        examples.sort();
        exceptions.sort();

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

#[cfg(test)]
mod test {
    use anyhow::Result;
    use std::{collections::BTreeMap, fs, path::Path};

    use crate::data::{
        course_generator::literacy::{LiteracyConfig, LiteracyLesson},
        CourseGenerator, CourseManifest, ExerciseAsset, ExerciseManifest, ExerciseType,
        GenerateManifests, GeneratedCourse, LessonManifest, UserPreferences,
    };

    /// Writes the given number of example and exception files to the given directory.
    fn generate_test_files(root_dir: &Path, num_examples: u8, num_exceptions: u8) -> Result<()> {
        for i in 0..num_examples {
            let example_file = root_dir.join(format!("example_{i}.example.md"));
            let example_content = format!("example_{i}");
            fs::write(&example_file, example_content)?;
        }
        for i in 0..num_exceptions {
            let exception_file = root_dir.join(format!("exception_{i}.exception.md"));
            let exception_content = format!("exception_{i}");
            fs::write(&exception_file, exception_content)?;
        }
        Ok(())
    }

    /// Verifies generating a literacy course with a dictation lesson.
    #[test]
    fn test_generate_manifests_dictation() -> Result<()> {
        // Create course manifest and files.
        let config = LiteracyConfig {
            generate_dictation: true,
            inlined_examples: vec![
                "inlined_example_0".to_string(),
                "inlined_example_1".to_string(),
            ],
            inlined_exceptions: vec![
                "inlined_exception_0".to_string(),
                "inlined_exception_1".to_string(),
            ],
        };
        let course_manifest = CourseManifest {
            id: "literacy_course".into(),
            name: "Literacy Course".into(),
            dependencies: vec![],
            superseded: vec![],
            description: None,
            authors: None,
            metadata: None,
            course_material: None,
            course_instructions: None,
            generator_config: Some(CourseGenerator::Literacy(config.clone())),
        };
        let temp_dir = tempfile::tempdir()?;
        generate_test_files(temp_dir.path(), 2, 2)?;

        // Generate the manifests.
        let prefs = UserPreferences::default();
        let got = config.generate_manifests(temp_dir.path(), &course_manifest, &prefs)?;
        let want = GeneratedCourse {
            lessons: vec![
                (
                    LessonManifest {
                        id: "literacy_course::reading".into(),
                        dependencies: vec![],
                        superseded: vec![],
                        course_id: "literacy_course".into(),
                        name: "Literacy Course - Reading".into(),
                        description: None,
                        metadata: Some(BTreeMap::from([(
                            "literacy_lesson".to_string(),
                            vec!["reading".to_string()],
                        )])),
                        lesson_material: None,
                        lesson_instructions: None,
                    },
                    vec![ExerciseManifest {
                        id: "literacy_course::reading::exercise".into(),
                        lesson_id: "literacy_course::reading".into(),
                        course_id: "literacy_course".into(),
                        name: "Literacy Course - Reading".into(),
                        description: None,
                        exercise_type: ExerciseType::Procedural,
                        exercise_asset: ExerciseAsset::LiteracyAsset {
                            lesson_type: LiteracyLesson::Reading,
                            examples: vec![
                                "example_0".to_string(),
                                "example_1".to_string(),
                                "inlined_example_0".to_string(),
                                "inlined_example_1".to_string(),
                            ],
                            exceptions: vec![
                                "exception_0".to_string(),
                                "exception_1".to_string(),
                                "inlined_exception_0".to_string(),
                                "inlined_exception_1".to_string(),
                            ],
                        },
                    }],
                ),
                (
                    LessonManifest {
                        id: "literacy_course::dictation".into(),
                        dependencies: vec!["literacy_course::reading".into()],
                        superseded: vec![],
                        course_id: "literacy_course".into(),
                        name: "Literacy Course - Dictation".into(),
                        description: None,
                        metadata: Some(BTreeMap::from([(
                            "literacy_lesson".to_string(),
                            vec!["dictation".to_string()],
                        )])),
                        lesson_material: None,
                        lesson_instructions: None,
                    },
                    vec![ExerciseManifest {
                        id: "literacy_course::dictation::exercise".into(),
                        lesson_id: "literacy_course::dictation".into(),
                        course_id: "literacy_course".into(),
                        name: "Literacy Course - Dictation".into(),
                        description: None,
                        exercise_type: ExerciseType::Procedural,
                        exercise_asset: ExerciseAsset::LiteracyAsset {
                            lesson_type: LiteracyLesson::Reading,
                            examples: vec![
                                "example_0".to_string(),
                                "example_1".to_string(),
                                "inlined_example_0".to_string(),
                                "inlined_example_1".to_string(),
                            ],
                            exceptions: vec![
                                "exception_0".to_string(),
                                "exception_1".to_string(),
                                "inlined_exception_0".to_string(),
                                "inlined_exception_1".to_string(),
                            ],
                        },
                    }],
                ),
            ],
            updated_metadata: Some(BTreeMap::from([(
                "literacy_course".to_string(),
                vec!["true".to_string()],
            )])),
            updated_instructions: None,
        };
        assert_eq!(got, want);
        Ok(())
    }

    /// Verifies generating a literacy course with no dictation lesson.
    #[test]
    fn test_generate_manifests_no_dictation() -> Result<()> {
        // Create course manifest and files.
        let config = LiteracyConfig {
            generate_dictation: false,
            inlined_examples: vec![
                "inlined_example_0".to_string(),
                "inlined_example_1".to_string(),
            ],
            inlined_exceptions: vec![
                "inlined_exception_0".to_string(),
                "inlined_exception_1".to_string(),
            ],
        };
        let course_manifest = CourseManifest {
            id: "literacy_course".into(),
            name: "Literacy Course".into(),
            dependencies: vec![],
            superseded: vec![],
            description: None,
            authors: None,
            metadata: None,
            course_material: None,
            course_instructions: None,
            generator_config: Some(CourseGenerator::Literacy(config.clone())),
        };
        let temp_dir = tempfile::tempdir()?;
        generate_test_files(temp_dir.path(), 2, 2)?;

        // Generate the manifests.
        let prefs = UserPreferences::default();
        let got = config.generate_manifests(temp_dir.path(), &course_manifest, &prefs)?;
        let want = GeneratedCourse {
            lessons: vec![(
                LessonManifest {
                    id: "literacy_course::reading".into(),
                    dependencies: vec![],
                    superseded: vec![],
                    course_id: "literacy_course".into(),
                    name: "Literacy Course - Reading".into(),
                    description: None,
                    metadata: Some(BTreeMap::from([(
                        "literacy_lesson".to_string(),
                        vec!["reading".to_string()],
                    )])),
                    lesson_material: None,
                    lesson_instructions: None,
                },
                vec![ExerciseManifest {
                    id: "literacy_course::reading::exercise".into(),
                    lesson_id: "literacy_course::reading".into(),
                    course_id: "literacy_course".into(),
                    name: "Literacy Course - Reading".into(),
                    description: None,
                    exercise_type: ExerciseType::Procedural,
                    exercise_asset: ExerciseAsset::LiteracyAsset {
                        lesson_type: LiteracyLesson::Reading,
                        examples: vec![
                            "example_0".to_string(),
                            "example_1".to_string(),
                            "inlined_example_0".to_string(),
                            "inlined_example_1".to_string(),
                        ],
                        exceptions: vec![
                            "exception_0".to_string(),
                            "exception_1".to_string(),
                            "inlined_exception_0".to_string(),
                            "inlined_exception_1".to_string(),
                        ],
                    },
                }],
            )],
            updated_metadata: Some(BTreeMap::from([(
                "literacy_course".to_string(),
                vec!["true".to_string()],
            )])),
            updated_instructions: None,
        };
        assert_eq!(got, want);
        Ok(())
    }
}
