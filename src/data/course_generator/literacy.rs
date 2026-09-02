//! Defines a special course to teach literacy skills.
//!
//! The student is presented with examples and exceptions that match a certain spelling rule or type
//! of reading material. They are asked to read the example and exceptions and are scored based on
//! how many they get right. Optionally, a dictation lesson can be generated where the student is
//! asked to write the examples and exceptions based on the tutor's dictation.

use anyhow::{Context, Error, Result, anyhow};
use noyalib::compat::serde_yaml;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::BTreeMap;
use strum::Display;
use ustr::{Ustr, UstrMap, UstrSet};
use vfs::VfsPath;

use crate::data::{
    BasicAsset, CourseGenerator, CourseManifest, ExerciseAsset, ExerciseManifest, ExerciseType,
    GenerateManifests, GeneratedCourse, LessonManifest, UserPreferences,
};

/// The metadata key indicating this is a literacy course. Its value should be set to "true".
pub const COURSE_METADATA: &str = "literacy_course";

/// The name of the file containing the course instructions. Overrides the instructions in the
/// course manifest, so it should be the preferred way to set the instructions for a literacy
/// course.
pub const COURSE_INSTRUCTIONS_FILE: &str = "course.instructions.md";

/// The suffix used to recognize a directory as a knowledge base lesson.
pub const LESSON_SUFFIX: &str = ".lesson";

/// The name of the file containing the dependencies of a lesson.
pub const LESSON_DEPENDENCIES_FILE: &str = "lesson.dependencies.json";

/// The name of the file containing the courses or lessons encompassed by the lesson.
pub const LESSON_ENCOMPASSED_FILE: &str = "lesson.encompassed.json";

/// The name of the file containing the courses or lessons superseded by the lesson.
pub const LESSON_SUPERSEDED_FILE: &str = "lesson.superseded.json";

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

/// The extension of files containing the answer to an example.
pub const ANSWER_SUFFIX: &str = ".answer.md";

/// The extension of files containing the answer to an exception.
pub const EXCEPTION_ANSWER_SUFFIX: &str = ".exception_answer.md";

/// The name of the file containing a list of examples.
pub const SIMPLE_EXAMPLES_FILE: &str = "simple_examples.md";

/// The name of the file containing a list of exceptions.
pub const SIMPLE_EXCEPTIONS_FILE: &str = "simple_exceptions.md";

/// The name of the file containing a list of examples with optional answers.
pub const SIMPLE_EXAMPLES_WITH_ANSWERS_FILE: &str = "simple_examples_with_answers.yaml";

/// The name of the file containing a list of exceptions with optional answers.
pub const SIMPLE_EXCEPTIONS_WITH_ANSWERS_FILE: &str = "simple_exceptions_with_answers.yaml";

/// An enum representing a type of files that can be found in a literacy lesson directory.
#[derive(Debug, Eq, PartialEq)]
pub enum LiteracyFile {
    /// The file containing the course instructions.
    CourseInstructions,

    /// The file containing the name of the lesson.
    LessonName,

    /// The file containing the description of the lesson.
    LessonDescription,

    /// The file containing the dependencies of the lesson.
    LessonDependencies,

    /// The file containing the courses or lessons encompassed by the lesson.
    LessonEncompassed,

    /// The file containing the courses or lessons superseded by the lesson.
    LessonSuperseded,

    /// The file containing the lesson instructions.
    LessonInstructions,

    /// The file containing the front of the flashcard for the exercise with the given short ID.
    Example(String),

    /// The file containing the back of the flashcard for the exercise with the given short ID.
    Exception(String),

    /// The file containing the answer to the example with the given short ID.
    ExampleAnswer(String),

    /// The file containing the answer to the exception with the given short ID.
    ExceptionAnswer(String),

    /// The file containing one example per line.
    SimpleExamples,

    /// The file containing one exception per line.
    SimpleExceptions,

    /// The file containing a list of examples with optional answers.
    SimpleExamplesWithAnswers,

    /// The file containing a list of exceptions with optional answers.
    SimpleExceptionsWithAnswers,
}

impl LiteracyFile {
    /// Opens the knowledge base file at the given path and deserializes its contents.
    pub fn open_serialized<T: DeserializeOwned>(path: &VfsPath) -> Result<T> {
        let display = path.as_str();
        let file = path
            .open_file()
            .context(format!("cannot open literacy file {display}"))?;
        serde_json::from_reader(file).context(format!("cannot parse literacy file {display}"))
    }

    /// Opens a file that contains an example or exception stored as markdown.
    pub fn open_md(path: &VfsPath) -> Result<String> {
        let display = path.as_str();
        path.read_to_string()
            .context(format!("cannot read literacy markdown file {display}"))
    }

    /// Opens a file that contains one example or exception per line.
    pub fn open_md_list(path: &VfsPath) -> Result<Vec<String>> {
        let contents = Self::open_md(path)?;
        Ok(contents
            .lines()
            .map(ToString::to_string)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect())
    }

    /// Opens a YAML file containing a list of examples or exceptions with optional answers and
    /// deserializes its contents.
    pub fn open_yml_list<T: DeserializeOwned + 'static>(path: &VfsPath) -> Result<Vec<T>> {
        let display = path.as_str();
        let file = path
            .open_file()
            .context(format!("cannot open literacy yaml file {display}"))?;
        serde_yaml::from_reader(file).context(format!("cannot parse literacy yaml file {display}"))
    }
}

impl TryFrom<&str> for LiteracyFile {
    type Error = Error;

    /// Converts a file name to a `KnowledgeBaseFile` variant.
    fn try_from(file_name: &str) -> Result<Self> {
        match file_name {
            LESSON_DEPENDENCIES_FILE => Ok(LiteracyFile::LessonDependencies),
            LESSON_ENCOMPASSED_FILE => Ok(LiteracyFile::LessonEncompassed),
            LESSON_SUPERSEDED_FILE => Ok(LiteracyFile::LessonSuperseded),
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
            file_name if file_name.ends_with(ANSWER_SUFFIX) => {
                let short_id = file_name.strip_suffix(ANSWER_SUFFIX).unwrap();
                Ok(LiteracyFile::ExampleAnswer(short_id.to_string()))
            }
            file_name if file_name.ends_with(EXCEPTION_ANSWER_SUFFIX) => {
                let short_id = file_name.strip_suffix(EXCEPTION_ANSWER_SUFFIX).unwrap();
                Ok(LiteracyFile::ExceptionAnswer(short_id.to_string()))
            }
            SIMPLE_EXAMPLES_FILE => Ok(LiteracyFile::SimpleExamples),
            SIMPLE_EXCEPTIONS_FILE => Ok(LiteracyFile::SimpleExceptions),
            SIMPLE_EXAMPLES_WITH_ANSWERS_FILE => Ok(LiteracyFile::SimpleExamplesWithAnswers),
            SIMPLE_EXCEPTIONS_WITH_ANSWERS_FILE => Ok(LiteracyFile::SimpleExceptionsWithAnswers),
            _ => Err(anyhow!("Not a valid literacy file name: {file_name}")), // grcov-excl-line
        }
    }
}

/// The types of literacy lessons that can be generated.
#[derive(Clone, Debug, Deserialize, Display, PartialEq, Serialize)]
pub enum LiteracyLessonType {
    /// A lesson that takes examples and exceptions and asks the student to read them.
    Reading,

    /// A lesson that takes examples and exceptions and asks the student to write them based on the
    /// tutor's dictation.
    Dictation,
}

/// A single entry in a `simple_examples_with_answers.yaml` file.
#[derive(Debug, Deserialize)]
pub struct SimpleExamplesWithAnswersEntry {
    /// The example.
    pub example: String,

    /// The optional answer to the example, such as a translation into another script.
    #[serde(default)]
    pub answer: Option<String>,
}

/// A single entry in a `simple_exceptions_with_answers.yaml` file.
#[derive(Debug, Deserialize)]
pub struct SimpleExceptionsWithAnswersEntry {
    /// The exception.
    pub exception: String,

    /// The optional answer to the exception, such as a translation into another script.
    #[serde(default)]
    pub answer: Option<String>,
}

/// A representation of a literacy lesson containing examples and exceptions from which the raw
/// lesson and exercise manifests are generated.
///
/// In a literacy course, lessons are generated by searching for all directories with a name in the
/// format `<short_id>.lesson`. Examples are read from files with the suffix `.example.md`. The
/// optional exceptions are read from files with the suffix `.exception.md`. Examples and exceptions
/// can be paired with an optional answer, such as a translation into another script, by adding a
/// file with the suffix `.answer.md` for examples and `.exception_answer.md` for exceptions.
///
/// Simple examples and exceptions can be added by reading examples from the file
/// `simple_examples.md` and exceptions from the file `simple_exceptions.md`. Each line of these
/// files is treated as a separate example or exception. Examples and exceptions with an optional
/// answer can be added from the files `simple_examples_with_answers.yaml` and
/// `simple_exceptions_with_answers.yaml`. Each entry in these files contains the example or
/// exception and an optional answer.
///
/// Additional fields like the name, dependencies, and superseded lessons can be set by creating a
/// file named `lesson.<PROPERTY_NAME>.json` in the lesson directory with the serialized value of
/// the property.
///
/// An instruction file can be created by creating a file named `lesson.instructions.md` in the
/// lesson directory.
#[derive(Clone, Debug, PartialEq)]
pub struct LiteracyLesson {
    /// The short ID of the lesson, which is used to easily identify the lesson and to generate the
    /// final lesson ID.
    pub short_id: Ustr,

    /// The IDs of all dependencies of this lesson. The values can be full lesson IDs or the short
    /// ID of one of the other lessons in the course. If Trane finds a dependency with a short ID,
    /// it will automatically generate the full lesson ID. Not setting this value will indicate that
    /// the lesson has no dependencies.
    pub dependencies: Vec<Ustr>,

    /// The IDs of all courses or lessons encompassed by this lesson and their respective weights.
    pub encompassed: Vec<(Ustr, f32)>,

    /// The IDs of all courses or lessons superseded by this lesson. The values can be full lesson
    /// IDs or the short ID of one of the other lessons in the course.
    pub superseded: Vec<Ustr>,

    /// The name of the lesson to be presented to the user.
    pub name: Option<String>,

    /// An optional description of the lesson.
    pub description: Option<String>,

    /// Optional instructions for the lesson.
    pub instructions: Option<BasicAsset>,

    /// The examples for the lesson. Each example can optionally be paired with an answer, such as
    /// a translation into another script.
    pub examples: Vec<(String, Option<String>)>,

    /// The exceptions for the lesson. Each exception can optionally be paired with an answer, such
    /// as a translation into another script.
    pub exceptions: Vec<(String, Option<String>)>,
}

impl LiteracyLesson {
    /// Generates the lesson from a list of literacy files.
    fn create_lesson(
        lesson_root: &VfsPath,
        short_lesson_id: Ustr,
        files: &[LiteracyFile],
    ) -> Result<Self> {
        // Create the lesson with all the optional fields set to a default value.
        let mut lesson = Self {
            short_id: short_lesson_id,
            dependencies: vec![],
            encompassed: vec![],
            superseded: vec![],
            name: None,
            description: None,
            instructions: None,
            examples: vec![],
            exceptions: vec![],
        };

        // Collect the answers to the individual examples and exceptions. This is done before
        // reading the examples and exceptions themselves, because the answer files might be
        // processed in any order.
        let mut example_answers = BTreeMap::new();
        let mut exception_answers = BTreeMap::new();
        for lesson_file in files {
            match lesson_file {
                LiteracyFile::ExampleAnswer(short_id) => {
                    let path = lesson_root.join(format!("{short_id}{ANSWER_SUFFIX}"))?;
                    let answer = LiteracyFile::open_md(&path)?;
                    example_answers.insert(short_id.clone(), answer);
                }
                LiteracyFile::ExceptionAnswer(short_id) => {
                    let path = lesson_root.join(format!("{short_id}{EXCEPTION_ANSWER_SUFFIX}"))?;
                    let answer = LiteracyFile::open_md(&path)?;
                    exception_answers.insert(short_id.clone(), answer);
                }
                _ => {}
            }
        }

        // Iterate through the lesson files found in the lesson directory and update the
        // corresponding field in the lesson.
        for lesson_file in files {
            match lesson_file {
                // grcov-excl-start
                LiteracyFile::CourseInstructions => {
                    return Err(anyhow!(
                        "Found course instructions file in lesson directory: {}",
                        lesson_root.as_str()
                    ));
                }
                // grcov-excl-stop
                LiteracyFile::LessonDependencies => {
                    let path = lesson_root.join(LESSON_DEPENDENCIES_FILE)?;
                    lesson.dependencies = LiteracyFile::open_serialized(&path)?;
                }
                LiteracyFile::LessonEncompassed => {
                    let path = lesson_root.join(LESSON_ENCOMPASSED_FILE)?;
                    lesson.encompassed = LiteracyFile::open_serialized(&path)?;
                }
                LiteracyFile::LessonSuperseded => {
                    let path = lesson_root.join(LESSON_SUPERSEDED_FILE)?;
                    lesson.superseded = LiteracyFile::open_serialized(&path)?;
                }
                LiteracyFile::LessonName => {
                    let path = lesson_root.join(LESSON_NAME_FILE)?;
                    lesson.name = Some(LiteracyFile::open_serialized(&path)?);
                }
                LiteracyFile::LessonDescription => {
                    let path = lesson_root.join(LESSON_DESCRIPTION_FILE)?;
                    lesson.description = Some(LiteracyFile::open_serialized(&path)?);
                }
                LiteracyFile::LessonInstructions => {
                    let path = lesson_root.join(LESSON_INSTRUCTIONS_FILE)?;
                    lesson.instructions = Some(BasicAsset::InlinedAsset {
                        content: LiteracyFile::open_md(&path)?,
                    });
                }
                LiteracyFile::Example(short_id) => {
                    let path = lesson_root.join(format!("{short_id}{EXAMPLE_SUFFIX}"))?;
                    let example = LiteracyFile::open_md(&path)?;
                    let answer = example_answers.get(short_id).cloned();
                    lesson.examples.push((example, answer));
                }
                LiteracyFile::Exception(short_id) => {
                    let path = lesson_root.join(format!("{short_id}{EXCEPTION_SUFFIX}"))?;
                    let exception = LiteracyFile::open_md(&path)?;
                    let answer = exception_answers.get(short_id).cloned();
                    lesson.exceptions.push((exception, answer));
                }
                LiteracyFile::ExampleAnswer(_) | LiteracyFile::ExceptionAnswer(_) => {}
                LiteracyFile::SimpleExamples => {
                    let path = lesson_root.join(SIMPLE_EXAMPLES_FILE)?;
                    let examples = LiteracyFile::open_md_list(&path)?;
                    lesson
                        .examples
                        .extend(examples.into_iter().map(|example| (example, None)));
                }
                LiteracyFile::SimpleExceptions => {
                    let path = lesson_root.join(SIMPLE_EXCEPTIONS_FILE)?;
                    let exceptions = LiteracyFile::open_md_list(&path)?;
                    lesson
                        .exceptions
                        .extend(exceptions.into_iter().map(|exception| (exception, None)));
                }
                LiteracyFile::SimpleExamplesWithAnswers => {
                    let path = lesson_root.join(SIMPLE_EXAMPLES_WITH_ANSWERS_FILE)?;
                    let examples =
                        LiteracyFile::open_yml_list::<SimpleExamplesWithAnswersEntry>(&path)?;
                    lesson.examples.extend(
                        examples
                            .into_iter()
                            .map(|entry| (entry.example, entry.answer)),
                    );
                }
                LiteracyFile::SimpleExceptionsWithAnswers => {
                    let path = lesson_root.join(SIMPLE_EXCEPTIONS_WITH_ANSWERS_FILE)?;
                    let exceptions =
                        LiteracyFile::open_yml_list::<SimpleExceptionsWithAnswersEntry>(&path)?;
                    lesson.exceptions.extend(
                        exceptions
                            .into_iter()
                            .map(|entry| (entry.exception, entry.answer)),
                    );
                }
            }
        }

        // Examples and exceptions are sorted to have predictable outputs.
        lesson.examples.sort();
        lesson.exceptions.sort();
        Ok(lesson)
    }

    /// Opens a literacy lesson from the given directory.
    fn open_lesson(lesson_root: &VfsPath, short_lesson_id: Ustr) -> Result<Self> {
        // Iterate through the directory to find all the matching files in the lesson directory.
        let lesson_files = lesson_root
            .read_dir()?
            .flat_map(|entry| LiteracyFile::try_from(entry.filename().as_str()))
            .collect::<Vec<_>>();

        // Create the literacy lesson.
        Self::create_lesson(lesson_root, short_lesson_id, &lesson_files)
    }

    /// Detectes whether the given ID is one of the short IDs for one of the lesson of the course
    /// and returns the full ID of the reading lesson. Otherwise, it returns the ID as is.
    fn full_reading_lesson_id(course_id: Ustr, lesson_id: Ustr, short_ids: &UstrSet) -> Ustr {
        if short_ids.contains(&lesson_id) {
            let full_id = format!("{course_id}::{lesson_id}::reading");
            full_id.into()
        } else {
            lesson_id
        }
    }

    /// Detects whether the given ID is one of the short IDs for one of the lesson of the course
    /// and returns the full ID of the dictation lesson. Otherwise, it returns the ID as is.k
    fn full_dictation_lesson_id(course_id: Ustr, lesson_id: Ustr, short_ids: &UstrSet) -> Ustr {
        if short_ids.contains(&lesson_id) {
            let full_id = format!("{course_id}::{lesson_id}::dictation");
            full_id.into()
        } else {
            lesson_id
        }
    }

    // Returns the name of the course, returning the ID if the name is empty.
    fn course_name(course_manifest: &CourseManifest) -> String {
        if course_manifest.name.is_empty() {
            course_manifest.id.to_string()
        } else {
            course_manifest.name.clone()
        }
    }

    // Retuns the name of the lesson, returning a sane default if the name is empty.
    fn lesson_name(&self, course_name: &str, lesson_type: &LiteracyLessonType) -> String {
        let lesson_type = match lesson_type {
            LiteracyLessonType::Reading => "Reading",
            LiteracyLessonType::Dictation => "Dictation",
        };
        if let Some(name) = &self.name {
            format!("{course_name} - {name} - {lesson_type}")
        } else {
            format!("{course_name} - {} - {lesson_type}", self.short_id)
        }
    }

    /// Generates the manifests for the reading lesson.
    fn generate_reading_lesson(
        &self,
        course_manifest: &CourseManifest,
        short_id: Ustr,
        short_ids: &UstrSet,
        exercise_type: &ExerciseType,
    ) -> (LessonManifest, Vec<ExerciseManifest>) {
        // Generate basic info for the lesson.
        let lesson_id = Self::full_reading_lesson_id(course_manifest.id, short_id, short_ids);
        let course_name = Self::course_name(course_manifest);
        let lesson_name = self.lesson_name(&course_name, &LiteracyLessonType::Reading);
        let mut dependencies = self
            .dependencies
            .iter()
            .map(|id| Self::full_reading_lesson_id(course_manifest.id, *id, short_ids))
            .collect::<Vec<_>>();
        dependencies.sort();
        let mut encompassed = self
            .encompassed
            .iter()
            .map(|(id, weight)| {
                (
                    Self::full_reading_lesson_id(course_manifest.id, *id, short_ids),
                    *weight,
                )
            })
            .collect::<Vec<_>>();
        encompassed.sort_by_key(|(id, _)| *id);
        let mut superseded = self
            .superseded
            .iter()
            .map(|id| Self::full_reading_lesson_id(course_manifest.id, *id, short_ids))
            .collect::<Vec<_>>();
        superseded.sort();

        // Create the lesson manifest.
        let lesson_manifest = LessonManifest {
            id: lesson_id,
            dependencies,
            encompassed,
            superseded,
            course_id: course_manifest.id,
            name: lesson_name.clone(),
            description: self.description.clone(),
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
            name: lesson_name,
            description: self.description.clone(),
            exercise_type: exercise_type.clone(),
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
        exercise_type: &ExerciseType,
    ) -> (LessonManifest, Vec<ExerciseManifest>) {
        // Generate basic info for the lesson. The dependencies are the dictation lessons of the
        // other lessons in the course that are marked as a dependency of this lesson. Exclude
        // dependencies outside the course. The reading lesson is always a dependency of the
        // dictation lesson.
        let lesson_id = Self::full_dictation_lesson_id(course_manifest.id, short_id, short_ids);
        let course_name = Self::course_name(course_manifest);
        let lesson_name = self.lesson_name(&course_name, &LiteracyLessonType::Dictation);
        let reading_lesson_id =
            Self::full_reading_lesson_id(course_manifest.id, short_id, short_ids);
        let mut dependencies = self
            .dependencies
            .iter()
            .filter_map(|id| {
                let full_dependency =
                    Self::full_dictation_lesson_id(course_manifest.id, *id, short_ids);
                if full_dependency == *id {
                    None
                } else {
                    Some(full_dependency)
                }
            })
            .collect::<Vec<_>>();
        dependencies.push(reading_lesson_id);
        dependencies.sort();
        let mut encompassed = self
            .encompassed
            .iter()
            .map(|(id, weight)| {
                (
                    Self::full_dictation_lesson_id(course_manifest.id, *id, short_ids),
                    *weight,
                )
            })
            .collect::<Vec<_>>();
        encompassed.sort_by_key(|(id, _)| *id);
        let mut superseded = self
            .superseded
            .iter()
            .map(|id| Self::full_dictation_lesson_id(course_manifest.id, *id, short_ids))
            .collect::<Vec<_>>();
        superseded.sort();

        // Create the lesson manifest.
        let lesson_manifest = LessonManifest {
            id: lesson_id,
            dependencies,
            encompassed,
            superseded,
            course_id: course_manifest.id,
            name: lesson_name.clone(),
            description: self.description.clone(),
            metadata: Some(BTreeMap::from([(
                LESSON_METADATA.to_string(),
                vec!["dictation".to_string()],
            )])),
            lesson_instructions: self.instructions.clone(),
            lesson_material: None,
        };

        // Create the exercise manifest.
        let exercise_manifest = ExerciseManifest {
            id: format!("{lesson_id}::exercise").into(),
            lesson_id: lesson_manifest.id,
            course_id: course_manifest.id,
            name: lesson_name,
            description: self.description.clone(),
            exercise_type: exercise_type.clone(),
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
        exercise_type: &ExerciseType,
    ) -> Vec<(LessonManifest, Vec<ExerciseManifest>)> {
        let mut generate_dictation = false;
        if let Some(CourseGenerator::Literacy(config)) = &course_manifest.generator_config {
            generate_dictation = config.generate_dictation;
        }

        if generate_dictation {
            vec![
                self.generate_reading_lesson(course_manifest, short_id, short_ids, exercise_type),
                self.generate_dictation_lesson(course_manifest, short_id, short_ids, exercise_type),
            ]
        } else {
            vec![self.generate_reading_lesson(course_manifest, short_id, short_ids, exercise_type)]
        }
    }
}

/// The configuration to create a course that teaches literacy based on the provided material.
/// Material can be of two types.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct LiteracyConfig {
    /// Indicates whether to generate a lesson that asks the student to write the examples and
    /// exceptions based on the tutor's dictation.
    #[serde(default)]
    pub generate_dictation: bool,

    /// The type of the generated exercises.
    #[serde(default)]
    pub exercise_type: ExerciseType,
}

impl LiteracyConfig {
    // Opens the course instructions if they exist.
    fn open_course_instructions(course_root: &VfsPath) -> Result<Option<BasicAsset>> {
        let path = course_root.join(COURSE_INSTRUCTIONS_FILE)?;
        if path.is_file()? {
            Ok(Some(BasicAsset::InlinedAsset {
                content: LiteracyFile::open_md(&path)?,
            }))
        } else {
            Ok(None) // grcov-excl-line
        }
    }
}

impl GenerateManifests for LiteracyConfig {
    fn generate_manifests(
        &self,
        course_root: &VfsPath,
        course_manifest: &CourseManifest,
        _preferences: &UserPreferences,
    ) -> Result<GeneratedCourse> {
        // Create the lessons by iterating through all the directories in the course root,
        // processing only those whose name fits the pattern `<SHORT_LESSON_ID>.lesson`.
        let mut lessons = UstrMap::default();
        let valid_entries = course_root
            .read_dir()?
            .filter(|path| path.is_dir().unwrap_or(false))
            .collect::<Vec<_>>();
        for path in valid_entries {
            // Check if the directory name is in the format `<SHORT_LESSON_ID>.lesson`. If so, read
            // the knowledge base lesson and its exercises.
            let dir_name = path.filename();
            if let Some(short_id) = dir_name.strip_suffix(LESSON_SUFFIX)
                && !short_id.is_empty()
            {
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
                lesson.generate_manifests(
                    course_manifest,
                    short_id,
                    &short_ids,
                    &self.exercise_type,
                )
            })
            .collect();
        let mut metadata = course_manifest.metadata.clone().unwrap_or_default();
        metadata.insert(COURSE_METADATA.to_string(), vec!["true".to_string()]);
        Ok(GeneratedCourse {
            lessons,
            updated_metadata: Some(metadata),
            updated_instructions: Self::open_course_instructions(course_root)?,
        })
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use anyhow::Result;
    use pretty_assertions::assert_eq;
    use std::{collections::BTreeMap, fs, path::Path};
    use ustr::{Ustr, UstrSet};

    use crate::data::{
        BasicAsset, CourseGenerator, CourseManifest, ExerciseAsset, ExerciseManifest, ExerciseType,
        GenerateManifests, GeneratedCourse, LessonManifest, UserPreferences,
        course_generator::literacy::{LiteracyConfig, LiteracyLesson, LiteracyLessonType},
    };
    use vfs::VfsPath;

    fn vfs_path(path: &Path) -> VfsPath {
        VfsPath::new(vfs::PhysicalFS::new(path))
    }

    /// Verifies that lesson IDs are generated correctly.
    #[test]
    fn full_lesson_ids() {
        let course_id = Ustr::from("course_id");
        let short_id = Ustr::from("lesson_id");
        let not_in_short_ids = "other_course_id::other_lesson_id".into();
        let short_ids: UstrSet = vec!["lesson_id".into()].into_iter().collect();

        // Reading lesson is one of the short IDs.
        let reading_lesson_id =
            LiteracyLesson::full_reading_lesson_id(course_id, short_id, &short_ids);
        assert_eq!(
            reading_lesson_id,
            Ustr::from("course_id::lesson_id::reading"),
        );

        // Reading lesson is not one of the short IDs.
        let reading_lesson_id =
            LiteracyLesson::full_reading_lesson_id(course_id, not_in_short_ids, &short_ids);
        assert_eq!(
            reading_lesson_id,
            Ustr::from("other_course_id::other_lesson_id")
        );

        // Dictation lesson is one of the short IDs.
        let dictation_lesson_id =
            LiteracyLesson::full_dictation_lesson_id(course_id, short_id, &short_ids);
        assert_eq!(
            dictation_lesson_id,
            Ustr::from("course_id::lesson_id::dictation"),
        );

        // Dictation lesson is not one of the short IDs.
        let dictation_lesson_id =
            LiteracyLesson::full_dictation_lesson_id(course_id, not_in_short_ids, &short_ids);
        assert_eq!(
            dictation_lesson_id,
            Ustr::from("other_course_id::other_lesson_id")
        );
    }

    /// Verifies creating the course name.
    #[test]
    fn course_name() {
        // Manifest with a name.
        let course_manifest = CourseManifest {
            id: "course_id".into(),
            name: "Course Name".into(),
            dependencies: vec![],
            encompassed: vec![],
            superseded: vec![],
            description: None,
            authors: None,
            metadata: None,
            course_material: None,
            course_instructions: None,
            generator_config: None,
        };
        assert_eq!(LiteracyLesson::course_name(&course_manifest), "Course Name");

        // Manifest with an empty name.
        let course_manifest = CourseManifest {
            id: "course_id".into(),
            name: String::new(),
            dependencies: vec![],
            encompassed: vec![],
            superseded: vec![],
            description: None,
            authors: None,
            metadata: None,
            course_material: None,
            course_instructions: None,
            generator_config: None,
        };
        assert_eq!(LiteracyLesson::course_name(&course_manifest), "course_id");
    }

    /// Verifies creating the lesson name.
    #[test]
    fn lesson_name() {
        // Lesson with a name.
        let lesson = LiteracyLesson {
            short_id: Ustr::from("lesson_id"),
            dependencies: vec![],
            encompassed: vec![],
            superseded: vec![],
            name: Some("Lesson Name".to_string()),
            description: None,
            instructions: None,
            examples: vec![],
            exceptions: vec![],
        };
        assert_eq!(
            lesson.lesson_name("Course Name", &LiteracyLessonType::Reading),
            "Course Name - Lesson Name - Reading"
        );

        // Lesson without a name.
        let lesson = LiteracyLesson {
            short_id: Ustr::from("lesson_id"),
            dependencies: vec![],
            encompassed: vec![],
            superseded: vec![],
            name: None,
            description: None,
            instructions: None,
            examples: vec![],
            exceptions: vec![],
        };
        assert_eq!(
            lesson.lesson_name("Course Name", &LiteracyLessonType::Reading),
            "Course Name - lesson_id - Reading"
        );
    }

    /// Verifies creating a literacy lesson from a directory with all possible files.
    #[test]
    fn open_lesson() -> Result<()> {
        // Create a temporary directory for the test.
        let temp_dir = tempfile::tempdir()?;
        let lesson_dir = temp_dir.path().join("lesson_0.lesson");
        fs::create_dir_all(&lesson_dir)?;

        // Create the files in the lesson directory.
        fs::write(
            lesson_dir.join("lesson.dependencies.json"),
            "[\"other_course\"]",
        )?;
        fs::write(
            lesson_dir.join("lesson.encompassed.json"),
            "[[\"other_course\", 0.5]]",
        )?;
        fs::write(
            lesson_dir.join("lesson.superseded.json"),
            "[\"superseded_course\"]",
        )?;
        fs::write(lesson_dir.join("lesson.name.json"), "\"Lesson 0\"")?;
        fs::write(
            lesson_dir.join("lesson.description.json"),
            "\"Description\"",
        )?;
        fs::write(lesson_dir.join("lesson.instructions.md"), "Instructions")?;
        fs::write(lesson_dir.join("example_0.example.md"), "Example 0")?;
        fs::write(lesson_dir.join("example_1.example.md"), "Example 1")?;
        fs::write(lesson_dir.join("example_0.answer.md"), "Answer 0")?;
        fs::write(lesson_dir.join("exception_0.exception.md"), "Exception 0")?;
        fs::write(lesson_dir.join("exception_1.exception.md"), "Exception 1")?;
        fs::write(
            lesson_dir.join("exception_0.exception_answer.md"),
            "Exception Answer 0",
        )?;
        fs::write(
            lesson_dir.join("simple_examples.md"),
            "Simple Example 0\nSimple Example 1",
        )?;
        fs::write(
            lesson_dir.join("simple_exceptions.md"),
            "Simple Exception 0\nSimple Exception 1",
        )?;
        fs::write(
            lesson_dir.join("simple_examples_with_answers.yaml"),
            "- example: Yaml Example 0\n  answer: Yaml Answer 0\n- example: Yaml Example 1",
        )?;
        fs::write(
            lesson_dir.join("simple_exceptions_with_answers.yaml"),
            "- exception: Yaml Exception 0\n  answer: Yaml Answer 0\n- exception: Yaml Exception 1",
        )?;

        // Open the lesson and verify its contents.
        let lesson = LiteracyLesson::open_lesson(&vfs_path(&lesson_dir), Ustr::from("lesson_0"))?;
        let want = LiteracyLesson {
            short_id: Ustr::from("lesson_0"),
            dependencies: vec![Ustr::from("other_course")],
            encompassed: vec![(Ustr::from("other_course"), 0.5)],
            superseded: vec![Ustr::from("superseded_course")],
            name: Some("Lesson 0".to_string()),
            description: Some("Description".to_string()),
            instructions: Some(BasicAsset::InlinedAsset {
                content: "Instructions".to_string(),
            }),
            examples: vec![
                ("Example 0".to_string(), Some("Answer 0".to_string())),
                ("Example 1".to_string(), None),
                ("Simple Example 0".to_string(), None),
                ("Simple Example 1".to_string(), None),
                (
                    "Yaml Example 0".to_string(),
                    Some("Yaml Answer 0".to_string()),
                ),
                ("Yaml Example 1".to_string(), None),
            ],
            exceptions: vec![
                (
                    "Exception 0".to_string(),
                    Some("Exception Answer 0".to_string()),
                ),
                ("Exception 1".to_string(), None),
                ("Simple Exception 0".to_string(), None),
                ("Simple Exception 1".to_string(), None),
                (
                    "Yaml Exception 0".to_string(),
                    Some("Yaml Answer 0".to_string()),
                ),
                ("Yaml Exception 1".to_string(), None),
            ],
        };
        assert_eq!(lesson, want);
        Ok(())
    }

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
        // Generate the course instructions.
        let course_instructions_file = root_dir.join("course.instructions.md");
        fs::write(&course_instructions_file, "# Course Instructions")?;

        // Generate the lessons.
        for i in 0..num_lessons {
            // Create the lesson directory and make lesson depend on the previous one. Add another
            // dependency that is outside the course to verify that functionality.
            let lesson_dir = root_dir.join(format!("lesson_{i}.lesson"));
            fs::create_dir_all(&lesson_dir)?;
            if i == 0 {
                let dependencies_file = lesson_dir.join("lesson.dependencies.json");
                let dependencies_content = "[\"other_lesson\"]";
                fs::write(&dependencies_file, dependencies_content)?;
            } else {
                let dependencies_file = lesson_dir.join("lesson.dependencies.json");
                let dependencies_content = format!("[\"lesson_{}\", \"other_lesson\"]", i - 1);
                fs::write(&dependencies_file, dependencies_content)?;
            }

            // Write the encompassed file. It includes a reference to a lesson in the course to
            // verify the short ID resolution and a reference to a lesson outside the course to
            // verify that external references are preserved.
            let encompassed_file = lesson_dir.join("lesson.encompassed.json");
            let encompassed_content = "[[\"lesson_0\", 1.0], [\"other_lesson\", 0.5]]";
            fs::write(&encompassed_file, encompassed_content)?;

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

            // If simple examples and exceptions with answers are requested, generate the
            // `simple_examples_with_answers.yaml` and `simple_exceptions_with_answers.yaml` files.
            if num_simple_examples > 0 {
                let simple_example_file = lesson_dir.join("simple_examples_with_answers.yaml");
                let simple_example_content = (0..num_simple_examples)
                    .map(|j| {
                        format!(
                            "- example: simple_example_with_answer_{j}\n  answer: simple_answer_{j}"
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                fs::write(&simple_example_file, simple_example_content)?;
            }
            if num_simple_exceptions > 0 {
                let simple_exception_file = lesson_dir.join("simple_exceptions_with_answers.yaml");
                let simple_exception_content = (0..num_simple_exceptions)
                    .map(|j| {
                        format!(
                            "- exception: simple_exception_with_answer_{j}\n  answer: simple_answer_{j}"
                        )
                    })
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
            exercise_type: ExerciseType::Procedural,
        });
        let course_manifest = CourseManifest {
            id: "literacy_course".into(),
            name: "Literacy Course".into(),
            dependencies: vec![],
            encompassed: vec![],
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
        let mut got =
            config.generate_manifests(&vfs_path(temp_dir.path()), &course_manifest, &prefs)?;
        got.lessons.sort_by_key(|lesson| lesson.0.id);
        for (_, exercises) in &mut got.lessons {
            exercises.sort_by_key(|exercise| exercise.id);
        }

        // Verify the generated course.
        let want = GeneratedCourse {
            lessons: vec![
                (
                    LessonManifest {
                        id: "literacy_course::lesson_0::dictation".into(),
                        dependencies: vec!["literacy_course::lesson_0::reading".into()],
                        encompassed: vec![
                            (Ustr::from("literacy_course::lesson_0::dictation"), 1.0),
                            (Ustr::from("other_lesson"), 0.5),
                        ],
                        superseded: vec![],
                        course_id: "literacy_course".into(),
                        name: "Literacy Course - lesson_0 - Dictation".into(),
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
                        name: "Literacy Course - lesson_0 - Dictation".into(),
                        description: None,
                        exercise_type: ExerciseType::Procedural,
                        exercise_asset: ExerciseAsset::LiteracyAsset {
                            lesson_type: LiteracyLessonType::Dictation,
                            examples: vec![
                                ("example_0".to_string(), None),
                                ("example_1".to_string(), None),
                                ("simple_example_0".to_string(), None),
                                ("simple_example_1".to_string(), None),
                                (
                                    "simple_example_with_answer_0".to_string(),
                                    Some("simple_answer_0".to_string()),
                                ),
                                (
                                    "simple_example_with_answer_1".to_string(),
                                    Some("simple_answer_1".to_string()),
                                ),
                            ],
                            exceptions: vec![
                                ("exception_0".to_string(), None),
                                ("exception_1".to_string(), None),
                                ("simple_exception_0".to_string(), None),
                                ("simple_exception_1".to_string(), None),
                                (
                                    "simple_exception_with_answer_0".to_string(),
                                    Some("simple_answer_0".to_string()),
                                ),
                                (
                                    "simple_exception_with_answer_1".to_string(),
                                    Some("simple_answer_1".to_string()),
                                ),
                            ],
                        },
                    }],
                ),
                (
                    LessonManifest {
                        id: "literacy_course::lesson_0::reading".into(),
                        dependencies: vec!["other_lesson".into()],
                        encompassed: vec![
                            (Ustr::from("literacy_course::lesson_0::reading"), 1.0),
                            (Ustr::from("other_lesson"), 0.5),
                        ],
                        superseded: vec![],
                        course_id: "literacy_course".into(),
                        name: "Literacy Course - lesson_0 - Reading".into(),
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
                        name: "Literacy Course - lesson_0 - Reading".into(),
                        description: None,
                        exercise_type: ExerciseType::Procedural,
                        exercise_asset: ExerciseAsset::LiteracyAsset {
                            lesson_type: LiteracyLessonType::Reading,
                            examples: vec![
                                ("example_0".to_string(), None),
                                ("example_1".to_string(), None),
                                ("simple_example_0".to_string(), None),
                                ("simple_example_1".to_string(), None),
                                (
                                    "simple_example_with_answer_0".to_string(),
                                    Some("simple_answer_0".to_string()),
                                ),
                                (
                                    "simple_example_with_answer_1".to_string(),
                                    Some("simple_answer_1".to_string()),
                                ),
                            ],
                            exceptions: vec![
                                ("exception_0".to_string(), None),
                                ("exception_1".to_string(), None),
                                ("simple_exception_0".to_string(), None),
                                ("simple_exception_1".to_string(), None),
                                (
                                    "simple_exception_with_answer_0".to_string(),
                                    Some("simple_answer_0".to_string()),
                                ),
                                (
                                    "simple_exception_with_answer_1".to_string(),
                                    Some("simple_answer_1".to_string()),
                                ),
                            ],
                        },
                    }],
                ),
                (
                    LessonManifest {
                        id: "literacy_course::lesson_1::dictation".into(),
                        dependencies: vec![
                            "literacy_course::lesson_0::dictation".into(),
                            "literacy_course::lesson_1::reading".into(),
                        ],
                        encompassed: vec![
                            (Ustr::from("literacy_course::lesson_0::dictation"), 1.0),
                            (Ustr::from("other_lesson"), 0.5),
                        ],
                        superseded: vec![],
                        course_id: "literacy_course".into(),
                        name: "Literacy Course - lesson_1 - Dictation".into(),
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
                        name: "Literacy Course - lesson_1 - Dictation".into(),
                        description: None,
                        exercise_type: ExerciseType::Procedural,
                        exercise_asset: ExerciseAsset::LiteracyAsset {
                            lesson_type: LiteracyLessonType::Dictation,
                            examples: vec![
                                ("example_0".to_string(), None),
                                ("example_1".to_string(), None),
                                ("simple_example_0".to_string(), None),
                                ("simple_example_1".to_string(), None),
                                (
                                    "simple_example_with_answer_0".to_string(),
                                    Some("simple_answer_0".to_string()),
                                ),
                                (
                                    "simple_example_with_answer_1".to_string(),
                                    Some("simple_answer_1".to_string()),
                                ),
                            ],
                            exceptions: vec![
                                ("exception_0".to_string(), None),
                                ("exception_1".to_string(), None),
                                ("simple_exception_0".to_string(), None),
                                ("simple_exception_1".to_string(), None),
                                (
                                    "simple_exception_with_answer_0".to_string(),
                                    Some("simple_answer_0".to_string()),
                                ),
                                (
                                    "simple_exception_with_answer_1".to_string(),
                                    Some("simple_answer_1".to_string()),
                                ),
                            ],
                        },
                    }],
                ),
                (
                    LessonManifest {
                        id: "literacy_course::lesson_1::reading".into(),
                        dependencies: vec![
                            "literacy_course::lesson_0::reading".into(),
                            "other_lesson".into(),
                        ],
                        encompassed: vec![
                            (Ustr::from("literacy_course::lesson_0::reading"), 1.0),
                            (Ustr::from("other_lesson"), 0.5),
                        ],
                        superseded: vec![],
                        course_id: "literacy_course".into(),
                        name: "Literacy Course - lesson_1 - Reading".into(),
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
                        name: "Literacy Course - lesson_1 - Reading".into(),
                        description: None,
                        exercise_type: ExerciseType::Procedural,
                        exercise_asset: ExerciseAsset::LiteracyAsset {
                            lesson_type: LiteracyLessonType::Reading,
                            examples: vec![
                                ("example_0".to_string(), None),
                                ("example_1".to_string(), None),
                                ("simple_example_0".to_string(), None),
                                ("simple_example_1".to_string(), None),
                                (
                                    "simple_example_with_answer_0".to_string(),
                                    Some("simple_answer_0".to_string()),
                                ),
                                (
                                    "simple_example_with_answer_1".to_string(),
                                    Some("simple_answer_1".to_string()),
                                ),
                            ],
                            exceptions: vec![
                                ("exception_0".to_string(), None),
                                ("exception_1".to_string(), None),
                                ("simple_exception_0".to_string(), None),
                                ("simple_exception_1".to_string(), None),
                                (
                                    "simple_exception_with_answer_0".to_string(),
                                    Some("simple_answer_0".to_string()),
                                ),
                                (
                                    "simple_exception_with_answer_1".to_string(),
                                    Some("simple_answer_1".to_string()),
                                ),
                            ],
                        },
                    }],
                ),
            ],
            updated_metadata: Some(BTreeMap::from([(
                "literacy_course".to_string(),
                vec!["true".to_string()],
            )])),
            updated_instructions: Some(BasicAsset::InlinedAsset {
                content: "# Course Instructions".to_string(),
            }),
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
            exercise_type: ExerciseType::Procedural,
        });
        let course_manifest = CourseManifest {
            id: "literacy_course".into(),
            name: "Literacy Course".into(),
            dependencies: vec![],
            encompassed: vec![],
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
        let mut got =
            config.generate_manifests(&vfs_path(temp_dir.path()), &course_manifest, &prefs)?;
        got.lessons.sort_by_key(|lesson| lesson.0.id);
        for (_, exercises) in &mut got.lessons {
            exercises.sort_by_key(|exercise| exercise.id);
        }

        // Verify the generated course.
        let want = GeneratedCourse {
            lessons: vec![
                (
                    LessonManifest {
                        id: "literacy_course::lesson_0::reading".into(),
                        dependencies: vec!["other_lesson".into()],
                        encompassed: vec![
                            (Ustr::from("literacy_course::lesson_0::reading"), 1.0),
                            (Ustr::from("other_lesson"), 0.5),
                        ],
                        superseded: vec![],
                        course_id: "literacy_course".into(),
                        name: "Literacy Course - lesson_0 - Reading".into(),
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
                        name: "Literacy Course - lesson_0 - Reading".into(),
                        description: None,
                        exercise_type: ExerciseType::Procedural,
                        exercise_asset: ExerciseAsset::LiteracyAsset {
                            lesson_type: LiteracyLessonType::Reading,
                            examples: vec![
                                ("example_0".to_string(), None),
                                ("example_1".to_string(), None),
                                ("simple_example_0".to_string(), None),
                                ("simple_example_1".to_string(), None),
                                (
                                    "simple_example_with_answer_0".to_string(),
                                    Some("simple_answer_0".to_string()),
                                ),
                                (
                                    "simple_example_with_answer_1".to_string(),
                                    Some("simple_answer_1".to_string()),
                                ),
                            ],
                            exceptions: vec![
                                ("exception_0".to_string(), None),
                                ("exception_1".to_string(), None),
                                ("simple_exception_0".to_string(), None),
                                ("simple_exception_1".to_string(), None),
                                (
                                    "simple_exception_with_answer_0".to_string(),
                                    Some("simple_answer_0".to_string()),
                                ),
                                (
                                    "simple_exception_with_answer_1".to_string(),
                                    Some("simple_answer_1".to_string()),
                                ),
                            ],
                        },
                    }],
                ),
                (
                    LessonManifest {
                        id: "literacy_course::lesson_1::reading".into(),
                        dependencies: vec![
                            "literacy_course::lesson_0::reading".into(),
                            "other_lesson".into(),
                        ],
                        encompassed: vec![
                            (Ustr::from("literacy_course::lesson_0::reading"), 1.0),
                            (Ustr::from("other_lesson"), 0.5),
                        ],
                        superseded: vec![],
                        course_id: "literacy_course".into(),
                        name: "Literacy Course - lesson_1 - Reading".into(),
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
                        name: "Literacy Course - lesson_1 - Reading".into(),
                        description: None,
                        exercise_type: ExerciseType::Procedural,
                        exercise_asset: ExerciseAsset::LiteracyAsset {
                            lesson_type: LiteracyLessonType::Reading,
                            examples: vec![
                                ("example_0".to_string(), None),
                                ("example_1".to_string(), None),
                                ("simple_example_0".to_string(), None),
                                ("simple_example_1".to_string(), None),
                                (
                                    "simple_example_with_answer_0".to_string(),
                                    Some("simple_answer_0".to_string()),
                                ),
                                (
                                    "simple_example_with_answer_1".to_string(),
                                    Some("simple_answer_1".to_string()),
                                ),
                            ],
                            exceptions: vec![
                                ("exception_0".to_string(), None),
                                ("exception_1".to_string(), None),
                                ("simple_exception_0".to_string(), None),
                                ("simple_exception_1".to_string(), None),
                                (
                                    "simple_exception_with_answer_0".to_string(),
                                    Some("simple_answer_0".to_string()),
                                ),
                                (
                                    "simple_exception_with_answer_1".to_string(),
                                    Some("simple_answer_1".to_string()),
                                ),
                            ],
                        },
                    }],
                ),
            ],
            updated_metadata: Some(BTreeMap::from([(
                "literacy_course".to_string(),
                vec!["true".to_string()],
            )])),
            updated_instructions: Some(BasicAsset::InlinedAsset {
                content: "# Course Instructions".to_string(),
            }),
        };
        assert_eq!(got, want);
        Ok(())
    }

    /// Verifies that the configured exercise type is applied to reading and dictation exercises.
    #[test]
    fn test_generate_manifests_exercise_type() -> Result<()> {
        // Craete a test declarative course.
        let config = LiteracyConfig {
            generate_dictation: true,
            exercise_type: ExerciseType::Declarative,
        };
        let course_manifest = CourseManifest {
            id: "literacy_course".into(),
            name: "Literacy Course".into(),
            dependencies: vec![],
            encompassed: vec![],
            superseded: vec![],
            description: None,
            authors: None,
            metadata: None,
            course_material: None,
            course_instructions: None,
            generator_config: Some(CourseGenerator::Literacy(config.clone())),
        };
        let temp_dir = tempfile::tempdir()?;
        generate_test_files(temp_dir.path(), 1, 1, 1, 0, 0)?;

        let generated = config.generate_manifests(
            &vfs_path(temp_dir.path()),
            &course_manifest,
            &UserPreferences::default(),
        )?;
        let exercise_types = generated
            .lessons
            .iter()
            .flat_map(|(_, exercises)| exercises)
            .map(|exercise| exercise.exercise_type.clone())
            .collect::<Vec<_>>();
        assert_eq!(exercise_types.len(), 2);
        assert!(
            exercise_types
                .iter()
                .all(|exercise_type| *exercise_type == ExerciseType::Declarative)
        );
        Ok(())
    }
}
