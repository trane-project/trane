//! Contains the logic for generating a course to learn a piece of music.
//!
//! Given a piece of music and the passages and sub-passages in which it is divided, this module
//! generates a course that allows the user to learn the piece of music by first practicing the
//! smallest passages and then working up until the full piece is mastered.

use anyhow::Result;
use indoc::formatdoc;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};
use ustr::Ustr;

use crate::data::{
    BasicAsset, CourseManifest, ExerciseAsset, ExerciseManifest, ExerciseType, GenerateManifests,
    GeneratedCourse, LessonManifest, UserPreferences,
};

/// Represents a music asset to be practiced.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum MusicAsset {
    /// A link to a SoundSlice.
    SoundSlice(String),

    /// The path to a local file. For example, the path to a PDF of the sheet music.
    LocalFile(String),
}

impl MusicAsset {
    /// Generates an exercise asset from this music asset.
    pub fn generate_exercise_asset(&self, start: &str, end: &str) -> ExerciseAsset {
        match self {
            MusicAsset::SoundSlice(url) => {
                let description = formatdoc! {"
                    Play the following passage of music in the piece.

                        Start: {}
                        End: {}
                ", start, end};
                ExerciseAsset::SoundSliceAsset {
                    link: url.clone(),
                    description: Some(description),
                    backup: None,
                }
            }
            MusicAsset::LocalFile(path) => {
                let description = formatdoc! {"
                    Play the following passage of music in the piece.
                    - Start: {}
                    -End: {}
                    
                    The file containing the music sheet is located at {}.
                ", path, start, end};
                ExerciseAsset::BasicAsset(BasicAsset::InlinedAsset {
                    content: description,
                })
            }
        }
    }
}

/// Represents a music passage to be practiced.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum MusicPassage {
    /// A single passage with no dependencies.
    SimplePassage {
        /// The start of the passage. For example, "Measure 1", "Measure 3, beat 2", "Start of third
        /// movement", etc. Start and end values are stored as strings to allow for flexibility in
        /// defining their values.
        start: String,

        /// The end of the passage. For example, "End of measure 3", "End of movement 2", etc. Just
        /// like start, it's defined as a string to allow for flexibility.
        end: String,
    },

    /// A passage that requires mastery of other smaller passages. For example, a three-movement
    /// piano concerto requires that the musician masters each of the individual movements.
    ComplexPassage {
        /// The start of the passage.
        start: String,

        /// The end of the passage.
        end: String,

        /// The passages that must be mastered before this passage can be mastered. Each passage
        /// should be given a unique index which will be used to generate the lesson ID. Those
        /// values should not change once they are defined or progress for this lesson will be lost.
        dependencies: HashMap<usize, MusicPassage>,
    },
}

impl MusicPassage {
    /// Retrieves the dependencies of this passage.
    fn passage_dependencies(&self) -> Option<&HashMap<usize, MusicPassage>> {
        match self {
            MusicPassage::SimplePassage { .. } => None,
            MusicPassage::ComplexPassage { dependencies, .. } => Some(dependencies),
        }
    }

    /// Retrieve the start of the passage.
    fn passage_start(&self) -> &str {
        match self {
            MusicPassage::SimplePassage { start, .. } => start,
            MusicPassage::ComplexPassage { start, .. } => start,
        }
    }

    /// Retrieve the end of the passage.
    fn passage_end(&self) -> &str {
        match self {
            MusicPassage::SimplePassage { end, .. } => end,
            MusicPassage::ComplexPassage { end, .. } => end,
        }
    }

    /// Generates the lesson ID for this course and passage, identified by the given path.
    pub fn generate_lesson_id(
        &self,
        course_manifest: &CourseManifest,
        passage_path: Vec<usize>,
    ) -> Ustr {
        // An empty passage path means the course consists of only a lesson. Give this lesson a
        // hard-coded ID.
        if passage_path.is_empty() {
            return Ustr::from(&format!("{}::lesson", course_manifest.id));
        }

        // Otherwise, generate the lesson ID from the passage path.
        let mut lesson_id = "".to_string();
        for index in passage_path {
            lesson_id.push_str(&format!("::{}", index));
        }
        Ustr::from(&format!("{}::{}", course_manifest.id, lesson_id))
    }

    /// Generates the lesson and exercise manifests for this passage, recursively doing so if the
    /// dependencies are not empty.
    pub fn generate_lesson_helper(
        &self,
        course_manifest: &CourseManifest,
        passage_path: Vec<usize>,
        dependencies: Option<&HashMap<usize, MusicPassage>>,
        music_asset: &MusicAsset,
    ) -> Vec<(LessonManifest, Vec<ExerciseManifest>)> {
        // Recursively generate the dependency lessons and IDs.
        let mut lessons = vec![];
        let mut dependency_ids = vec![];
        if let Some(dependencies) = dependencies {
            for (index, dependency) in dependencies {
                // Create the dependency path.
                let mut dependency_path = passage_path.clone();
                dependency_path.push(*index);

                // Generate the dependency ID and lessons.
                dependency_ids
                    .push(dependency.generate_lesson_id(course_manifest, dependency_path.clone()));
                lessons.append(&mut dependency.generate_lesson_helper(
                    course_manifest,
                    dependency_path,
                    dependency.passage_dependencies(),
                    music_asset,
                ));
            }
        }

        // Create the lesson and exercise manifests for this passage and add them to the list.
        let lesson_manifest = LessonManifest {
            id: self.generate_lesson_id(course_manifest, passage_path),
            course_id: course_manifest.id,
            name: course_manifest.name.clone(),
            description: None,
            dependencies: dependency_ids,
            metadata: None,
            lesson_instructions: None,
            lesson_material: None,
        };
        let exercise_manifest = ExerciseManifest {
            id: Ustr::from(&format!("{}::exercise", lesson_manifest.id)),
            lesson_id: lesson_manifest.id,
            course_id: course_manifest.id,
            name: course_manifest.name.clone(),
            description: None,
            exercise_type: ExerciseType::Procedural,
            exercise_asset: music_asset
                .generate_exercise_asset(self.passage_start(), self.passage_end()),
        };
        lessons.push((lesson_manifest, vec![exercise_manifest]));

        lessons
    }

    /// Generates the lesson and exercise manifests for this passage.
    pub fn generate_lessons(
        &self,
        course_manifest: &CourseManifest,
        music_asset: &MusicAsset,
    ) -> Vec<(LessonManifest, Vec<ExerciseManifest>)> {
        match &self {
            MusicPassage::SimplePassage { .. } => {
                self.generate_lesson_helper(course_manifest, vec![], None, music_asset)
            }
            MusicPassage::ComplexPassage { dependencies, .. } => self.generate_lesson_helper(
                course_manifest,
                vec![0],
                Some(dependencies),
                music_asset,
            ),
        }
    }
}

/// The config to create a course that teaches a piece of music.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MusicPieceConfig {
    /// The asset containing the music to be practiced.
    pub music_asset: MusicAsset,

    /// The passages in which the music is divided for practice.
    pub passages: MusicPassage,
}

impl GenerateManifests for MusicPieceConfig {
    fn generate_manifests(
        &self,
        _course_root: &Path,
        course_manifest: &CourseManifest,
        _preferences: &UserPreferences,
    ) -> Result<GeneratedCourse> {
        Ok(GeneratedCourse {
            lessons: self
                .passages
                .generate_lessons(course_manifest, &self.music_asset),
            updated_instructions: None,
            updated_metadata: None,
        })
    }
}
