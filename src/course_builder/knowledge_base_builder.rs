//! Contains utilities to make it easier to build knowledge base courses.
//!
//! The knowledge base course format is a plain-text format that is intended to be easy to edit by
//! hand. This module contains utilities to make it easier to generate these files, specially for
//! testing purposes.

use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, create_dir_all, File},
    io::Write,
    path::Path,
};
use ustr::Ustr;

use crate::{
    course_builder::AssetBuilder,
    course_library::COURSE_MANIFEST_FILENAME,
    data::{course_generator::knowledge_base::*, CourseManifest},
};

/// A builder to generate a knowledge base exercise and associated assets.
pub struct ExerciseBuilder {
    /// The knowledge base exercise to build.
    pub exercise: KnowledgeBaseExercise,

    /// The assets associated with this exercise, which include the front and back of the flashcard.
    pub asset_builders: Vec<AssetBuilder>,
}

impl ExerciseBuilder {
    /// Writes the files needed for this exercise to the given directory.
    pub fn build(&self, lesson_directory: &Path) -> Result<()> {
        // Build all the assets.
        for builder in &self.asset_builders {
            builder.build(lesson_directory)?;
        }

        // Write the exercise properties to the corresponding file.
        if let Some(name) = &self.exercise.name {
            let name_json = serde_json::to_string_pretty(name)?;
            let name_path = lesson_directory.join(format!(
                "{}{}",
                self.exercise.short_id, EXERCISE_NAME_SUFFIX
            ));
            let mut name_file = File::create(name_path)?;
            name_file.write_all(name_json.as_bytes())?;
        }
        if let Some(description) = &self.exercise.description {
            let description_json = serde_json::to_string_pretty(description)?;
            let description_path = lesson_directory.join(format!(
                "{}{}",
                self.exercise.short_id, EXERCISE_DESCRIPTION_SUFFIX
            ));
            let mut description_file = File::create(description_path)?;
            description_file.write_all(description_json.as_bytes())?;
        }
        if let Some(exercise_type) = &self.exercise.exercise_type {
            let exercise_type_json = serde_json::to_string_pretty(exercise_type)?;
            let exercise_type_path = lesson_directory.join(format!(
                "{}{}",
                self.exercise.short_id, EXERCISE_TYPE_SUFFIX
            ));
            let mut exercise_type_file = File::create(exercise_type_path)?;
            exercise_type_file.write_all(exercise_type_json.as_bytes())?;
        }
        Ok(())
    }
}

/// A builder to generate a knowledge base lesson and associated assets.
pub struct LessonBuilder {
    /// The knowledge base lesson to build.
    pub lesson: KnowledgeBaseLesson,

    /// The exercise builders for this lesson.
    pub exercises: Vec<ExerciseBuilder>,

    /// The assets associated with this lesson, which include the lesson instructions and materials.
    pub asset_builders: Vec<AssetBuilder>,
}

impl LessonBuilder {
    /// Writes the files needed for this lesson to the given directory.
    pub fn build(&self, lesson_directory: &Path) -> Result<()> {
        // Build all the assets.
        for builder in &self.asset_builders {
            builder.build(lesson_directory)?;
        }

        // Build all the exercises.
        for builder in &self.exercises {
            builder.build(lesson_directory)?;
        }

        // Write the lesson properties to the corresponding file.
        if let Some(name) = &self.lesson.name {
            let name_json = serde_json::to_string_pretty(name)?;
            let name_path = lesson_directory.join(LESSON_NAME_FILE);
            let mut name_file = File::create(name_path)?;
            name_file.write_all(name_json.as_bytes())?;
        }
        if let Some(description) = &self.lesson.description {
            let description_json = serde_json::to_string_pretty(description)?;
            let description_path = lesson_directory.join(LESSON_DESCRIPTION_FILE);
            let mut description_file = File::create(description_path)?;
            description_file.write_all(description_json.as_bytes())?;
        }
        if let Some(dependencies) = &self.lesson.dependencies {
            let dependencies_json = serde_json::to_string_pretty(dependencies)?;
            let dependencies_path = lesson_directory.join(LESSON_DEPENDENCIES_FILE);
            let mut dependencies_file = File::create(dependencies_path)?;
            dependencies_file.write_all(dependencies_json.as_bytes())?;
        }
        if let Some(metadata) = &self.lesson.metadata {
            let metadata_json = serde_json::to_string_pretty(metadata)?;
            let metadata_path = lesson_directory.join(LESSON_METADATA_FILE);
            let mut metadata_file = File::create(metadata_path)?;
            metadata_file.write_all(metadata_json.as_bytes())?;
        }
        Ok(())
    }
}

/// A builder to generate a knowledge base course and associated assets.
pub struct CourseBuilder {
    /// Base name of the directory on which to store this course.
    pub directory_name: String,

    /// The builders for the lessons in this course.
    pub lessons: Vec<LessonBuilder>,

    /// The assets associated with this course.
    pub assets: Vec<AssetBuilder>,

    /// The manifest for this course.
    pub manifest: CourseManifest,
}

impl CourseBuilder {
    /// Writes the files needed for this course to the given directory.
    pub fn build(&self, parent_directory: &Path) -> Result<()> {
        // Verify that the directory doesn't already exist and create it.
        let course_directory = parent_directory.join(&self.directory_name);
        ensure!(
            !course_directory.is_dir(),
            "course directory {} already exists",
            course_directory.display(), // grcov-excl-line
        );
        create_dir_all(&course_directory)?;

        // Write all the assets.
        for builder in &self.assets {
            builder.build(&course_directory)?;
        }

        // For each lesson in the course, create a directory with the name
        // `<LESSON_SHORT_ID>.lesson` and build the lesson in that directory.
        for builder in &self.lessons {
            let lesson_directory =
                course_directory.join(format!("{}{}", builder.lesson.short_id, LESSON_SUFFIX));
            fs::create_dir_all(&lesson_directory)?;
            builder.build(&lesson_directory)?;
        }

        // Write the manifest to disk.
        let manifest_json = serde_json::to_string_pretty(&self.manifest)? + "\n";
        let manifest_path = course_directory.join("course_manifest.json");
        let mut manifest_file = File::create(manifest_path)?;
        manifest_file.write_all(manifest_json.as_bytes())?;
        Ok(())
    }
}

/// Represents a simple knowledge base exercise which only specifies the short ID of the exercise,
/// and the front and (optional) back of the card, which in a lot of cases are enough to deliver full
/// functionality of Trane. It is meant to help course authors write simple knowledge base courses by
/// writing a simple configuration to a single JSON file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimpleKnowledgeBaseExercise {
    /// The short ID of the exercise.
    pub short_id: String,

    /// The content of the front of the card.
    pub front: Vec<String>,

    /// The optional content of the back of the card. If the list is empty, no file will be created.
    pub back: Vec<String>,
}

impl SimpleKnowledgeBaseExercise {
    /// Generates the exercise builder for this simple knowledge base exercise.
    fn generate_exercise_builder(
        &self,
        short_lesson_id: Ustr,
        course_id: Ustr,
    ) -> Result<ExerciseBuilder> {
        // Ensure that the short ID is not empty.
        ensure!(!self.short_id.is_empty(), "short ID cannot be empty");

        // Generate the asset builders for the front and back of the card.
        let front_file = format!("{}{}", self.short_id, EXERCISE_FRONT_SUFFIX);
        let back_file = if !self.back.is_empty() {
            Some(format!("{}{}", self.short_id, EXERCISE_BACK_SUFFIX))
        } else {
            None
        };

        let mut asset_builders = vec![AssetBuilder {
            file_name: front_file.clone(),
            contents: self.front.join("\n"),
        }];
        if !self.back.is_empty() {
            asset_builders.push(AssetBuilder {
                file_name: back_file.clone().unwrap(),
                contents: self.back.join("\n"),
            })
        }

        // Generate the exercise builder.
        Ok(ExerciseBuilder {
            exercise: KnowledgeBaseExercise {
                short_id: self.short_id.to_string(),
                short_lesson_id,
                course_id,
                front_file,
                back_file,
                name: None,
                description: None,
                exercise_type: None,
            },
            asset_builders,
        })
    }
}

/// Represents a simple knowledge base lesson which only specifies the short ID of the lesson, the
/// dependencies of the lesson, and a list of simple exercises. The instructions, material, and
/// metadata can be optionally specified as well. In a lot of cases, this is enough to deliver the
/// full functionality of Trane. It is meant to help course authors write simple knowledge base
/// courses by writing a simple configuration to a single JSON file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimpleKnowledgeBaseLesson {
    /// The short ID of the lesson.
    pub short_id: Ustr,

    /// The dependencies of the lesson.
    pub dependencies: Vec<Ustr>,

    /// The simple exercises in the lesson.
    pub exercises: Vec<SimpleKnowledgeBaseExercise>,

    /// The optional instructions for the lesson.
    pub instructions: Option<String>,

    /// The optional material for the lesson.
    pub material: Option<String>,

    /// The optional metadata for the lesson.
    pub metadata: Option<BTreeMap<String, Vec<String>>>,

    /// A list of additional files to write in the lesson directory.
    pub additional_files: Vec<AssetBuilder>,
}

impl SimpleKnowledgeBaseLesson {
    /// Generates the lesson builder from this simple lesson.
    fn generate_lesson_builder(&self, course_id: Ustr) -> Result<LessonBuilder> {
        // Ensure that the lesson short ID is not empty and that the exercise short IDs are unique.
        ensure!(
            !self.short_id.is_empty(),
            "short ID of lesson cannot be empty"
        );
        let mut short_ids = HashSet::new();
        for exercise in &self.exercises {
            ensure!(
                !short_ids.contains(&exercise.short_id),
                "short ID {} of exercise is not unique",
                exercise.short_id
            );
            short_ids.insert(&exercise.short_id);
        }

        // Generate the exercise builders.
        let exercises = self
            .exercises
            .iter()
            .map(|exercise| exercise.generate_exercise_builder(self.short_id, course_id))
            .collect::<Result<Vec<_>>>()?;

        // Generate the assets for the instructions and material.
        let mut asset_builders = self.additional_files.clone();

        if let Some(instructions) = &self.instructions {
            asset_builders.push(AssetBuilder {
                file_name: LESSON_INSTRUCTIONS_FILE.into(),
                contents: instructions.clone(),
            })
        }
        if let Some(material) = &self.material {
            asset_builders.push(AssetBuilder {
                file_name: LESSON_MATERIAL_FILE.into(),
                contents: material.clone(),
            })
        }

        // Generate the lesson builder.
        let dependencies = if self.dependencies.is_empty() {
            None
        } else {
            Some(self.dependencies.clone())
        };
        let lesson_builder = LessonBuilder {
            lesson: KnowledgeBaseLesson {
                short_id: self.short_id,
                course_id,
                dependencies,
                name: None,
                description: None,
                metadata: self.metadata.clone(),
                has_instructions: self.instructions.is_some(),
                has_material: self.material.is_some(),
            },
            exercises,
            asset_builders,
        };
        Ok(lesson_builder)
    }
}

/// Represents a simple knowledge base course which only specifies the course manifest and a list of
/// simple lessons. It is meant to help course authors write simple knowledge base courses by
/// writing a simple configuration to a single JSON file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimpleKnowledgeBaseCourse {
    /// The manifest for this course.
    pub manifest: CourseManifest,

    /// The simple lessons in this course.
    pub lessons: Vec<SimpleKnowledgeBaseLesson>,
}

impl SimpleKnowledgeBaseCourse {
    /// Writes the course manifests and the lesson directories with the assets and exercises to the
    /// given directory.
    pub fn build(&self, root_directory: &Path) -> Result<()> {
        // Ensure that the lesson short IDs are unique.
        let mut short_ids = HashSet::new();
        for lesson in &self.lessons {
            ensure!(
                !short_ids.contains(&lesson.short_id),
                "short ID {} of lesson is not unique",
                lesson.short_id
            );
            short_ids.insert(&lesson.short_id);
        }

        // Generate the lesson builders.
        let lesson_builders = self
            .lessons
            .iter()
            .map(|lesson| lesson.generate_lesson_builder(self.manifest.id))
            .collect::<Result<Vec<_>>>()?;

        // Build the lessons in the course.
        for lesson_builder in lesson_builders {
            let lesson_directory = root_directory.join(format!(
                "{}{}",
                lesson_builder.lesson.short_id, LESSON_SUFFIX
            ));

            // Remove the lesson directories if they already exist.
            if lesson_directory.exists() {
                fs::remove_dir_all(&lesson_directory).with_context(|| {
                    // grcov-excl-start
                    format!(
                        "failed to remove existing lesson directory at {}",
                        lesson_directory.display()
                    )
                    // grcov-excl-stop
                })?; // grcov-excl-line
            }

            lesson_builder.build(&lesson_directory)?;
        }

        // Write the course manifest.
        let manifest_path = root_directory.join(COURSE_MANIFEST_FILENAME);
        let mut manifest_file = fs::File::create(&manifest_path).with_context(|| {
            // grcov-excl-start
            format!(
                "failed to create course manifest file at {}",
                manifest_path.display()
            )
            // grcov-excl-stop
        })?; // grcov-excl-line
        manifest_file
            .write_all(serde_json::to_string_pretty(&self.manifest)?.as_bytes())
            .with_context(|| {
                // grcov-excl-start
                format!(
                    "failed to write course manifest file at {}",
                    manifest_path.display()
                )
                // grcov-excl-stop
            }) // grcov-excl-line
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

    use anyhow::Result;

    use crate::{
        course_builder::knowledge_base_builder::*,
        data::{course_generator::knowledge_base::KnowledgeBaseFile, ExerciseType},
    };

    /// Creates a test lesson builder.
    fn test_lesson_builder() -> LessonBuilder {
        let exercise_builder = ExerciseBuilder {
            exercise: KnowledgeBaseExercise {
                short_id: "ex1".to_string(),
                short_lesson_id: "lesson1".into(),
                course_id: "course1".into(),
                front_file: "ex1.front.md".to_string(),
                back_file: Some("ex1.back.md".to_string()),
                name: Some("Exercise 1".to_string()),
                description: Some("Exercise 1 description".to_string()),
                exercise_type: Some(ExerciseType::Procedural),
            },
            asset_builders: vec![
                AssetBuilder {
                    file_name: "ex1.front.md".to_string(),
                    contents: "Exercise 1 front".to_string(),
                },
                AssetBuilder {
                    file_name: "ex1.back.md".to_string(),
                    contents: "Exercise 1 back".to_string(),
                },
            ],
        };
        LessonBuilder {
            lesson: KnowledgeBaseLesson {
                short_id: "lesson1".into(),
                course_id: "course1".into(),
                name: Some("Lesson 1".to_string()),
                description: Some("Lesson 1 description".to_string()),
                dependencies: Some(vec!["lesson2".into()]),
                metadata: Some(BTreeMap::from([(
                    "key".to_string(),
                    vec!["value".to_string()],
                )])),
                has_instructions: true,
                has_material: true,
            },
            exercises: vec![exercise_builder],
            asset_builders: vec![
                AssetBuilder {
                    file_name: LESSON_INSTRUCTIONS_FILE.to_string(),
                    contents: "Instructions".to_string(),
                },
                AssetBuilder {
                    file_name: LESSON_MATERIAL_FILE.to_string(),
                    contents: "Material".to_string(),
                },
            ],
        }
    }

    /// Verifies that the course builder writes the correct files to disk.
    #[test]
    fn course_builder() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let course_builder = CourseBuilder {
            directory_name: "course1".into(),
            manifest: CourseManifest {
                id: "course1".into(),
                name: "Course 1".into(),
                dependencies: vec![],
                description: None,
                authors: None,
                metadata: None,
                course_material: None,
                course_instructions: None,
                generator_config: None,
            },
            lessons: vec![test_lesson_builder()],
            assets: vec![
                AssetBuilder {
                    file_name: "course_instructions.md".to_string(),
                    contents: "Course Instructions".to_string(),
                },
                AssetBuilder {
                    file_name: "course_material.md".to_string(),
                    contents: "Course Material".to_string(),
                },
            ],
        };

        course_builder.build(temp_dir.path())?;

        // Verify that the exercise was built correctly.
        let course_dir = temp_dir.path().join("course1");
        let lesson_dir = course_dir.join("lesson1.lesson");
        assert!(lesson_dir.exists());
        let front_file = lesson_dir.join("ex1.front.md");
        assert!(front_file.exists());
        assert_eq!(fs::read_to_string(front_file)?, "Exercise 1 front");
        let back_file = lesson_dir.join("ex1.back.md");
        assert!(back_file.exists());
        assert_eq!(fs::read_to_string(back_file)?, "Exercise 1 back");
        let name_file = lesson_dir.join("ex1.name.json");
        assert!(name_file.exists());
        assert_eq!(KnowledgeBaseFile::open::<String>(&name_file)?, "Exercise 1",);
        let description_file = lesson_dir.join("ex1.description.json");
        assert!(description_file.exists());
        assert_eq!(
            KnowledgeBaseFile::open::<String>(&description_file)?,
            "Exercise 1 description",
        );
        let type_file = lesson_dir.join("ex1.type.json");
        assert!(type_file.exists());
        assert_eq!(
            KnowledgeBaseFile::open::<ExerciseType>(&type_file)?,
            ExerciseType::Procedural,
        );

        // Verify that the lesson was built correctly.
        let name_file = lesson_dir.join(LESSON_NAME_FILE);
        assert!(name_file.exists());
        assert_eq!(KnowledgeBaseFile::open::<String>(&name_file)?, "Lesson 1",);
        let description_file = lesson_dir.join(LESSON_DESCRIPTION_FILE);
        assert!(description_file.exists());
        assert_eq!(
            KnowledgeBaseFile::open::<String>(&description_file)?,
            "Lesson 1 description",
        );
        let dependencies_file = lesson_dir.join(LESSON_DEPENDENCIES_FILE);
        assert!(lesson_dir.join(LESSON_DEPENDENCIES_FILE).exists());
        assert_eq!(
            KnowledgeBaseFile::open::<Vec<String>>(&dependencies_file)?,
            vec!["lesson2".to_string()],
        );
        let metadata_file = lesson_dir.join(LESSON_METADATA_FILE);
        assert!(metadata_file.exists());
        assert_eq!(
            KnowledgeBaseFile::open::<BTreeMap<String, Vec<String>>>(&metadata_file)?,
            BTreeMap::from([("key".to_string(), vec!["value".to_string()])]),
        );
        let instructions_file = lesson_dir.join(LESSON_INSTRUCTIONS_FILE);
        assert!(instructions_file.exists());
        assert_eq!(fs::read_to_string(instructions_file)?, "Instructions",);
        let material_file = lesson_dir.join(LESSON_MATERIAL_FILE);
        assert!(material_file.exists());
        assert_eq!(fs::read_to_string(material_file)?, "Material",);

        // Verify that the course was built correctly.
        assert!(course_dir.join("course_manifest.json").exists());
        assert_eq!(
            KnowledgeBaseFile::open::<CourseManifest>(&course_dir.join("course_manifest.json"))
                .unwrap(),
            course_builder.manifest,
        );
        assert!(course_dir.join("course_instructions.md").exists());
        assert_eq!(
            fs::read_to_string(course_dir.join("course_instructions.md"))?,
            "Course Instructions",
        );
        assert!(course_dir.join("course_material.md").exists());
        assert_eq!(
            fs::read_to_string(course_dir.join("course_material.md"))?,
            "Course Material",
        );

        Ok(())
    }

    /// Verifies that the simple course builder writes the correct files to disk.
    #[test]
    fn build_simple_course() -> Result<()> {
        // Create a simple course. The first lesson sets up the minimum required fields for a
        // lesson, and the second lesson sets up all optional fields.
        let simple_course = SimpleKnowledgeBaseCourse {
            manifest: CourseManifest {
                id: "course1".into(),
                name: "Course 1".into(),
                dependencies: vec![],
                description: None,
                authors: None,
                metadata: None,
                course_material: None,
                course_instructions: None,
                generator_config: None,
            },
            lessons: vec![
                SimpleKnowledgeBaseLesson {
                    short_id: "1".into(),
                    dependencies: vec![],
                    exercises: vec![
                        SimpleKnowledgeBaseExercise {
                            short_id: "1".into(),
                            front: vec!["Lesson 1, Exercise 1 front".into()],
                            back: vec![],
                        },
                        SimpleKnowledgeBaseExercise {
                            short_id: "2".into(),
                            front: vec!["Lesson 1, Exercise 2 front".into()],
                            back: vec![],
                        },
                    ],
                    instructions: None,
                    material: None,
                    metadata: None,
                    additional_files: vec![],
                },
                SimpleKnowledgeBaseLesson {
                    short_id: "2".into(),
                    dependencies: vec!["1".into()],
                    exercises: vec![
                        SimpleKnowledgeBaseExercise {
                            short_id: "1".into(),
                            front: vec!["Lesson 2, Exercise 1 front".into()],
                            back: vec!["Lesson 2, Exercise 1 back".into()],
                        },
                        SimpleKnowledgeBaseExercise {
                            short_id: "2".into(),
                            front: vec!["Lesson 2, Exercise 2 front".into()],
                            back: vec!["Lesson 2, Exercise 2 back".into()],
                        },
                    ],
                    instructions: Some("Lesson 2 instructions".into()),
                    material: Some("Lesson 2 material".into()),
                    metadata: Some(BTreeMap::from([(
                        "key".to_string(),
                        vec!["value".to_string()],
                    )])),
                    additional_files: vec![AssetBuilder {
                        file_name: "dummy.md".into(),
                        contents: "I'm a dummy file".into(),
                    }],
                },
            ],
        };

        // Create a temp directory and one of the lesson directories with some content to ensure
        // that is deleted. Then build the course and verify the contents of the output directory.
        let temp_dir = tempfile::tempdir()?;
        let dummy_dir = temp_dir.path().join("1.lesson").join("dummy");
        fs::create_dir_all(&dummy_dir)?;
        assert!(dummy_dir.exists());
        simple_course.build(&temp_dir.path())?;
        assert!(!dummy_dir.exists());

        // Verify that the first lesson was built correctly.
        let lesson_dir = temp_dir.path().join("1.lesson");
        assert!(lesson_dir.exists());
        let front_file = lesson_dir.join("1.front.md");
        assert!(front_file.exists());
        assert_eq!(
            fs::read_to_string(&front_file)?,
            "Lesson 1, Exercise 1 front"
        );
        let front_file = lesson_dir.join("2.front.md");
        assert!(front_file.exists());
        assert_eq!(
            fs::read_to_string(&front_file)?,
            "Lesson 1, Exercise 2 front"
        );
        let dependencies_file = lesson_dir.join(LESSON_DEPENDENCIES_FILE);
        assert!(!dependencies_file.exists());
        let instructions_file = lesson_dir.join(LESSON_INSTRUCTIONS_FILE);
        assert!(!instructions_file.exists());
        let material_file = lesson_dir.join(LESSON_MATERIAL_FILE);
        assert!(!material_file.exists());

        // Verify that the second lesson was built correctly.
        let lesson_dir = temp_dir.path().join("2.lesson");
        assert!(lesson_dir.exists());
        let front_file = lesson_dir.join("1.front.md");
        assert!(front_file.exists());
        assert_eq!(
            fs::read_to_string(&front_file)?,
            "Lesson 2, Exercise 1 front"
        );
        let back_file = lesson_dir.join("1.back.md");
        assert!(back_file.exists());
        assert_eq!(fs::read_to_string(&back_file)?, "Lesson 2, Exercise 1 back");
        let front_file = lesson_dir.join("2.front.md");
        assert!(front_file.exists());
        assert_eq!(
            fs::read_to_string(&front_file)?,
            "Lesson 2, Exercise 2 front"
        );
        let back_file = lesson_dir.join("2.back.md");
        assert!(back_file.exists());
        assert_eq!(fs::read_to_string(&back_file)?, "Lesson 2, Exercise 2 back");
        let dependencies_file = lesson_dir.join(LESSON_DEPENDENCIES_FILE);
        assert!(dependencies_file.exists());
        assert_eq!(
            KnowledgeBaseFile::open::<Vec<String>>(&dependencies_file)?,
            vec!["1".to_string()]
        );
        let instructions_file = lesson_dir.join(LESSON_INSTRUCTIONS_FILE);
        assert!(instructions_file.exists());
        assert_eq!(
            fs::read_to_string(&instructions_file)?,
            "Lesson 2 instructions"
        );
        let material_file = lesson_dir.join(LESSON_MATERIAL_FILE);
        assert!(material_file.exists());
        assert_eq!(fs::read_to_string(&material_file)?, "Lesson 2 material");
        let metadata_file = lesson_dir.join(LESSON_METADATA_FILE);
        assert!(metadata_file.exists());
        assert_eq!(
            KnowledgeBaseFile::open::<BTreeMap<String, Vec<String>>>(&metadata_file)?,
            BTreeMap::from([("key".to_string(), vec!["value".to_string()])])
        );
        let dummy_file = lesson_dir.join("dummy.md");
        assert!(dummy_file.exists());
        assert_eq!(fs::read_to_string(&dummy_file)?, "I'm a dummy file");

        // Finally, clone the simple knowledge course to satisfy the code coverage check.
        assert_eq!(simple_course.clone(), simple_course);
        Ok(())
    }

    // Verifies that the simple knowledge course checks for duplicate lesson IDs.
    #[test]
    fn duplicate_short_lesson_ids() -> Result<()> {
        // Build a simple course with duplicate lesson IDs.
        let simple_course = SimpleKnowledgeBaseCourse {
            manifest: CourseManifest {
                id: "course1".into(),
                name: "Course 1".into(),
                dependencies: vec![],
                description: None,
                authors: None,
                metadata: None,
                course_material: None,
                course_instructions: None,
                generator_config: None,
            },
            lessons: vec![
                SimpleKnowledgeBaseLesson {
                    short_id: "1".into(),
                    dependencies: vec![],
                    exercises: vec![SimpleKnowledgeBaseExercise {
                        short_id: "1".into(),
                        front: vec!["Lesson 1, Exercise 1 front".into()],
                        back: vec![],
                    }],
                    instructions: None,
                    material: None,
                    metadata: None,
                    additional_files: vec![],
                },
                SimpleKnowledgeBaseLesson {
                    short_id: "1".into(),
                    dependencies: vec![],
                    exercises: vec![SimpleKnowledgeBaseExercise {
                        short_id: "1".into(),
                        front: vec!["Lesson 2, Exercise 1 front".into()],
                        back: vec![],
                    }],
                    instructions: None,
                    material: None,
                    metadata: None,
                    additional_files: vec![],
                },
            ],
        };

        // Verify that the course builder fails.
        let temp_dir = tempfile::tempdir()?;
        assert!(simple_course.build(&temp_dir.path()).is_err());
        Ok(())
    }

    // Verifies that the simple knowledge course checks for duplicate exercise IDs.
    #[test]
    fn duplicate_short_exercise_ids() -> Result<()> {
        // Build a simple course with duplicate exercise IDs.
        let simple_course = SimpleKnowledgeBaseCourse {
            manifest: CourseManifest {
                id: "course1".into(),
                name: "Course 1".into(),
                dependencies: vec![],
                description: None,
                authors: None,
                metadata: None,
                course_material: None,
                course_instructions: None,
                generator_config: None,
            },
            lessons: vec![SimpleKnowledgeBaseLesson {
                short_id: "1".into(),
                dependencies: vec![],
                exercises: vec![
                    SimpleKnowledgeBaseExercise {
                        short_id: "1".into(),
                        front: vec!["Lesson 1, Exercise 1 front".into()],
                        back: vec![],
                    },
                    SimpleKnowledgeBaseExercise {
                        short_id: "1".into(),
                        front: vec!["Lesson 1, Exercise 2 front".into()],
                        back: vec![],
                    },
                ],
                instructions: None,
                material: None,
                metadata: None,
                additional_files: vec![],
            }],
        };

        // Verify that the course builder fails.
        let temp_dir = tempfile::tempdir()?;
        assert!(simple_course.build(&temp_dir.path()).is_err());
        Ok(())
    }

    // Verifies that the simple knowledge course checks empty lesson IDs.
    #[test]
    fn empty_short_lesson_ids() -> Result<()> {
        // Build a simple course with empty lesson IDs.
        let simple_course = SimpleKnowledgeBaseCourse {
            manifest: CourseManifest {
                id: "course1".into(),
                name: "Course 1".into(),
                dependencies: vec![],
                description: None,
                authors: None,
                metadata: None,
                course_material: None,
                course_instructions: None,
                generator_config: None,
            },
            lessons: vec![SimpleKnowledgeBaseLesson {
                short_id: "".into(),
                dependencies: vec![],
                exercises: vec![SimpleKnowledgeBaseExercise {
                    short_id: "1".into(),
                    front: vec!["Lesson 1, Exercise 1 front".into()],
                    back: vec![],
                }],
                instructions: None,
                material: None,
                metadata: None,
                additional_files: vec![],
            }],
        };

        // Verify that the course builder fails.
        let temp_dir = tempfile::tempdir()?;
        assert!(simple_course.build(&temp_dir.path()).is_err());
        Ok(())
    }

    // Verifies that the simple knowledge course checks empty exercise IDs.
    #[test]
    fn empty_short_exercise_ids() -> Result<()> {
        // Build a simple course with empty exercise IDs.
        let simple_course = SimpleKnowledgeBaseCourse {
            manifest: CourseManifest {
                id: "course1".into(),
                name: "Course 1".into(),
                dependencies: vec![],
                description: None,
                authors: None,
                metadata: None,
                course_material: None,
                course_instructions: None,
                generator_config: None,
            },
            lessons: vec![SimpleKnowledgeBaseLesson {
                short_id: "1".into(),
                dependencies: vec![],
                exercises: vec![SimpleKnowledgeBaseExercise {
                    short_id: "".into(),
                    front: vec!["Lesson 1, Exercise 1 front".into()],
                    back: vec![],
                }],
                instructions: None,
                material: None,
                metadata: None,
                additional_files: vec![],
            }],
        };

        // Verify that the course builder fails.
        let temp_dir = tempfile::tempdir()?;
        assert!(simple_course.build(&temp_dir.path()).is_err());
        Ok(())
    }
}
