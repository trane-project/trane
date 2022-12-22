//! Contains the logic to generate a Trane course based on a knowledge base of markdown files
//! representing the front and back of flashcard exercises.

use anyhow::{anyhow, Error, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::read_dir,
    path::Path,
};
use ustr::{Ustr, UstrMap};

use crate::data::{
    BasicAsset, CourseManifest, ExerciseAsset, ExerciseManifest, ExerciseType, GenerateManifests,
    GeneratedCourse, LessonManifest, UserPreferences,
};

/// An enum representing a type of valid file that can be found in a knowledge base lesson
/// directory.
enum KnowledgeBaseFile {
    /// The file containing the dependencies of the lesson.
    Dependencies,

    /// The file containing the name of the lesson.
    Name,

    /// The file containing the description of the lesson.
    Description,

    /// The file containing the metadata of the lesson.
    Metadata,

    /// The file containing the path to the lesson material.
    LessonMaterial,

    /// The file containing the path to the lesson instructions.
    LessonInstructions,

    /// The file containing the front of the flashcard for the exercise with the given short ID.
    Front(String),

    /// The file containing the back of the flashcard for the exercise with the given short ID.
    Back(String),

    /// The file containing the name of the exercise with the given short ID.
    ExerciseName(String),

    /// The file containing the description of the exercise with the given short ID.
    ExerciseDescription(String),

    /// The file containing the type of the exercise with the given short ID.
    ExerciseType(String),
}

impl TryFrom<&str> for KnowledgeBaseFile {
    type Error = Error;

    /// Converts a file name to a `KnowledgeBaseFile` variant.
    fn try_from(file_name: &str) -> Result<Self> {
        match file_name {
            "lesson.dependencies.json" => Ok(KnowledgeBaseFile::Dependencies),
            "lesson.name.json" => Ok(KnowledgeBaseFile::Name),
            "lesson.description.json" => Ok(KnowledgeBaseFile::Description),
            "lesson.metadata.json" => Ok(KnowledgeBaseFile::Metadata),
            "lesson.material.json" => Ok(KnowledgeBaseFile::LessonMaterial),
            "lesson.instructions.json" => Ok(KnowledgeBaseFile::LessonInstructions),
            file_name if file_name.ends_with(".front.md") => {
                let short_id = file_name.strip_suffix(".front.md").unwrap();
                Ok(KnowledgeBaseFile::Front(short_id.to_string()))
            }
            file_name if file_name.ends_with(".back.md") => {
                let short_id = file_name.strip_suffix(".back.md").unwrap();
                Ok(KnowledgeBaseFile::Back(short_id.to_string()))
            }
            file_name if file_name.ends_with(".name.json") => {
                let short_id = file_name.strip_suffix(".name.json").unwrap();
                Ok(KnowledgeBaseFile::ExerciseName(short_id.to_string()))
            }
            file_name if file_name.ends_with(".description.json") => {
                let short_id = file_name.strip_suffix(".description.json").unwrap();
                Ok(KnowledgeBaseFile::ExerciseDescription(short_id.to_string()))
            }
            file_name if file_name.ends_with(".type.json") => {
                let short_id = file_name.strip_suffix(".type.json").unwrap();
                Ok(KnowledgeBaseFile::ExerciseType(short_id.to_string()))
            }
            _ => Err(anyhow!("Unknown file name: {}", file_name)),
        }
    }
}

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

    /// The path to the file containing the back of the flashcard.
    pub back_file: String,

    /// The name of the exercise to be presented to the user.
    pub name: Option<String>,

    /// An optional description of the exercise.
    pub description: Option<String>,

    /// The type of knowledge the exercise tests. Currently, Trane does not make any distinction
    /// between the types of exercises, but that will likely change in the future. The option to set
    /// the type is provided, but most users should not need to use it.
    pub exercise_type: Option<ExerciseType>,
}

impl KnowledgeBaseExercise {
    /// Generates the exercise from a list of knowledge base files.
    fn create_exercise(
        _lesson_root: &Path,
        _short_id: &str,
        _short_lesson_id: Ustr,
        _course_manifest: &CourseManifest,
        _files: &[KnowledgeBaseFile],
    ) -> Result<Self> {
        unimplemented!()
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

    /// The path to a markdown file containing the material covered in the lesson.
    pub lesson_material: Option<String>,

    /// The path to a markdown file containing the instructions common to all exercises in the
    /// lesson.
    pub lesson_instructions: Option<String>,
}

impl KnowledgeBaseLesson {
    // Filters out exercises that don't have both a front and back file.
    fn find_matching_exercises(exercise_files: &mut HashMap<String, Vec<KnowledgeBaseFile>>) {
        let mut to_remove = Vec::new();
        for (short_id, files) in exercise_files.iter() {
            let has_front = files
                .iter()
                .any(|file| matches!(file, KnowledgeBaseFile::Front(_)));
            let has_back = files
                .iter()
                .any(|file| matches!(file, KnowledgeBaseFile::Back(_)));
            if !has_front || !has_back {
                to_remove.push(short_id.clone());
            }
        }
        for short_id in to_remove {
            exercise_files.remove(&short_id);
        }
    }

    /// Generates the exercise from a list of knowledge base files.
    fn create_lesson(
        _lesson_root: &Path,
        _short_lesson_id: Ustr,
        _course_manifest: &CourseManifest,
        _files: &[KnowledgeBaseFile],
    ) -> Result<Self> {
        unimplemented!()
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
                    KnowledgeBaseFile::Front(ref short_id)
                    | KnowledgeBaseFile::Back(ref short_id)
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

        let lesson =
            Self::create_lesson(lesson_root, short_lesson_id, course_manifest, &lesson_files)?;

        // Remove exercises for the empty short ID. This can happen if the user has a file named
        // `.front.md`, for example.
        exercise_files.remove("");

        // Filter out exercises that don't have both a front and back file and generate the
        // exercises.
        Self::find_matching_exercises(&mut exercise_files);
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
            .collect::<Result<Vec<_>>>()?;
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
            lesson_instructions: lesson
                .lesson_instructions
                .map(|path| BasicAsset::MarkdownAsset { path }),
            lesson_material: lesson
                .lesson_material
                .map(|path| BasicAsset::MarkdownAsset { path }),
        }
    }
}

/// The configuration for a knowledge base course. Currently, this is an empty struct, but it is
/// added for consistency with other course generators and to implement the
/// [GenerateManifests](crate::data::GenerateManifests) trait.
#[derive(Clone, Debug, Deserialize, Serialize)]
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
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Check if the directory name is in the format `<SHORT_LESSON_ID>.lesson`. If so, read
            // the knowledge base lesson and its exercises.
            let dir_name = path.file_name().unwrap_or_default().to_str().unwrap();
            if let Some(short_id) = dir_name.strip_suffix(".lesson") {
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
        let manifests = lessons
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
