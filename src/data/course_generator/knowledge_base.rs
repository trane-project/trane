//! Contains the logic to generate a Trane course based on a knowledge base of markdown files
//! representing the front and back of flashcard exercises.

use anyhow::{anyhow, Context, Error, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::{read_dir, File},
    io::BufReader,
    path::Path,
};
use ustr::{Ustr, UstrMap};

use crate::data::{
    BasicAsset, CourseManifest, ExerciseAsset, ExerciseManifest, ExerciseType, GenerateManifests,
    GeneratedCourse, LessonManifest, UserPreferences,
};

/// The suffix used to recognize a directory as a knowledge base lesson.
pub const LESSON_SUFFIX: &str = ".lesson";

/// The name of the file containing the dependencies of a lesson.
pub const LESSON_DEPENDENCIES_FILE: &str = "lesson.dependencies.json";

/// The name of the file containing the name of a lesson.
pub const LESSON_NAME_FILE: &str = "lesson.name.json";

/// The name of the file containing the description of a lesson.
pub const LESSON_DESCRIPTION_FILE: &str = "lesson.description.json";

/// The name of the file containing the metadata of a lesson.
pub const LESSON_METADATA_FILE: &str = "lesson.metadata.json";

/// The name of the file containing the lesson instructions.
pub const LESSON_INSTRUCTIONS_FILE: &str = "lesson.instructions.md";

/// The name of the file containing the lesson material.
pub const LESSON_MATERIAL_FILE: &str = "lesson.material.md";

/// The suffix of the file containing the front of the flashcard for an exercise.
pub const EXERCISE_FRONT_SUFFIX: &str = ".front.md";

/// The suffix of the file containing the back of the flashcard for an exercise.
pub const EXERCISE_BACK_SUFFIX: &str = ".back.md";

/// The suffix of the file containing the name of an exercise.
pub const EXERCISE_NAME_SUFFIX: &str = ".name.json";

/// The suffix of the file containing the description of an exercise.
pub const EXERCISE_DESCRIPTION_SUFFIX: &str = ".description.json";

/// The suffix of the file containing the metadata of an exercise.
pub const EXERCISE_TYPE_SUFFIX: &str = ".type.json";

/// An enum representing a type of file that can be found in a knowledge base lesson directory.
#[derive(Debug, Eq, PartialEq)]
pub enum KnowledgeBaseFile {
    /// The file containing the name of the lesson.
    LessonName,

    /// The file containing the description of the lesson.
    LessonDescription,

    /// The file containing the dependencies of the lesson.
    LessonDependencies,

    /// The file containing the metadata of the lesson.
    LessonMetadata,

    /// The file containing the lesson instructions.
    LessonInstructions,

    /// The file containing the lesson material.
    LessonMaterial,

    /// The file containing the front of the flashcard for the exercise with the given short ID.
    ExerciseFront(String),

    /// The file containing the back of the flashcard for the exercise with the given short ID.
    ExerciseBack(String),

    /// The file containing the name of the exercise with the given short ID.
    ExerciseName(String),

    /// The file containing the description of the exercise with the given short ID.
    ExerciseDescription(String),

    /// The file containing the type of the exercise with the given short ID.
    ExerciseType(String),
}

impl KnowledgeBaseFile {
    /// Opens the knowledge base file at the given path and deserializes its contents.
    pub fn open<T: DeserializeOwned>(path: &Path) -> Result<T> {
        let file = File::open(path)
            .with_context(|| anyhow!("cannot open knowledge base file {}", path.display()))?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader)
            .with_context(|| anyhow!("cannot parse knowledge base file {}", path.display()))
    }
}

impl TryFrom<&str> for KnowledgeBaseFile {
    type Error = Error;

    /// Converts a file name to a `KnowledgeBaseFile` variant.
    fn try_from(file_name: &str) -> Result<Self> {
        match file_name {
            LESSON_DEPENDENCIES_FILE => Ok(KnowledgeBaseFile::LessonDependencies),
            LESSON_NAME_FILE => Ok(KnowledgeBaseFile::LessonName),
            LESSON_DESCRIPTION_FILE => Ok(KnowledgeBaseFile::LessonDescription),
            LESSON_METADATA_FILE => Ok(KnowledgeBaseFile::LessonMetadata),
            LESSON_MATERIAL_FILE => Ok(KnowledgeBaseFile::LessonMaterial),
            LESSON_INSTRUCTIONS_FILE => Ok(KnowledgeBaseFile::LessonInstructions),
            file_name if file_name.ends_with(EXERCISE_FRONT_SUFFIX) => {
                let short_id = file_name.strip_suffix(EXERCISE_FRONT_SUFFIX).unwrap();
                Ok(KnowledgeBaseFile::ExerciseFront(short_id.to_string()))
            }
            file_name if file_name.ends_with(EXERCISE_BACK_SUFFIX) => {
                let short_id = file_name.strip_suffix(EXERCISE_BACK_SUFFIX).unwrap();
                Ok(KnowledgeBaseFile::ExerciseBack(short_id.to_string()))
            }
            file_name if file_name.ends_with(EXERCISE_NAME_SUFFIX) => {
                let short_id = file_name.strip_suffix(EXERCISE_NAME_SUFFIX).unwrap();
                Ok(KnowledgeBaseFile::ExerciseName(short_id.to_string()))
            }
            file_name if file_name.ends_with(EXERCISE_DESCRIPTION_SUFFIX) => {
                let short_id = file_name.strip_suffix(EXERCISE_DESCRIPTION_SUFFIX).unwrap();
                Ok(KnowledgeBaseFile::ExerciseDescription(short_id.to_string()))
            }
            file_name if file_name.ends_with(EXERCISE_TYPE_SUFFIX) => {
                let short_id = file_name.strip_suffix(EXERCISE_TYPE_SUFFIX).unwrap();
                Ok(KnowledgeBaseFile::ExerciseType(short_id.to_string()))
            }
            _ => Err(anyhow!(
                "Not a valid knowledge base file name: {}",
                file_name
            )),
        }
    }
}

//@<knowledge-base-exercise
/// Represents a knowledge base exercise.
///
/// Inside a knowledge base lesson directory, Trane will look for matching pairs of files with names
/// `<SHORT_EXERCISE_ID>.front.md` and `<SHORT_EXERCISE_ID>.back.md`. The short ID is used to
/// generate the final exercise ID, by combining it with the lesson ID. For example, files
/// `e.front.md` and `e.back.md` in a course with ID `a::b::c` inside a lesson directory named
/// `d.lesson` will generate and exercise with ID `a::b::c::d::e`.
///
/// Each the optional fields mirror one of the fields in the
/// [ExerciseManifest](crate::data::ExerciseManifest) and their values can be set by writing a JSON
/// file inside the lesson directory with the name `<SHORT_EXERCISE_ID>.<PROPERTY_NAME>.json`. This
/// file should contain a JSON serialization of the desired value. For example, to set the
/// exercise's name for an exercise with a short ID value of `ex1`, one would write a file named
/// `ex1.name.json` containing a JSON string with the desired name.
///
/// Trane will ignore any markdown files that do not match the exercise name pattern or that do not
/// have a matching pair of front and back files.
pub struct KnowledgeBaseExercise {
    /// The short ID of the lesson, which is used to easily identify the exercise and to generate
    /// the final exercise ID.
    pub short_id: String,

    /// The short ID of the lesson to which this exercise belongs.
    pub short_lesson_id: Ustr,

    /// The ID of the course to which this lesson belongs.
    pub course_id: Ustr,

    /// The path to the file containing the front of the flashcard.
    pub front_file: String,

    /// The path to the file containing the back of the flashcard. This path is optional, because a
    /// flashcard is not required to provide an answer.
    pub back_file: Option<String>,

    /// The name of the exercise to be presented to the user.
    pub name: Option<String>,

    /// An optional description of the exercise.
    pub description: Option<String>,

    /// The type of knowledge the exercise tests. Currently, Trane does not make any distinction
    /// between the types of exercises, but that will likely change in the future. The option to set
    /// the type is provided, but most users should not need to use it.
    pub exercise_type: Option<ExerciseType>,
}
//>@knowledge-base-exercise

impl KnowledgeBaseExercise {
    /// Generates the exercise from a list of knowledge base files.
    fn create_exercise(
        lesson_root: &Path,
        short_id: &str,
        short_lesson_id: Ustr,
        course_manifest: &CourseManifest,
        files: &[KnowledgeBaseFile],
    ) -> Result<Self> {
        // Check if the exercise has a back file and create it accordingly.
        let has_back_file = files.iter().any(|file| match file {
            KnowledgeBaseFile::ExerciseBack(id) => id == short_id,
            _ => false,
        });
        let back_file = if has_back_file {
            Some(
                lesson_root
                    .join(format!("{short_id}{EXERCISE_BACK_SUFFIX}"))
                    .to_str()
                    .unwrap_or_default()
                    .to_string(),
            )
        } else {
            None
        };

        // Create the exercise with `None` values for all optimal fields.
        let mut exercise = KnowledgeBaseExercise {
            short_id: short_id.to_string(),
            short_lesson_id,
            course_id: course_manifest.id,
            front_file: lesson_root
                .join(format!("{short_id}{EXERCISE_FRONT_SUFFIX}"))
                .to_str()
                .unwrap_or_default()
                .to_string(),
            back_file,
            name: None,
            description: None,
            exercise_type: None,
        };

        // Iterate through the exercise files found in the lesson directory and set the
        // corresponding field in the exercise. The front and back files are ignored because the
        // correct values were already set above.
        for exercise_file in files {
            match exercise_file {
                KnowledgeBaseFile::ExerciseName(..) => {
                    let path = lesson_root.join(format!("{short_id}{EXERCISE_NAME_SUFFIX}"));
                    exercise.name = Some(KnowledgeBaseFile::open(&path)?);
                }
                KnowledgeBaseFile::ExerciseDescription(..) => {
                    let path = lesson_root.join(format!("{short_id}{EXERCISE_DESCRIPTION_SUFFIX}"));
                    exercise.description = Some(KnowledgeBaseFile::open(&path)?);
                }
                KnowledgeBaseFile::ExerciseType(..) => {
                    let path = lesson_root.join(format!("{short_id}{EXERCISE_TYPE_SUFFIX}"));
                    exercise.exercise_type = Some(KnowledgeBaseFile::open(&path)?);
                }
                _ => {}
            }
        }
        Ok(exercise)
    }
}

impl From<KnowledgeBaseExercise> for ExerciseManifest {
    /// Generates the manifest for this exercise.
    fn from(exercise: KnowledgeBaseExercise) -> Self {
        Self {
            id: format!(
                "{}::{}::{}",
                exercise.course_id, exercise.short_lesson_id, exercise.short_id
            )
            .into(),
            lesson_id: format!("{}::{}", exercise.course_id, exercise.short_lesson_id).into(),
            course_id: exercise.course_id,
            name: exercise
                .name
                .unwrap_or(format!("Exercise {}", exercise.short_id)),
            description: exercise.description,
            exercise_type: exercise.exercise_type.unwrap_or(ExerciseType::Procedural),
            exercise_asset: ExerciseAsset::FlashcardAsset {
                front_path: exercise.front_file,
                back_path: exercise.back_file,
            },
        }
    }
}

//@<knowledge-base-lesson
/// Represents a knowledge base lesson.
///
/// In a knowledge base course, lessons are generated by searching for all directories with a name
/// in the format `<SHORT_LESSON_ID>.lesson`. In this case, the short ID is not the entire lesson ID
/// one would use in the lesson manifest, but rather a short identifier that is combined with the
/// course ID to generate the final lesson ID. For example, a course with ID `a::b::c` which
/// contains a directory of name `d.lesson` will generate the manifest for a lesson with ID
/// `a::b::c::d`.
///
/// All the optional fields mirror one of the fields in the
/// [LessonManifest](crate::data::LessonManifest) and their values can be set by writing a JSON file
/// inside the lesson directory with the name `lesson.<PROPERTY_NAME>.json`. This file should
/// contain a JSON serialization of the desired value. For example, to set the lesson's dependencies
/// one would write a file named `lesson.dependencies.json` containing a JSON array of strings, each
/// of them the ID of a dependency.
///
/// The material and instructions of the lesson do not follow this convention. Instead, the files
/// `lesson.instructoins.md` and `lesson.material.md` contain the instructions and material of the
/// lesson.
///
/// None of the `<SHORT_LESSON_ID>.lesson` directories should contain a `lesson_manifest.json` file,
/// as that file would indicate to Trane that this is a regular lesson and not a generated lesson.
pub struct KnowledgeBaseLesson {
    /// The short ID of the lesson, which is used to easily identify the lesson and to generate the
    /// final lesson ID.
    pub short_id: Ustr,

    /// The ID of the course to which this lesson belongs.
    pub course_id: Ustr,

    /// The IDs of all dependencies of this lesson. The values can be full lesson IDs or the short
    /// ID of one of the other lessons in the course. If Trane finds a dependency with a short ID,
    /// it will automatically generate the full lesson ID. Not setting this value will indicate that
    /// the lesson has no dependencies.
    pub dependencies: Option<Vec<Ustr>>,

    /// The name of the lesson to be presented to the user.
    pub name: Option<String>,

    /// An optional description of the lesson.
    pub description: Option<String>,

    //// A mapping of String keys to a list of String values used to store arbitrary metadata about
    ///the lesson. This value is set to a `BTreeMap` to ensure that the keys are sorted in a
    ///consistent order when serialized. This is an implementation detail and does not affect how
    ///the value should be written to a file. A JSON map of strings to list of strings works.
    pub metadata: Option<BTreeMap<String, Vec<String>>>,

    /// Indicates whether the `lesson.instructions.md` file is present in the lesson directory.
    pub has_instructions: bool,

    /// Indicates whether the `lesson.material.md` file is present in the lesson directory.
    pub has_material: bool,
}
//>@knowledge-base-lesson

impl KnowledgeBaseLesson {
    // Filters out exercises that don't have both a front file. Exercises without a back file are
    // allowed, as it is not required to have one.
    fn filter_matching_exercises(exercise_files: &mut HashMap<String, Vec<KnowledgeBaseFile>>) {
        let mut to_remove = Vec::new();
        for (short_id, files) in exercise_files.iter() {
            let has_front = files
                .iter()
                .any(|file| matches!(file, KnowledgeBaseFile::ExerciseFront(_)));
            if !has_front {
                to_remove.push(short_id.clone());
            }
        }
        for short_id in to_remove {
            exercise_files.remove(&short_id);
        }
    }

    /// Generates the exercise from a list of knowledge base files.
    fn create_lesson(
        lesson_root: &Path,
        short_lesson_id: Ustr,
        course_manifest: &CourseManifest,
        files: &[KnowledgeBaseFile],
    ) -> Result<Self> {
        // Create the lesson with all the optional fields set to a default value.
        let mut lesson = Self {
            short_id: short_lesson_id,
            course_id: course_manifest.id,
            dependencies: None,
            name: None,
            description: None,
            metadata: None,
            has_instructions: false,
            has_material: false,
        };

        // Iterate through the lesson files found in the lesson directory and set the corresponding
        // field in the lesson.
        for lesson_file in files {
            match lesson_file {
                KnowledgeBaseFile::LessonDependencies => {
                    let path = lesson_root.join(LESSON_DEPENDENCIES_FILE);
                    lesson.dependencies = Some(KnowledgeBaseFile::open(&path)?)
                }
                KnowledgeBaseFile::LessonName => {
                    let path = lesson_root.join(LESSON_NAME_FILE);
                    lesson.name = Some(KnowledgeBaseFile::open(&path)?)
                }
                KnowledgeBaseFile::LessonDescription => {
                    let path = lesson_root.join(LESSON_DESCRIPTION_FILE);
                    lesson.description = Some(KnowledgeBaseFile::open(&path)?)
                }
                KnowledgeBaseFile::LessonMetadata => {
                    let path = lesson_root.join(LESSON_METADATA_FILE);
                    lesson.metadata = Some(KnowledgeBaseFile::open(&path)?)
                }
                KnowledgeBaseFile::LessonInstructions => lesson.has_instructions = true,
                KnowledgeBaseFile::LessonMaterial => lesson.has_material = true,
                _ => {}
            }
        }
        Ok(lesson)
    }

    /// Opens a lesson from the knowledge base with the given root and short ID.
    fn open_lesson(
        lesson_root: &Path,
        course_manifest: &CourseManifest,
        short_lesson_id: Ustr,
    ) -> Result<(KnowledgeBaseLesson, Vec<KnowledgeBaseExercise>)> {
        // Iterate through the directory to find all the matching files in the lesson directory.
        let mut lesson_files = Vec::new();
        let mut exercise_files = HashMap::new();
        for entry in read_dir(lesson_root)? {
            let entry = entry?;
            let file_name = entry.file_name();
            let file_name: &str = file_name.to_str().unwrap_or_default();
            if let Ok(kb_file) = KnowledgeBaseFile::try_from(file_name) {
                match kb_file {
                    KnowledgeBaseFile::ExerciseFront(ref short_id)
                    | KnowledgeBaseFile::ExerciseBack(ref short_id)
                    | KnowledgeBaseFile::ExerciseName(ref short_id)
                    | KnowledgeBaseFile::ExerciseDescription(ref short_id)
                    | KnowledgeBaseFile::ExerciseType(ref short_id) => {
                        exercise_files
                            .entry(short_id.clone())
                            .or_insert_with(Vec::new)
                            .push(kb_file);
                    }
                    _ => lesson_files.push(kb_file),
                }
            }
        }

        // Create the knowledge base lesson.
        let lesson =
            Self::create_lesson(lesson_root, short_lesson_id, course_manifest, &lesson_files)?;

        // Remove exercises for the empty short ID. This can happen if the user has a file named
        // `.front.md`, for example.
        exercise_files.remove("");

        // Filter out exercises that don't have both a front and back file and create the knowledge
        // base exercises.
        Self::filter_matching_exercises(&mut exercise_files);
        let exercises = exercise_files
            .into_iter()
            .map(|(short_id, files)| {
                KnowledgeBaseExercise::create_exercise(
                    lesson_root,
                    &short_id,
                    short_lesson_id,
                    course_manifest,
                    &files,
                )
            })
            .collect::<Result<Vec<_>>>()?; // grcov-excl-line
        Ok((lesson, exercises))
    }
}

impl From<KnowledgeBaseLesson> for LessonManifest {
    /// Generates the manifest for this lesson.
    fn from(lesson: KnowledgeBaseLesson) -> Self {
        Self {
            id: format!("{}::{}", lesson.course_id, lesson.short_id).into(),
            course_id: lesson.course_id,
            dependencies: lesson.dependencies.unwrap_or_default(),
            name: lesson.name.unwrap_or(format!("Lesson {}", lesson.short_id)),
            description: lesson.description,
            metadata: lesson.metadata,
            lesson_instructions: if lesson.has_instructions {
                Some(BasicAsset::MarkdownAsset {
                    path: LESSON_INSTRUCTIONS_FILE.into(),
                })
            } else {
                None
            },
            lesson_material: if lesson.has_material {
                Some(BasicAsset::MarkdownAsset {
                    path: LESSON_MATERIAL_FILE.into(),
                })
            } else {
                None
            },
        }
    }
}

/// The configuration for a knowledge base course. Currently, this is an empty struct, but it is
/// added for consistency with other course generators and to implement the
/// [GenerateManifests](crate::data::GenerateManifests) trait.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct KnowledgeBaseConfig {}

impl KnowledgeBaseConfig {
    // Checks if the dependencies refer to another lesson in the course by its short ID and updates
    // them to refer to the full lesson ID.
    fn convert_to_full_dependencies(
        course_manifest: &CourseManifest,
        short_ids: HashSet<Ustr>,
        lessons: &mut UstrMap<(KnowledgeBaseLesson, Vec<KnowledgeBaseExercise>)>,
    ) {
        lessons.iter_mut().for_each(|(_, lesson)| {
            if let Some(dependencies) = &lesson.0.dependencies {
                let updated_dependencies = dependencies
                    .iter()
                    .map(|dependency| {
                        if short_ids.contains(dependency) {
                            // The dependency is a short ID, so we need to update it to the full ID.
                            format!("{}::{}", course_manifest.id, dependency).into()
                        } else {
                            // The dependency is already a full ID, so we can just add it to the
                            // list.
                            *dependency
                        }
                    })
                    .collect();
                lesson.0.dependencies = Some(updated_dependencies);
            }
        });
    }
}

impl GenerateManifests for KnowledgeBaseConfig {
    fn generate_manifests(
        &self,
        course_root: &Path,
        course_manifest: &CourseManifest,
        _preferences: &UserPreferences,
    ) -> Result<GeneratedCourse> {
        // Store the lessons and their exercises in a map of short lesson ID to a tuple of the
        // lesson and its exercises.
        let mut lessons = UstrMap::default();

        // Iterate through all the directories in the course root, processing only those whose name
        // fits the pattern `<SHORT_LESSON_ID>.lesson`.
        for entry in read_dir(course_root)? {
            // Ignore the entry if it's not a directory.
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Check if the directory name is in the format `<SHORT_LESSON_ID>.lesson`. If so, read
            // the knowledge base lesson and its exercises.
            let dir_name = path.file_name().unwrap_or_default().to_str().unwrap();
            if let Some(short_id) = dir_name.strip_suffix(LESSON_SUFFIX) {
                lessons.insert(
                    short_id.into(),
                    KnowledgeBaseLesson::open_lesson(&path, course_manifest, short_id.into())?,
                );
            }
        }

        // Convert all the dependencies to full lesson IDs.
        let short_ids: HashSet<Ustr> = lessons.keys().cloned().collect();
        KnowledgeBaseConfig::convert_to_full_dependencies(course_manifest, short_ids, &mut lessons);

        // Generate the manifests for all the lessons and exercises.
        let manifests: Vec<(LessonManifest, Vec<ExerciseManifest>)> = lessons
            .into_iter()
            .map(|(_, (lesson, exercises))| {
                let lesson_manifest = LessonManifest::from(lesson);
                let exercise_manifests =
                    exercises.into_iter().map(ExerciseManifest::from).collect();
                (lesson_manifest, exercise_manifests)
            })
            .collect();

        Ok(GeneratedCourse {
            lessons: manifests,
            updated_instructions: None,
            updated_metadata: None,
        })
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use std::{
        fs::{self, Permissions},
        io::{BufWriter, Write},
        os::unix::prelude::PermissionsExt,
    };

    use super::*;

    // Verifies opening a valid knowledge base file.
    #[test]
    fn open_knowledge_base_file() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let file_path = temp_dir.path().join("lesson.dependencies.properties");
        let mut file = File::create(&file_path)?;
        file.write_all(b"[\"lesson1\"]")?;

        let dependencies: Vec<String> = KnowledgeBaseFile::open(&file_path)?;
        assert_eq!(dependencies, vec!["lesson1".to_string()]);
        Ok(())
    }

    // Verifies the handling of invalid knowledge base files.
    #[test]
    fn open_knowledge_base_file_bad_format() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let file_path = temp_dir.path().join("lesson.dependencies.properties");
        let mut file = File::create(&file_path)?;
        file.write_all(b"[\"lesson1\"")?;

        let dependencies: Result<Vec<String>> = KnowledgeBaseFile::open(&file_path);
        assert!(dependencies.is_err());
        Ok(())
    }

    // Verifies the handling of knowledge base files that cannot be opened.
    #[test]
    fn open_knowledge_base_file_bad_permissions() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let file_path = temp_dir.path().join("lesson.dependencies.properties");
        let mut file = File::create(&file_path)?;
        file.write_all(b"[\"lesson1\"]")?;

        // Make the directory non-readable to test that the file can't be opened.
        std::fs::set_permissions(temp_dir.path(), Permissions::from_mode(0o000))?;

        let dependencies: Result<Vec<String>> = KnowledgeBaseFile::open(&file_path);
        assert!(dependencies.is_err());
        Ok(())
    }

    // Verifies that the all the files with knowledge base names are detected correctly.
    #[test]
    fn to_knowledge_base_file() {
        // Parse lesson file names.
        assert_eq!(
            KnowledgeBaseFile::LessonDependencies,
            KnowledgeBaseFile::try_from(LESSON_DEPENDENCIES_FILE).unwrap(),
        );
        assert_eq!(
            KnowledgeBaseFile::LessonDescription,
            KnowledgeBaseFile::try_from(LESSON_DESCRIPTION_FILE).unwrap(),
        );
        assert_eq!(
            KnowledgeBaseFile::LessonMetadata,
            KnowledgeBaseFile::try_from(LESSON_METADATA_FILE).unwrap(),
        );
        assert_eq!(
            KnowledgeBaseFile::LessonInstructions,
            KnowledgeBaseFile::try_from(LESSON_INSTRUCTIONS_FILE).unwrap(),
        );
        assert_eq!(
            KnowledgeBaseFile::LessonMaterial,
            KnowledgeBaseFile::try_from(LESSON_MATERIAL_FILE).unwrap(),
        );

        // Parse exercise file names.
        assert_eq!(
            KnowledgeBaseFile::ExerciseName("ex1".to_string()),
            KnowledgeBaseFile::try_from(format!("{}{}", "ex1", EXERCISE_NAME_SUFFIX).as_str())
                .unwrap(),
        );
        assert_eq!(
            KnowledgeBaseFile::ExerciseFront("ex1".to_string()),
            KnowledgeBaseFile::try_from(format!("{}{}", "ex1", EXERCISE_FRONT_SUFFIX).as_str())
                .unwrap(),
        );
        assert_eq!(
            KnowledgeBaseFile::ExerciseBack("ex1".to_string()),
            KnowledgeBaseFile::try_from(format!("{}{}", "ex1", EXERCISE_BACK_SUFFIX).as_str())
                .unwrap(),
        );
        assert_eq!(
            KnowledgeBaseFile::ExerciseDescription("ex1".to_string()),
            KnowledgeBaseFile::try_from(
                format!("{}{}", "ex1", EXERCISE_DESCRIPTION_SUFFIX).as_str()
            )
            .unwrap(),
        );
        assert_eq!(
            KnowledgeBaseFile::ExerciseType("ex1".to_string()),
            KnowledgeBaseFile::try_from(format!("{}{}", "ex1", EXERCISE_TYPE_SUFFIX).as_str())
                .unwrap(),
        );

        // Parse exercise file names with invalid exercise names.
        assert!(KnowledgeBaseFile::try_from("ex1").is_err());
    }

    // Verifies the conversion from a knowledge base lesson to a lesson manifest.
    #[test]
    fn lesson_to_manifest() {
        let lesson = KnowledgeBaseLesson {
            short_id: "lesson1".into(),
            course_id: "course1".into(),
            name: Some("Name".into()),
            description: Some("Description".into()),
            dependencies: Some(vec!["lesson2".into()]),
            metadata: Some(BTreeMap::from([("key".into(), vec!["value".into()])])),
            has_instructions: true,
            has_material: true,
        };
        let expected_manifest = LessonManifest {
            id: "course1::lesson1".into(),
            course_id: "course1".into(),
            name: "Name".into(),
            description: Some("Description".into()),
            dependencies: vec!["lesson2".into()],
            lesson_instructions: Some(BasicAsset::MarkdownAsset {
                path: LESSON_INSTRUCTIONS_FILE.into(),
            }),
            lesson_material: Some(BasicAsset::MarkdownAsset {
                path: LESSON_MATERIAL_FILE.into(),
            }),
            metadata: Some(BTreeMap::from([("key".into(), vec!["value".into()])])),
        };
        let actual_manifest: LessonManifest = lesson.into();
        assert_eq!(actual_manifest, expected_manifest);
    }

    // Verifies the conversion from a knowledge base exercise to an exercise manifest.
    #[test]
    fn exercise_to_manifest() {
        let exercise = KnowledgeBaseExercise {
            short_id: "ex1".into(),
            short_lesson_id: "lesson1".into(),
            course_id: "course1".into(),
            front_file: "ex1.front.md".into(),
            back_file: Some("ex1.back.md".into()),
            name: Some("Name".into()),
            description: Some("Description".into()),
            exercise_type: Some(ExerciseType::Procedural),
        };
        let expected_manifest = ExerciseManifest {
            id: "course1::lesson1::ex1".into(),
            lesson_id: "course1::lesson1".into(),
            course_id: "course1".into(),
            name: "Name".into(),
            description: Some("Description".into()),
            exercise_type: ExerciseType::Procedural,
            exercise_asset: ExerciseAsset::FlashcardAsset {
                front_path: "ex1.front.md".into(),
                back_path: Some("ex1.back.md".into()),
            },
        };
        let actual_manifest: ExerciseManifest = exercise.into();
        assert_eq!(actual_manifest, expected_manifest);
    }

    // Verifies that dependencies referenced by their short IDs are converted to full IDs.
    #[test]
    fn convert_to_full_dependencies() {
        // Create an example course manifest.
        let course_manifest = CourseManifest {
            id: "course1".into(),
            name: "Course 1".into(),
            dependencies: vec![],
            description: Some("Description".into()),
            authors: None,
            metadata: Some(BTreeMap::from([("key".into(), vec!["value".into()])])),
            course_instructions: None,
            course_material: None,
            generator_config: None,
        };

        // Create an example lesson with a dependency referred to by its short ID and an example
        // exercise.
        let short_lesson_id = Ustr::from("lesson1");
        let lesson = KnowledgeBaseLesson {
            short_id: short_lesson_id,
            course_id: "course1".into(),
            name: Some("Name".into()),
            description: Some("Description".into()),
            dependencies: Some(vec!["lesson2".into(), "other::lesson1".into()]),
            metadata: Some(BTreeMap::from([("key".into(), vec!["value".into()])])),
            has_instructions: false,
            has_material: false,
        };
        let exercise = KnowledgeBaseExercise {
            short_id: "ex1".into(),
            short_lesson_id,
            course_id: "course1".into(),
            front_file: "ex1.front.md".into(),
            back_file: Some("ex1.back.md".into()),
            name: Some("Name".into()),
            description: Some("Description".into()),
            exercise_type: Some(ExerciseType::Procedural),
        };
        let mut lesson_map = UstrMap::default();
        lesson_map.insert("lesson1".into(), (lesson, vec![exercise]));

        // Convert the short IDs to full IDs.
        let short_ids = HashSet::from_iter(vec!["lesson1".into(), "lesson2".into()]);
        KnowledgeBaseConfig::convert_to_full_dependencies(
            &course_manifest,
            short_ids.clone(),
            &mut lesson_map,
        );

        assert_eq!(
            lesson_map.get(&short_lesson_id).unwrap().0.dependencies,
            Some(vec!["course1::lesson2".into(), "other::lesson1".into()])
        );
    }

    /// Verifies that exercises with a missing front or back files are filtered out.
    #[test]
    fn filter_matching_exercises() {
        let mut exercise_map = HashMap::default();
        // Exercise 1 has both a front and back file.
        let ex1_id: String = "ex1".into();
        let ex1_files = vec![
            KnowledgeBaseFile::ExerciseFront("ex1".into()),
            KnowledgeBaseFile::ExerciseBack("ex1".into()),
        ];
        // Exercise 2 only has a front file.
        let ex2_id: String = "ex2".into();
        let ex2_files = vec![KnowledgeBaseFile::ExerciseFront("ex2".into())];
        // Exercise 3 only has a back file.
        let ex3_id: String = "ex3".into();
        let ex3_files = vec![KnowledgeBaseFile::ExerciseBack("ex3".into())];
        exercise_map.insert(ex1_id.clone(), ex1_files);
        exercise_map.insert(ex2_id.clone(), ex2_files);
        exercise_map.insert(ex3_id.clone(), ex3_files);

        // Verify that the correct exercises were filtered out.
        KnowledgeBaseLesson::filter_matching_exercises(&mut exercise_map);
        let ex1_expected = vec![
            KnowledgeBaseFile::ExerciseFront("ex1".into()),
            KnowledgeBaseFile::ExerciseBack("ex1".into()),
        ];
        assert_eq!(exercise_map.get(&ex1_id).unwrap(), &ex1_expected);
        let ex2_expected = vec![KnowledgeBaseFile::ExerciseFront("ex2".into())];
        assert_eq!(exercise_map.get(&ex2_id).unwrap(), &ex2_expected);
        assert!(!exercise_map.contains_key(&ex3_id));
    }

    // Serializes the object in JSON and writes it to the given file.
    fn write_json<T: Serialize>(obj: &T, file: &Path) -> Result<()> {
        let file = File::create(file)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, obj)?;
        Ok(())
    }

    // Verifies opening a lesson directory.
    #[test]
    fn open_lesson_dir() -> Result<()> {
        // Create a test course and lesson directory.
        let course_dir = tempfile::tempdir()?;
        let lesson_dir = course_dir.path().join("lesson1.lesson");
        fs::create_dir(&lesson_dir)?;

        // Create lesson files in the directory.
        let name = "Name";
        let name_path = lesson_dir.join(LESSON_NAME_FILE);
        write_json(&name, &name_path)?;

        let description = "Description";
        let description_path = lesson_dir.join(LESSON_DESCRIPTION_FILE);
        write_json(&description, &description_path)?;

        let dependencies: Vec<Ustr> = vec!["lesson2".into(), "lesson3".into()];
        let dependencies_path = lesson_dir.join(LESSON_DEPENDENCIES_FILE);
        write_json(&dependencies, &dependencies_path)?;

        let metadata: BTreeMap<String, Vec<String>> =
            BTreeMap::from([("key".into(), vec!["value".into()])]);
        let metadata_path = lesson_dir.join(LESSON_METADATA_FILE);
        write_json(&metadata, &metadata_path)?;

        let instructions = "instructions";
        let instructions_path = lesson_dir.join(LESSON_INSTRUCTIONS_FILE);
        write_json(&instructions, &instructions_path)?;

        let material = "material";
        let material_path = lesson_dir.join(LESSON_MATERIAL_FILE);
        write_json(&material, &material_path)?;

        // Create an example exercise and all of its files.
        let front_content = "Front content";
        let front_path = lesson_dir.join("ex1.front.md");
        fs::write(front_path, front_content)?;

        let back_content = "Back content";
        let back_path = lesson_dir.join("ex1.back.md");
        fs::write(back_path, back_content)?;

        let exercise_name = "Exercise name";
        let exercise_name_path = lesson_dir.join("ex1.name.json");
        write_json(&exercise_name, &exercise_name_path)?;

        let exercise_description = "Exercise description";
        let exercise_description_path = lesson_dir.join("ex1.description.json");
        write_json(&exercise_description, &exercise_description_path)?;

        let exercise_type = ExerciseType::Procedural;
        let exercise_type_path = lesson_dir.join("ex1.type.json");
        write_json(&exercise_type, &exercise_type_path)?;

        // Create a test course manifest.
        let course_manifest = CourseManifest {
            id: "course1".into(),
            name: "Course 1".into(),
            dependencies: vec![],
            description: Some("Description".into()),
            authors: None,
            metadata: Some(BTreeMap::from([("key".into(), vec!["value".into()])])),
            course_instructions: None,
            course_material: None,
            generator_config: None,
        };

        // Open the lesson directory.
        let (lesson, exercises) =
            KnowledgeBaseLesson::open_lesson(&lesson_dir, &course_manifest, "lesson1".into())?;

        // Verify the lesson.
        assert_eq!(lesson.name, Some(name.into()));
        assert_eq!(lesson.description, Some(description.into()));
        assert_eq!(lesson.dependencies, Some(dependencies));
        assert_eq!(lesson.metadata, Some(metadata));
        assert!(lesson.has_instructions);
        assert!(lesson.has_material);

        // Verify the exercise.
        assert_eq!(exercises.len(), 1);
        let exercise = &exercises[0];
        assert_eq!(exercise.name, Some(exercise_name.into()));
        assert_eq!(exercise.description, Some(exercise_description.into()));
        assert_eq!(exercise.exercise_type, Some(exercise_type));
        assert_eq!(
            exercise.front_file,
            lesson_dir
                .join("ex1.front.md")
                .to_str()
                .unwrap()
                .to_string()
        );
        assert_eq!(
            exercise.back_file.to_owned().unwrap_or_default(),
            lesson_dir.join("ex1.back.md").to_str().unwrap().to_string()
        );
        Ok(())
    }
}
