//! Contains the logic for generating a course to learn a piece of music.
//!
//! Given a piece of music and the passages and sub-passages in which it is divided, this module
//! generates a course that allows the user to learn the piece of music by first practicing the
//! smallest passages and then working up until the full piece is mastered.

use anyhow::Result;
use indoc::{formatdoc, indoc};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};
use ustr::Ustr;

use crate::data::{
    BasicAsset, CourseManifest, ExerciseAsset, ExerciseManifest, ExerciseType, GenerateManifests,
    GeneratedCourse, LessonManifest, UserPreferences,
};

/// The common instructions for all lessons in the course.
const INSTRUCTIONS: &str = indoc! {"
    Given the following passage from the piece, start by listening to it repeatedly
    until you can audiate it clearly in your head. You can also attempt to hum or
    sing it if possible. Then, play the passage on your instrument.
"};

//@<music-asset
/// Represents a music asset to be practiced.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum MusicAsset {
    /// A link to a SoundSlice.
    SoundSlice(String),

    /// The path to a local file. For example, the path to a PDF of the sheet music.
    LocalFile(String),
}
//>@music-asset

impl MusicAsset {
    /// Generates an exercise asset from this music asset.
    pub fn generate_exercise_asset(&self, start: &str, end: &str) -> ExerciseAsset {
        match self {
            MusicAsset::SoundSlice(url) => {
                let description = formatdoc! {"
                    {}

                    - Passage start: {}
                    - Passage end: {}
                ", INSTRUCTIONS, start, end};
                ExerciseAsset::SoundSliceAsset {
                    link: url.clone(),
                    description: Some(description),
                    backup: None,
                }
            }
            MusicAsset::LocalFile(path) => {
                let description = formatdoc! {"
                    {}

                    - Passage start: {}
                    - Passage end: {}
                    
                    The file containing the music sheet is located at {}. Relative paths are
                    relative to the root of the course.
                ", INSTRUCTIONS, start, end, path};
                ExerciseAsset::BasicAsset(BasicAsset::InlinedAsset {
                    content: description,
                })
            }
        }
    }
}

//@<music-passage
/// Represents a music passage to be practiced.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MusicPassage {
    /// The start of the passage.
    pub start: String,

    /// The end of the passage.
    pub end: String,

    /// The sub-passages that must be mastered before this passage can be mastered. Each
    /// sub-passage should be given a unique index which will be used to generate the lesson ID.
    /// Those values should not change once they are defined or progress for this lesson will be
    /// lost. This value is a map instead of a list because rearranging the order of the
    /// passages in a list would also change the IDs of the generated lessons.
    pub sub_passages: HashMap<usize, MusicPassage>,
}
//>@music-passage

impl MusicPassage {
    /// Generates the lesson ID for this course and passage, identified by the given path.
    pub fn generate_lesson_id(
        &self,
        course_manifest: &CourseManifest,
        passage_path: Vec<usize>,
    ) -> Ustr {
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
        sub_passages: &HashMap<usize, MusicPassage>,
        music_asset: &MusicAsset,
    ) -> Vec<(LessonManifest, Vec<ExerciseManifest>)> {
        // Recursively generate the dependency lessons and IDs.
        let mut lessons = vec![];
        let mut dependency_ids = vec![];
        for (index, sub_passage) in sub_passages {
            // Create the dependency path.
            let mut dependency_path = passage_path.clone();
            dependency_path.push(*index);

            // Generate the dependency ID and lessons.
            dependency_ids
                .push(sub_passage.generate_lesson_id(course_manifest, dependency_path.clone()));
            lessons.append(&mut sub_passage.generate_lesson_helper(
                course_manifest,
                dependency_path,
                &sub_passage.sub_passages,
                music_asset,
            ));
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
            exercise_asset: music_asset.generate_exercise_asset(&self.start, &self.end),
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
        // Use a starting path of [0].
        self.generate_lesson_helper(course_manifest, vec![0], &self.sub_passages, music_asset)
    }
}

//@<music-piece-config
/// The config to create a course that teaches a piece of music.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MusicPieceConfig {
    /// The asset containing the music to be practiced.
    pub music_asset: MusicAsset,

    /// The passages in which the music is divided for practice.
    pub passages: MusicPassage,
}
//>@music-piece-config

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
