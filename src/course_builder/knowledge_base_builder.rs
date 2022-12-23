use anyhow::Result;
use std::path::Path;

use crate::{
    course_builder::AssetBuilder,
    data::{
        course_generator::knowledge_base::{KnowledgeBaseExercise, KnowledgeBaseLesson},
        CourseManifest,
    },
};

pub struct ExerciseBuilder {
    pub exercise: KnowledgeBaseExercise,
    pub assets: Vec<AssetBuilder>,
}

impl ExerciseBuilder {
    /// Writes the files needed for this exercise to the given directory.
    pub fn build(&self, _parent_directory: &Path) -> Result<()> {
        unimplemented!()
    }
}

pub struct LessonBuilder {
    pub lesson: KnowledgeBaseLesson,
    pub exercises: Vec<ExerciseBuilder>,
    pub assets: Vec<AssetBuilder>,
}

impl LessonBuilder {
    /// Writes the files needed for this lesson to the given directory.
    pub fn build(&self, _parent_directory: &Path) -> Result<()> {
        unimplemented!()
    }
}

pub struct CourseBuilder {
    pub lessons: Vec<LessonBuilder>,
    pub assets: Vec<AssetBuilder>,
    pub manifest: CourseManifest,
}

impl CourseBuilder {
    /// Writes the files needed for this course to the given directory.
    pub fn build(&self, _parent_directory: &Path) -> Result<()> {
        unimplemented!()
    }
}
