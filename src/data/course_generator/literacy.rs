//! Defines a special course to teach literacy skills.
//!
//! The student is presented with examples and exceptions that match a certain spelling rule or type
//! of reading material. They are asked to read the example and exceptions and are scored based on
//! how many they get right. Optionally, a dictation lesson can be generated where the student is
//! asked to write the examples and exceptions based on the tutor's dictation.

use anyhow::{anyhow, Context, Error, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{read_dir, File},
    io::{BufReader, Read},
    path::Path,
};
use ts_rs::TS;
use ustr::{Ustr, UstrMap, UstrSet};

use crate::data::{
    BasicAsset, CourseGenerator, CourseManifest, ExerciseAsset, ExerciseManifest, ExerciseType,
    GenerateManifests, GeneratedCourse, LessonManifest, UserPreferences,
};

/// The metadata key indicating this is a literacy course. Its value should be set to "true".
pub const COURSE_METADATA: &str = "literacy_course";

/// The suffix used to recognize a directory as a knowledge base lesson.
pub const LESSON_SUFFIX: &str = ".lesson";

/// The name of the file containing the dependencies of a lesson.
pub const LESSON_DEPENDENCIES_FILE: &str = "lesson.dependencies.json";

/// The name of the file containing the name of a lesson.
pub const LESSON_NAME_FILE: &str = "lesson.name.json";

/// The name of the file containing the description of a lesson.
pub const LESSON_DESCRIPTION_FILE: &str = "lesson.description.json";

/// The name of the file containing the lesson instructions.
pub const LESSON_INSTRUCTIONS_FILE: &str = "lesson.instructions.md";

/// The name of the file containing the lesson material.
pub const LESSON_MATERIAL_FILE: &str = "lesson.material.md";

/// The metadata indicating the type of literacy lesson.
pub const LESSON_METADATA: &str = "literacy_lesson";

/// The extension of files containing examples.
pub const EXAMPLE_SUFFIX: &str = ".example.md";

/// The extension of files containing exceptions.
pub const EXCEPTION_SUFFIX: &str = ".exception.md";

/// The name of the file containing a list of examples.
pub const SIMPLE_EXAMPLES_FILE: &str = "simple_examples.md";

/// The name of the file containing a list of exceptions.
pub const SIMPLE_EXCEPTIONS_FILE: &str = "simple_exceptions.md";

/// An enum representing a type of files that can be found in a literacy lesson directory.
#[derive(Debug, Eq, PartialEq)]
pub enum LiteracyFile {
    /// The file containing the name of the lesson.
    LessonName,

    /// The file containing the description of the lesson.
    LessonDescription,

    /// The file containing the dependencies of the lesson.
    LessonDependencies,

    /// The file containing the lesson instructions.
    LessonInstructions,

    /// The file containing the front of the flashcard for the exercise with the given short ID.
    Example(String),

    /// The file containing the back of the flashcard for the exercise with the given short ID.
    Exception(String),

    /// The file containing one example per line.
    SimpleExamples,

    /// The file containing one exception per line.
    SimpleExceptions,
}

impl LiteracyFile {
    /// Opens the knowledge base file at the given path and deserializes its contents.
    pub fn open_serialized<T: DeserializeOwned>(path: &Path) -> Result<T> {
        let display = path.display();
        let file = File::open(path).context(format!("cannot open literacy file {display}"))?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader).context(format!("cannot parse literacy file {display}"))
    }

    // Opens a file that contains an example or exception stored as markdown.
    pub fn open_md(path: &Path) -> Result<String> {
        let display = path.display();
        let file = File::open(path).context(format!("cannot open literacy file {display}"))?;
        let mut reader = BufReader::new(file);
        let mut contents = String::new();
        reader
            .read_to_string(&mut contents)
            .context(format!("cannot read literacy markdown file {display}"))?;
        Ok(contents)
    }

    /// Opens a file that contains one example or exception per line.
    pub fn open_md_list(path: &Path) -> Result<Vec<String>> {
        let display = path.display();
        let file = File::open(path).context(format!("cannot open literacy file {display}"))?;
        let mut reader = BufReader::new(file);
        let mut contents = String::new();
        reader
            .read_to_string(&mut contents)
            .context(format!("cannot read literacy markdown file {display}"))?;
        Ok(contents.lines().map(ToString::to_string).collect())
    }
}

impl TryFrom<&str> for LiteracyFile {
    type Error = Error;

    /// Converts a file name to a `KnowledgeBaseFile` variant.
    fn try_from(file_name: &str) -> Result<Self> {
        match file_name {
            LESSON_DEPENDENCIES_FILE => Ok(LiteracyFile::LessonDependencies),
            LESSON_NAME_FILE => Ok(LiteracyFile::LessonName),
            LESSON_DESCRIPTION_FILE => Ok(LiteracyFile::LessonDescription),
            LESSON_INSTRUCTIONS_FILE => Ok(LiteracyFile::LessonInstructions),
            file_name if file_name.ends_with(EXAMPLE_SUFFIX) => {
                let short_id = file_name.strip_suffix(EXAMPLE_SUFFIX).unwrap();
                Ok(LiteracyFile::Example(short_id.to_string()))
            }
            file_name if file_name.ends_with(EXCEPTION_SUFFIX) => {
                let short_id = file_name.strip_suffix(EXCEPTION_SUFFIX).unwrap();
                Ok(LiteracyFile::Exception(short_id.to_string()))
            }
            SIMPLE_EXAMPLES_FILE => Ok(LiteracyFile::SimpleExamples),
            SIMPLE_EXCEPTIONS_FILE => Ok(LiteracyFile::SimpleExceptions),
            _ => Err(anyhow!("Not a valid literacy file name: {}", file_name)),
        }
    }
}

/// The types of literacy lessons that can be generated.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[ts(export)]
pub enum LiteracyLessonType {
    /// A lesson that takes examples and exceptions and asks the student to read them.
    Reading,

    /// A lesson that takes examples and exceptions and asks the student to write them based on the
    /// tutor's dictation.
    Dictation,
}

/// A representation of a literacy lesson containing examples and exceptions from which the raw
/// lesson and exercise manifests are generated.
///
/// In a literacy course, lessons are generated by searching for all directories with a name in the
/// format `<short_id>.lesson`. Examples are read from files with the suffix `.example.md`. The
/// optional exceptions are read from files with the suffix `.exception.md`.
///
/// Simple example and exceptions can be added by reading examples from the file
/// `simple_examples.md` and exceptions from the file `simple_exceptions.md`. Each line of these
/// files is treated as a separate example or exception.
///
/// Additional fields like the name and dependencies of the lesson can be set by creating a file
/// named `lesson.<PROPERTY_NAME>.json` in the lesson directory with the serialized value of the
/// property.
///
/// An instruction file can be created by creating a file named `lesson.instructions.md` in the
/// lesson directory.
pub struct LiteracyLesson {
    /// The short ID of the lesson, which is used to easily identify the lesson and to generate the
    /// final lesson ID.
    pub short_id: Ustr,

    /// The IDs of all dependencies of this lesson. The values can be full lesson IDs or the short
    /// ID of one of the other lessons in the course. If Trane finds a dependency with a short ID,
    /// it will automatically generate the full lesson ID. Not setting this value will indicate that
    /// the lesson has no dependencies.
    pub dependencies: Vec<Ustr>,

    /// The name of the lesson to be presented to the user.
    pub name: Option<String>,

    /// An optional description of the lesson.
    pub description: Option<String>,

    /// Optional instructions for the lesson.
    pub instructions: Option<BasicAsset>,

    /// The examples for the lesson.
    pub examples: Vec<String>,

    /// The exceptions for the lesson.
    pub exceptions: Vec<String>,
}

impl LiteracyLesson {
    /// Generates the lesson from a list of literacy files.
    fn create_lesson(
        lesson_root: &Path,
        short_lesson_id: Ustr,
        files: &[LiteracyFile],
    ) -> Result<Self> {
        // Create the lesson with all the optional fields set to a default value.
        let mut lesson = Self {
            short_id: short_lesson_id,
            dependencies: vec![],
            name: None,
            description: None,
            instructions: None,
            examples: vec![],
            exceptions: vec![],
        };

        // Iterate through the lesson files found in the lesson directory and update the
        // corresponding field in the lesson.
        for lesson_file in files {
            match lesson_file {
                LiteracyFile::LessonDependencies => {
                    let path = lesson_root.join(LESSON_DEPENDENCIES_FILE);
                    lesson.dependencies = LiteracyFile::open_serialized(&path)?;
                }
                LiteracyFile::LessonName => {
                    let path = lesson_root.join(LESSON_NAME_FILE);
                    lesson.name = Some(LiteracyFile::open_serialized(&path)?);
                }
                LiteracyFile::LessonDescription => {
                    let path = lesson_root.join(LESSON_DESCRIPTION_FILE);
                    lesson.description = Some(LiteracyFile::open_serialized(&path)?);
                }
                LiteracyFile::LessonInstructions => {
                    let path = lesson_root.join(LESSON_INSTRUCTIONS_FILE);
                    lesson.instructions = Some(BasicAsset::InlinedAsset {
                        content: LiteracyFile::open_md(&path)?,
                    });
                }
                LiteracyFile::Example(short_id) => {
                    let path = lesson_root.join(format!("{short_id}{EXAMPLE_SUFFIX}"));
                    let example = LiteracyFile::open_md(&path)?;
                    lesson.examples.push(example);
                }
                LiteracyFile::Exception(short_id) => {
                    let path = lesson_root.join(format!("{short_id}{EXCEPTION_SUFFIX}"));
                    let exception = LiteracyFile::open_md(&path)?;
                    lesson.exceptions.push(exception);
                }
                LiteracyFile::SimpleExamples => {
                    let path = lesson_root.join(SIMPLE_EXAMPLES_FILE);
                    let examples = LiteracyFile::open_md_list(&path)?;
                    lesson.examples.extend(examples);
                }
                LiteracyFile::SimpleExceptions => {
                    let path = lesson_root.join(SIMPLE_EXCEPTIONS_FILE);
                    let exceptions = LiteracyFile::open_md_list(&path)?;
                    lesson.exceptions.extend(exceptions);
                }
            }
        }

        // Examples and exceptions are sorted to have predictable outputs.
        lesson.examples.sort();
        lesson.exceptions.sort();

        // Make sure there is at least one example in the lesson.
        if lesson.examples.is_empty() {
            return Err(anyhow!("No examples found in the lesson"));
        }

        Ok(lesson)
    }

    /// Opens a literacy lesson from the given directory.
    fn open_lesson(lesson_root: &Path, short_lesson_id: Ustr) -> Result<Self> {
        // Iterate through the directory to find all the matching files in the lesson directory.
        let lesson_files = read_dir(lesson_root)?
            .flatten()
            .flat_map(|entry| {
                LiteracyFile::try_from(entry.file_name().to_str().unwrap_or_default())
            })
            .collect::<Vec<_>>();

        // TODO: figure out how to deal with empty short IDs.
        // // Remove exercises for the empty short ID. This can happen if the user has a file named
        // // `.front.md`, for example.
        // exercise_files.remove("");

        // Create the literacy lesson.
        Self::create_lesson(lesson_root, short_lesson_id, &lesson_files)
    }

    /// Detectes whether the given ID is one of the short IDs for one of the lesson of the course
    /// and returns the full ID of the reading lesson. Otherwise, it returns the ID as is.
    fn compute_full_reading_lesson_id(
        course_id: Ustr,
        short_id: Ustr,
        short_ids: &UstrSet,
    ) -> Ustr {
        if short_ids.contains(&short_id) {
            let full_id = format!("{course_id}::{short_id}::reading");
            full_id.into()
        } else {
            short_id
        }
    }

    /// Detects whether the given ID is one of the short IDs for one of the lesson of the course
    /// and returns the full ID of the dictation lesson. Otherwise, it returns the ID as is.k
    fn compute_full_dictation_lesson_id(
        course_id: Ustr,
        short_id: Ustr,
        short_ids: &UstrSet,
    ) -> Ustr {
        if short_ids.contains(&short_id) {
            let full_id = format!("{course_id}::{short_id}::dictation");
            full_id.into()
        } else {
            short_id
        }
    }

    /// Generates the manifests for the reading lesson.
    fn generate_reading_lesson(
        &self,
        course_manifest: &CourseManifest,
        short_id: Ustr,
        short_ids: &UstrSet,
    ) -> (LessonManifest, Vec<ExerciseManifest>) {
        // Create the lesson manifest.
        let lesson_id =
            Self::compute_full_reading_lesson_id(course_manifest.id, short_id, short_ids);
        let dependencies = self
            .dependencies
            .iter()
            .map(|id| Self::compute_full_reading_lesson_id(course_manifest.id, *id, short_ids))
            .collect::<Vec<_>>();
        let lesson_manifest = LessonManifest {
            id: lesson_id,
            dependencies,
            superseded: vec![],
            course_id: course_manifest.id,
            name: format!("{} - Reading", course_manifest.name),
            description: None,
            metadata: Some(BTreeMap::from([(
                LESSON_METADATA.to_string(),
                vec!["reading".to_string()],
            )])),
            lesson_instructions: self.instructions.clone(),
            lesson_material: None,
        };

        // Create the exercise manifest.
        let exercise_manifest = ExerciseManifest {
            id: format!("{lesson_id}::exercise").into(),
            lesson_id: lesson_manifest.id,
            course_id: course_manifest.id,
            name: format!("{} - Reading", course_manifest.name),
            description: None,
            exercise_type: ExerciseType::Procedural,
            exercise_asset: ExerciseAsset::LiteracyAsset {
                lesson_type: LiteracyLessonType::Reading,
                examples: self.examples.clone(),
                exceptions: self.exceptions.clone(),
            },
        };
        (lesson_manifest, vec![exercise_manifest])
    }

    /// Generates the manifests for the dictation lesson.
    fn generate_dictation_lesson(
        &self,
        course_manifest: &CourseManifest,
        short_id: Ustr,
        short_ids: &UstrSet,
    ) -> (LessonManifest, Vec<ExerciseManifest>) {
        // Create the lesson manifest.
        let lesson_id =
            Self::compute_full_dictation_lesson_id(course_manifest.id, short_id, short_ids);
        let reading_lesson_id =
            Self::compute_full_reading_lesson_id(course_manifest.id, short_id, short_ids);
        let lesson_manifest = LessonManifest {
            id: lesson_id,
            dependencies: vec![reading_lesson_id],
            superseded: vec![],
            course_id: course_manifest.id,
            name: format!("{} - Dictation", course_manifest.name),
            description: None,
            metadata: Some(BTreeMap::from([(
                LESSON_METADATA.to_string(),
                vec!["dictation".to_string()],
            )])),
            lesson_instructions: self.instructions.clone(),
            lesson_material: None,
        };

        // Create the exercise manifest.
        let exercise_manifest = ExerciseManifest {
            id: format!("{}::exercise", lesson_id).into(),
            lesson_id: lesson_manifest.id,
            course_id: course_manifest.id,
            name: format!("{} - Dictation", course_manifest.name),
            description: None,
            exercise_type: ExerciseType::Procedural,
            exercise_asset: ExerciseAsset::LiteracyAsset {
                lesson_type: LiteracyLessonType::Dictation,
                examples: self.examples.clone(),
                exceptions: self.exceptions.clone(),
            },
        };
        (lesson_manifest, vec![exercise_manifest])
    }

    /// Generates the manifests for the reading and dictation lessons.
    fn generate_manifests(
        &self,
        course_manifest: &CourseManifest,
        short_id: Ustr,
        short_ids: &UstrSet,
    ) -> Vec<(LessonManifest, Vec<ExerciseManifest>)> {
        let generate_dictation =
            if let Some(CourseGenerator::Literacy(config)) = &course_manifest.generator_config {
                config.generate_dictation
            } else {
                false
            };

        if generate_dictation {
            vec![
                self.generate_reading_lesson(course_manifest, short_id, short_ids),
                self.generate_dictation_lesson(course_manifest, short_id, short_ids),
            ]
        } else {
            vec![self.generate_reading_lesson(course_manifest, short_id, short_ids)]
        }
    }
}

/// The configuration to create a course that teaches literacy based on the provided material.
/// Material can be of two types.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct LiteracyConfig {
    /// Indicates whether to generate a lesson that asks the student to write the examples and
    /// exceptions based on the tutor's dictation.
    #[serde(default)]
    pub generate_dictation: bool,
}

impl GenerateManifests for LiteracyConfig {
    fn generate_manifests(
        &self,
        course_root: &Path,
        course_manifest: &CourseManifest,
        _preferences: &UserPreferences,
    ) -> Result<GeneratedCourse> {
        // Create the lessons by iterating through all the directories in the course root,
        // processing only those whose name fits the pattern `<SHORT_LESSON_ID>.lesson`.
        let mut lessons = UstrMap::default();
        let valid_entries = read_dir(course_root)?
            .flatten()
            .filter(|entry| {
                let path = entry.path();
                path.is_dir()
            })
            .collect::<Vec<_>>();
        for entry in valid_entries {
            // Check if the directory name is in the format `<SHORT_LESSON_ID>.lesson`. If so, read
            // the knowledge base lesson and its exercises.
            let path = entry.path();
            let dir_name = path.file_name().unwrap_or_default().to_str().unwrap();
            if let Some(short_id) = dir_name.strip_suffix(LESSON_SUFFIX) {
                lessons.insert(
                    short_id.into(),
                    LiteracyLesson::open_lesson(&path, short_id.into())?,
                );
            }
        }

        // Create the manifests.
        let short_ids: UstrSet = lessons.keys().copied().collect();
        let lessons: Vec<(LessonManifest, Vec<ExerciseManifest>)> = lessons
            .into_iter()
            .flat_map(|(short_id, lesson)| {
                lesson.generate_manifests(course_manifest, short_id, &short_ids)
            })
            .collect();
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
#[cfg_attr(coverage, coverage(off))]
mod test {
    use anyhow::Result;
    use pretty_assertions::assert_eq;
    use std::{collections::BTreeMap, fs, path::Path};

    use crate::data::{
        course_generator::literacy::{LiteracyConfig, LiteracyLessonType},
        CourseGenerator, CourseManifest, ExerciseAsset, ExerciseManifest, ExerciseType,
        GenerateManifests, GeneratedCourse, LessonManifest, UserPreferences,
    };

    /// Generates a set of test lessons, each with the given number of examples and exceptions.
    /// Each lesson will depend on the previous one to verify the generation of dependencies.
    fn generate_test_files(
        root_dir: &Path,
        num_lessons: u8,
        num_examples: u8,
        num_exceptions: u8,
        num_simple_examples: u8,
        num_simple_exceptions: u8,
    ) -> Result<()> {
        for i in 0..num_lessons {
            // Create the lesson directory and make lesson depend on the previous one.
            let lesson_dir = root_dir.join(format!("lesson_{i}.lesson"));
            fs::create_dir_all(&lesson_dir)?;
            if i > 0 {
                let dependencies_file = lesson_dir.join("lesson.dependencies.json");
                let dependencies_content = format!("[\"lesson_{}\"]", i - 1);
                fs::write(&dependencies_file, dependencies_content)?;
            }

            // Write individual example and exception files.
            for j in 0..num_examples {
                let example_file = lesson_dir.join(format!("example_{j}.example.md"));
                let example_content = format!("example_{j}");
                fs::write(&example_file, example_content)?;
            }
            for j in 0..num_exceptions {
                let exception_file = lesson_dir.join(format!("exception_{j}.exception.md"));
                let exception_content = format!("exception_{j}");
                fs::write(&exception_file, exception_content)?;
            }

            // If simple examples and exceptions are requested, generate the `simple_examples.md`
            // and `simple_exceptions.md` files.
            if num_simple_examples > 0 {
                let simple_example_file = lesson_dir.join("simple_examples.md");
                let simple_example_content = (0..num_simple_examples)
                    .map(|j| format!("simple_example_{j}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                fs::write(&simple_example_file, simple_example_content)?;
            }
            if num_simple_exceptions > 0 {
                let simple_exception_file = lesson_dir.join("simple_exceptions.md");
                let simple_exception_content = (0..num_simple_exceptions)
                    .map(|j| format!("simple_exception_{j}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                fs::write(&simple_exception_file, simple_exception_content)?;
            }
        }
        Ok(())
    }

    /// Verifies generating a literacy course with a dictation lesson.
    #[test]
    fn test_generate_manifests_dictation() -> Result<()> {
        // Create course manifest and files.
        let config = CourseGenerator::Literacy(LiteracyConfig {
            generate_dictation: true,
        });
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
            generator_config: Some(config.clone()),
        };
        let temp_dir = tempfile::tempdir()?;
        generate_test_files(temp_dir.path(), 2, 2, 2, 2, 2)?;

        // Generate the manifests. Sort lessons and exercises by ID to have predictable outputs.
        let prefs = UserPreferences::default();
        let mut got = config.generate_manifests(temp_dir.path(), &course_manifest, &prefs)?;
        got.lessons.sort_by(|a, b| a.0.id.cmp(&b.0.id));
        for (_, exercises) in &mut got.lessons {
            exercises.sort_by(|a, b| a.id.cmp(&b.id));
        }

        // Verify the generated course.
        let want = GeneratedCourse {
            lessons: vec![
                (
                    LessonManifest {
                        id: "literacy_course::lesson_0::dictation".into(),
                        dependencies: vec!["literacy_course::lesson_0::reading".into()],
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
                        id: "literacy_course::lesson_0::dictation::exercise".into(),
                        lesson_id: "literacy_course::lesson_0::dictation".into(),
                        course_id: "literacy_course".into(),
                        name: "Literacy Course - Dictation".into(),
                        description: None,
                        exercise_type: ExerciseType::Procedural,
                        exercise_asset: ExerciseAsset::LiteracyAsset {
                            lesson_type: LiteracyLessonType::Dictation,
                            examples: vec![
                                "example_0".to_string(),
                                "example_1".to_string(),
                                "simple_example_0".to_string(),
                                "simple_example_1".to_string(),
                            ],
                            exceptions: vec![
                                "exception_0".to_string(),
                                "exception_1".to_string(),
                                "simple_exception_0".to_string(),
                                "simple_exception_1".to_string(),
                            ],
                        },
                    }],
                ),
                (
                    LessonManifest {
                        id: "literacy_course::lesson_0::reading".into(),
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
                        id: "literacy_course::lesson_0::reading::exercise".into(),
                        lesson_id: "literacy_course::lesson_0::reading".into(),
                        course_id: "literacy_course".into(),
                        name: "Literacy Course - Reading".into(),
                        description: None,
                        exercise_type: ExerciseType::Procedural,
                        exercise_asset: ExerciseAsset::LiteracyAsset {
                            lesson_type: LiteracyLessonType::Reading,
                            examples: vec![
                                "example_0".to_string(),
                                "example_1".to_string(),
                                "simple_example_0".to_string(),
                                "simple_example_1".to_string(),
                            ],
                            exceptions: vec![
                                "exception_0".to_string(),
                                "exception_1".to_string(),
                                "simple_exception_0".to_string(),
                                "simple_exception_1".to_string(),
                            ],
                        },
                    }],
                ),
                (
                    LessonManifest {
                        id: "literacy_course::lesson_1::dictation".into(),
                        dependencies: vec!["literacy_course::lesson_1::reading".into()],
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
                        id: "literacy_course::lesson_1::dictation::exercise".into(),
                        lesson_id: "literacy_course::lesson_1::dictation".into(),
                        course_id: "literacy_course".into(),
                        name: "Literacy Course - Dictation".into(),
                        description: None,
                        exercise_type: ExerciseType::Procedural,
                        exercise_asset: ExerciseAsset::LiteracyAsset {
                            lesson_type: LiteracyLessonType::Dictation,
                            examples: vec![
                                "example_0".to_string(),
                                "example_1".to_string(),
                                "simple_example_0".to_string(),
                                "simple_example_1".to_string(),
                            ],
                            exceptions: vec![
                                "exception_0".to_string(),
                                "exception_1".to_string(),
                                "simple_exception_0".to_string(),
                                "simple_exception_1".to_string(),
                            ],
                        },
                    }],
                ),
                (
                    LessonManifest {
                        id: "literacy_course::lesson_1::reading".into(),
                        dependencies: vec!["literacy_course::lesson_0::reading".into()],
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
                        id: "literacy_course::lesson_1::reading::exercise".into(),
                        lesson_id: "literacy_course::lesson_1::reading".into(),
                        course_id: "literacy_course".into(),
                        name: "Literacy Course - Reading".into(),
                        description: None,
                        exercise_type: ExerciseType::Procedural,
                        exercise_asset: ExerciseAsset::LiteracyAsset {
                            lesson_type: LiteracyLessonType::Reading,
                            examples: vec![
                                "example_0".to_string(),
                                "example_1".to_string(),
                                "simple_example_0".to_string(),
                                "simple_example_1".to_string(),
                            ],
                            exceptions: vec![
                                "exception_0".to_string(),
                                "exception_1".to_string(),
                                "simple_exception_0".to_string(),
                                "simple_exception_1".to_string(),
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
        let config = CourseGenerator::Literacy(LiteracyConfig {
            generate_dictation: false,
        });
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
            generator_config: Some(config.clone()),
        };
        let temp_dir = tempfile::tempdir()?;
        generate_test_files(temp_dir.path(), 2, 2, 2, 2, 2)?;

        // Generate the manifests. Sort lessons and exercises by ID to have predictable outputs.
        let prefs = UserPreferences::default();
        let mut got = config.generate_manifests(temp_dir.path(), &course_manifest, &prefs)?;
        got.lessons.sort_by(|a, b| a.0.id.cmp(&b.0.id));
        for (_, exercises) in &mut got.lessons {
            exercises.sort_by(|a, b| a.id.cmp(&b.id));
        }

        // Verify the generated course.
        let want = GeneratedCourse {
            lessons: vec![
                (
                    LessonManifest {
                        id: "literacy_course::lesson_0::reading".into(),
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
                        id: "literacy_course::lesson_0::reading::exercise".into(),
                        lesson_id: "literacy_course::lesson_0::reading".into(),
                        course_id: "literacy_course".into(),
                        name: "Literacy Course - Reading".into(),
                        description: None,
                        exercise_type: ExerciseType::Procedural,
                        exercise_asset: ExerciseAsset::LiteracyAsset {
                            lesson_type: LiteracyLessonType::Reading,
                            examples: vec![
                                "example_0".to_string(),
                                "example_1".to_string(),
                                "simple_example_0".to_string(),
                                "simple_example_1".to_string(),
                            ],
                            exceptions: vec![
                                "exception_0".to_string(),
                                "exception_1".to_string(),
                                "simple_exception_0".to_string(),
                                "simple_exception_1".to_string(),
                            ],
                        },
                    }],
                ),
                (
                    LessonManifest {
                        id: "literacy_course::lesson_1::reading".into(),
                        dependencies: vec!["literacy_course::lesson_0::reading".into()],
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
                        id: "literacy_course::lesson_1::reading::exercise".into(),
                        lesson_id: "literacy_course::lesson_1::reading".into(),
                        course_id: "literacy_course".into(),
                        name: "Literacy Course - Reading".into(),
                        description: None,
                        exercise_type: ExerciseType::Procedural,
                        exercise_asset: ExerciseAsset::LiteracyAsset {
                            lesson_type: LiteracyLessonType::Reading,
                            examples: vec![
                                "example_0".to_string(),
                                "example_1".to_string(),
                                "simple_example_0".to_string(),
                                "simple_example_1".to_string(),
                            ],
                            exceptions: vec![
                                "exception_0".to_string(),
                                "exception_1".to_string(),
                                "simple_exception_0".to_string(),
                                "simple_exception_1".to_string(),
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
}
