//! Contains utilities to make it easier to build knowledge base courses.
//!
//! The knowledge base course format is a plain-text format that is intended to be easy to edit by
//! hand. This module contains utilities to make it easier to generate these files, specially for
//! testing purposes.

use anyhow::Result;
use std::{
    fs::{self, File},
    io::Write,
    path::Path,
};

use crate::{
    course_builder::AssetBuilder,
    data::{
        course_generator::knowledge_base::{
            KnowledgeBaseExercise, KnowledgeBaseLesson, EXERCISE_DESCRIPTION_SUFFIX,
            EXERCISE_NAME_SUFFIX, EXERCISE_TYPE_SUFFIX, LESSON_DEPENDENCIES_FILE,
            LESSON_DESCRIPTION_FILE, LESSON_INSTRUCTIONS_FILE, LESSON_MATERIAL_FILE,
            LESSON_METADATA_FILE, LESSON_NAME_FILE, LESSON_SUFFIX,
        },
        CourseManifest,
    },
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
    pub assets: Vec<AssetBuilder>,
}

impl LessonBuilder {
    /// Writes the files needed for this lesson to the given directory.
    pub fn build(&self, lesson_directory: &Path) -> Result<()> {
        // Build all the assets.
        for builder in &self.assets {
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
        if let Some(instructions) = &self.lesson.instructions {
            let instructions_json = serde_json::to_string_pretty(instructions)?;
            let instructions_path = lesson_directory.join(LESSON_INSTRUCTIONS_FILE);
            let mut instructions_file = File::create(instructions_path)?;
            instructions_file.write_all(instructions_json.as_bytes())?;
        }
        if let Some(material) = &self.lesson.material {
            let material_json = serde_json::to_string_pretty(material)?;
            let material_path = lesson_directory.join(LESSON_MATERIAL_FILE);
            let mut material_file = File::create(material_path)?;
            material_file.write_all(material_json.as_bytes())?;
        }
        Ok(())
    }
}

/// A builder to generate a knowledge base course and associated assets.
pub struct CourseBuilder {
    /// The builders for the lessons in this course.
    pub lessons: Vec<LessonBuilder>,

    /// The assets associated with this course.
    pub assets: Vec<AssetBuilder>,

    /// The manifest for this course.
    pub manifest: CourseManifest,
}

impl CourseBuilder {
    /// Writes the files needed for this course to the given directory.
    pub fn build(&self, course_directory: &Path) -> Result<()> {
        // Write all the assets.
        for builder in &self.assets {
            builder.build(course_directory)?;
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
