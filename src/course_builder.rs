//! Module defining utilities to make it easier to generate courses and lessons.
pub mod music;

use std::{
    fs::{create_dir_all, File},
    io::Write,
    path::{Path, PathBuf},
};

use crate::data::{CourseManifest, ExerciseManifestBuilder, LessonManifestBuilder, VerifyPaths};
use anyhow::{ensure, Result};
use strum::Display;

/// Common metadata keys for all courses and lessons.
#[derive(Display)]
#[strum(serialize_all = "snake_case")]
#[allow(missing_docs)]
pub enum TraneMetadata {
    Skill,
}

/// Builds plain-text asset files.
#[derive(Clone)]
pub struct AssetBuilder {
    /// The name of the file, which will be joined with the directory passed in the build function.
    pub file_name: String,

    /// The contents of the file as a string.
    pub contents: String,
}

impl AssetBuilder {
    /// Writes the asset to the given directory.
    pub fn build(&self, asset_directory: &PathBuf) -> Result<()> {
        create_dir_all(asset_directory)?;
        let asset_path = asset_directory.join(&self.file_name);
        ensure!(
            !asset_path.exists(),
            "asset path {} already exists",
            asset_path.display()
        );
        let mut asset_file = File::create(asset_path)?;
        asset_file.write_all(self.contents.as_bytes())?;
        Ok(())
    }
}

/// Builds the files needed to add an exercise to a lesson.
pub struct ExerciseBuilder {
    /// The base name of the directory on which to store this lesson.
    pub directory_name: String,

    /// A closure taking a template builder which returns the builder for the exercise manifest.
    pub manifest_closure: Box<dyn Fn(ExerciseManifestBuilder) -> ExerciseManifestBuilder>,

    /// A list of asset builders to create assets specific to this exercise.
    pub asset_builders: Vec<AssetBuilder>,
}

impl ExerciseBuilder {
    /// Writes the files needed for this exercises to the given directory.
    pub fn build(
        &self,
        exercise_directory: &PathBuf,
        manifest_template: ExerciseManifestBuilder,
    ) -> Result<()> {
        ensure!(
            !exercise_directory.is_dir(),
            "exercise directory {} already exists",
            exercise_directory.display(),
        );
        create_dir_all(exercise_directory)?;

        let manifest = (self.manifest_closure)(manifest_template).build()?;
        let manifest_json = serde_json::to_string_pretty(&manifest)? + "\n";
        let manifest_path = exercise_directory.join("exercise_manifest.json");
        let mut manifest_file = File::create(manifest_path)?;
        manifest_file.write_all(manifest_json.as_bytes())?;

        for asset_builder in &self.asset_builders {
            asset_builder.build(exercise_directory)?;
        }

        ensure! {
            manifest.verify_paths(exercise_directory)?,
            "cannot verify files mentioned in the manifest for exercise {}",
            manifest.id,
        };
        Ok(())
    }
}

/// Builds the files needed to add a lesson to a course.
pub struct LessonBuilder {
    /// Base name of the directory on which to store this lesson.
    pub directory_name: String,

    /// A closure taking a template builder which returns the builder for the lesson manifest.
    pub manifest_closure: Box<dyn Fn(LessonManifestBuilder) -> LessonManifestBuilder>,

    /// A template builder used to build the manifests for each exercise in the lesson. Common
    /// attributes to all exercises should be set here.
    pub exercise_manifest_template: ExerciseManifestBuilder,

    /// A list of tuples of exercise directory name and exercise builder to create the exercises in
    /// the lesson.
    pub exercise_builders: Vec<ExerciseBuilder>,

    /// A list of asset builders to create assets specific to this lesson.
    pub asset_builders: Vec<AssetBuilder>,
}

impl LessonBuilder {
    /// Writes the files needed for this lesson to the given directory.
    pub fn build(
        &self,
        lesson_directory: &PathBuf,
        manifest_template: LessonManifestBuilder,
    ) -> Result<()> {
        ensure!(
            !lesson_directory.is_dir(),
            "lesson directory {} already exists",
            lesson_directory.display(),
        );
        create_dir_all(lesson_directory)?;

        let manifest = (self.manifest_closure)(manifest_template).build()?;
        let manifest_json = serde_json::to_string_pretty(&manifest)? + "\n";
        let manifest_path = lesson_directory.join("lesson_manifest.json");
        let mut manifest_file = File::create(manifest_path)?;
        manifest_file.write_all(manifest_json.as_bytes())?;

        for asset_builder in &self.asset_builders {
            asset_builder.build(lesson_directory)?;
        }

        for exercise_builder in &self.exercise_builders {
            let exercise_directory = lesson_directory.join(&exercise_builder.directory_name);
            exercise_builder.build(&exercise_directory, self.exercise_manifest_template.clone())?;
        }

        ensure! {
            manifest.verify_paths(lesson_directory)?,
            "cannot verify files mentioned in the manifest for lesson {}",
            manifest.id,
        };
        Ok(())
    }
}

/// Builds the files needed to add a course.
pub struct CourseBuilder {
    /// Base name of the directory on which to store this lesson.
    pub directory_name: String,

    /// The manifest for the course.
    pub course_manifest: CourseManifest,

    /// A template builder used to build the manifests for each lesson in the course. Attributes
    /// common to all lessons should be set here.
    pub lesson_manifest_template: LessonManifestBuilder,

    /// A list of tuples of lesson directory name and lesson builder to create the lessons in the
    /// course.
    pub lesson_builders: Vec<LessonBuilder>,

    /// A list of asset builders to create assets specific to this course.
    pub asset_builders: Vec<AssetBuilder>,
}

impl CourseBuilder {
    /// Writes the files needed for this course to the given directory.
    pub fn build(&self, parent_directory: &Path) -> Result<()> {
        let course_directory = parent_directory.join(&self.directory_name);
        ensure!(
            !course_directory.is_dir(),
            "course directory {} already exists",
            course_directory.display(),
        );
        create_dir_all(&course_directory)?;

        let manifest_json = serde_json::to_string_pretty(&self.course_manifest)? + "\n";
        let manifest_path = course_directory.join("course_manifest.json");
        let mut manifest_file = File::create(manifest_path)?;
        manifest_file.write_all(manifest_json.as_bytes())?;

        for asset_builder in &self.asset_builders {
            asset_builder.build(&course_directory)?;
        }

        for lesson_builder in &self.lesson_builders {
            let lesson_directory = course_directory.join(&lesson_builder.directory_name);
            lesson_builder.build(&lesson_directory, self.lesson_manifest_template.clone())?;
        }

        ensure! {
            self.course_manifest.verify_paths(parent_directory)?,
            "cannot verify files mentioned in the manifest for course {}",
            self.course_manifest.id,
        };
        Ok(())
    }
}
