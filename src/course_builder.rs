//! Defines utilities to make it easier to generate courses and lessons.
//!
//! Courses, lessons, and exercises are stored in JSON files that are the serialized versions of the
//! manifests in the `data` module. This means that writers of Trane courses can simply generate the
//! files by hand. However, this process is tedious and error-prone, so this module provides
//! utilities to make it easier to generate these files. In addition, Trane is in early stages of
//! development, so the format of the manifests is not stable yet. Generating the files by code
//! makes it easier to make updates to the files as the format changes.

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

pub mod knowledge_base_builder;
#[cfg_attr(coverage, coverage(off))]
pub mod music;

use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{create_dir_all, File},
    io::Write,
    path::{Path, PathBuf},
};
use strum::Display;

use crate::data::{CourseManifest, ExerciseManifestBuilder, LessonManifestBuilder, VerifyPaths};

/// Common metadata keys for all courses and lessons.
#[derive(Display)]
#[strum(serialize_all = "snake_case")]
#[allow(missing_docs)]
pub enum TraneMetadata {
    Skill,
}

/// A builder to generate plain-text asset files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetBuilder {
    /// The name of the file, which will be joined with the directory passed in the build function.
    pub file_name: String,

    /// The contents of the file as a string.
    pub contents: String,
}

impl AssetBuilder {
    /// Writes the asset to the given directory.
    pub fn build(&self, asset_directory: &Path) -> Result<()> {
        // Create the asset directory and verify there's not an existing file with the same name.
        create_dir_all(asset_directory)?;
        let asset_path = asset_directory.join(&self.file_name);
        ensure!(
            !asset_path.exists(),
            "asset path {} already exists",
            asset_path.display()
        );

        // Create any parent directories to the asset path to support specifying a directory in the
        // path.
        create_dir_all(asset_path.parent().unwrap())?;

        // Write the asset file.
        let mut asset_file = File::create(asset_path)?;
        asset_file.write_all(self.contents.as_bytes())?;
        Ok(())
    }
}

/// A builder that generates all the files needed to add an exercise to a lesson.
pub struct ExerciseBuilder {
    /// The base name of the directory on which to store this lesson.
    pub directory_name: String,

    /// A closure taking a builder common to all exercises which returns the builder for a specific
    /// exercise manifest.
    pub manifest_closure: Box<dyn Fn(ExerciseManifestBuilder) -> ExerciseManifestBuilder>,

    /// A list of asset builders to create assets specific to this exercise.
    pub asset_builders: Vec<AssetBuilder>,
}

impl ExerciseBuilder {
    /// Writes the files needed for this exercise to the given directory.
    pub fn build(
        &self,
        exercise_directory: &PathBuf,
        manifest_template: ExerciseManifestBuilder,
    ) -> Result<()> {
        // Create the directory and write the exercise manifest.
        create_dir_all(exercise_directory)?;
        let manifest = (self.manifest_closure)(manifest_template).build()?;
        let manifest_json = serde_json::to_string_pretty(&manifest)? + "\n";
        let manifest_path = exercise_directory.join("exercise_manifest.json");
        let mut manifest_file = File::create(manifest_path)?;
        manifest_file.write_all(manifest_json.as_bytes())?;

        // Write all the assets.
        for asset_builder in &self.asset_builders {
            asset_builder.build(exercise_directory)?;
        }

        // Verify that all paths mentioned in the manifest are valid.
        manifest.verify_paths(exercise_directory).context(format!(
            "failed to verify files for exercise {}",
            manifest.id
        ))?;
        Ok(())
    }
}

/// A builder that generates the files needed to add a lesson to a course.
pub struct LessonBuilder {
    /// Base name of the directory on which to store this lesson.
    pub directory_name: String,

    /// A closure taking a builder common to all lessons which returns the builder for a specific
    /// lesson manifest.
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
        // Create the directory and write the lesson manifest.
        create_dir_all(lesson_directory)?;
        let manifest = (self.manifest_closure)(manifest_template).build()?;
        let manifest_json = serde_json::to_string_pretty(&manifest)? + "\n";
        let manifest_path = lesson_directory.join("lesson_manifest.json");
        let mut manifest_file = File::create(manifest_path)?;
        manifest_file.write_all(manifest_json.as_bytes())?;

        // Write all the assets.
        for asset_builder in &self.asset_builders {
            asset_builder.build(lesson_directory)?;
        }

        // Build all the exercises in the lesson.
        for exercise_builder in &self.exercise_builders {
            let exercise_directory = lesson_directory.join(&exercise_builder.directory_name);
            exercise_builder.build(&exercise_directory, self.exercise_manifest_template.clone())?;
        }

        // Verify that all paths mentioned in the manifest are valid.
        ensure!(
            manifest.verify_paths(lesson_directory)?,
            "cannot verify files mentioned in the manifest for lesson {}",
            manifest.id,
        );
        Ok(())
    }
}

/// A builder that generates the files needed to add a course.
pub struct CourseBuilder {
    /// Base name of the directory on which to store this course.
    pub directory_name: String,

    /// The manifest for the course.
    pub course_manifest: CourseManifest,

    /// A template builder used to build the manifests for each lesson in the course. Attributes
    /// common to all lessons should be set here.
    pub lesson_manifest_template: LessonManifestBuilder,

    /// A list of tuples of directory names and lesson builders to create the lessons in the
    /// course.
    pub lesson_builders: Vec<LessonBuilder>,

    /// A list of asset builders to create assets specific to this course.
    pub asset_builders: Vec<AssetBuilder>,
}

impl CourseBuilder {
    /// Writes the files needed for this course to the given directory.
    pub fn build(&self, parent_directory: &Path) -> Result<()> {
        // Create the directory and write the course manifest.
        let course_directory = parent_directory.join(&self.directory_name);
        create_dir_all(&course_directory)?;
        let manifest_json = serde_json::to_string_pretty(&self.course_manifest)? + "\n";
        let manifest_path = course_directory.join("course_manifest.json");
        let mut manifest_file = File::create(manifest_path)?;
        manifest_file.write_all(manifest_json.as_bytes())?;

        // Write all the assets.
        for asset_builder in &self.asset_builders {
            asset_builder.build(&course_directory)?;
        }

        // Build all the lessons in the course.
        for lesson_builder in &self.lesson_builders {
            let lesson_directory = course_directory.join(&lesson_builder.directory_name);
            lesson_builder.build(&lesson_directory, self.lesson_manifest_template.clone())?;
        }

        // Verify that all paths mentioned in the manifest are valid.
        ensure!(
            self.course_manifest
                .verify_paths(course_directory.as_path())?,
            "cannot verify files mentioned in the manifest for course {}",
            self.course_manifest.id,
        );
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use std::io::Read;

    use super::*;
    use crate::data::{BasicAsset, ExerciseAsset, ExerciseType};

    /// Verifies the asset builder writes the contents to the correct file.
    #[test]
    fn asset_builer() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let asset_builder = AssetBuilder {
            file_name: "asset1.md".to_string(),
            contents: "asset1 contents".to_string(),
        };
        asset_builder.build(temp_dir.path())?;
        assert!(temp_dir.path().join("asset1.md").is_file());
        let mut file = File::open(temp_dir.path().join("asset1.md"))?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        assert_eq!(contents, "asset1 contents");
        Ok(())
    }

    /// Verifies the course builder writes the correct files.
    #[test]
    fn course_builder() -> Result<()> {
        let exercise_builder = ExerciseBuilder {
            directory_name: "exercise1".to_string(),
            manifest_closure: Box::new(|builder| {
                builder
                    .clone()
                    .id("exercise1")
                    .name("Exercise 1".into())
                    .exercise_asset(ExerciseAsset::BasicAsset(BasicAsset::InlinedAsset {
                        content: String::new(),
                    }))
                    .clone()
            }),
            asset_builders: vec![],
        };
        let lesson_builder = LessonBuilder {
            directory_name: "lesson1".to_string(),
            manifest_closure: Box::new(|builder| {
                builder
                    .clone()
                    .id("lesson1")
                    .name("Lesson 1".into())
                    .dependencies(vec![])
                    .clone()
            }),
            exercise_manifest_template: ExerciseManifestBuilder::default()
                .lesson_id("lesson1")
                .course_id("course1")
                .exercise_type(ExerciseType::Procedural)
                .clone(),
            exercise_builders: vec![exercise_builder],
            asset_builders: vec![],
        };
        let course_builder = CourseBuilder {
            directory_name: "course1".to_string(),
            course_manifest: CourseManifest {
                id: "course1".into(),
                name: "Course 1".into(),
                dependencies: vec![],
                superseded: vec![],
                description: None,
                authors: None,
                metadata: None,
                course_material: None,
                course_instructions: None,
                generator_config: None,
            },
            lesson_manifest_template: LessonManifestBuilder::default()
                .course_id("course1")
                .clone(),
            lesson_builders: vec![lesson_builder],
            asset_builders: vec![],
        };

        let temp_dir = tempfile::tempdir()?;
        course_builder.build(temp_dir.path())?;

        let course_dir = temp_dir.path().join("course1");
        let lesson_dir = course_dir.join("lesson1");
        let exercise_dir = lesson_dir.join("exercise1");
        assert!(course_dir.is_dir());
        assert!(lesson_dir.is_dir());
        assert!(exercise_dir.is_dir());
        assert!(course_dir.join("course_manifest.json").is_file());
        assert!(lesson_dir.join("lesson_manifest.json").is_file());
        assert!(exercise_dir.join("exercise_manifest.json").is_file());
        Ok(())
    }
}
