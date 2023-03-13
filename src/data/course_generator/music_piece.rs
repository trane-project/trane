//! Contains the logic for generating a course to learn a piece of music.
//!
//! Given a piece of music and the passages and sub-passages in which it is divided, this module
//! generates a course that allows the user to learn the piece of music by first practicing the
//! smallest passages and then working up until the full piece is mastered.

use anyhow::Result;
use indoc::{formatdoc, indoc};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};
use typeshare::typeshare;
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
#[serde(tag = "type", content = "content")]
#[typeshare]
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
                    relative to the working directory.
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
#[typeshare]
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
    #[typeshare(serialized_as = "HashMap<u32, MusicPassage>")]
    pub sub_passages: HashMap<usize, MusicPassage>,
}
//>@music-passage

impl MusicPassage {
    /// Generates the lesson ID for this course and passage, identified by the given path.
    fn generate_lesson_id(course_manifest: &CourseManifest, passage_path: Vec<usize>) -> Ustr {
        let lesson_id = passage_path
            .iter()
            .map(|index| format!("{index}"))
            .collect::<Vec<String>>()
            .join("::");
        Ustr::from(&format!("{}::{}", course_manifest.id, lesson_id))
    }

    /// Generates a clone of the given path with the given index appended.
    fn new_path(passage_path: &[usize], index: usize) -> Vec<usize> {
        let mut new_path = passage_path.to_vec();
        new_path.push(index);
        new_path
    }

    /// Generates the lesson and exercise manifests for this passage, recursively doing so if the
    /// dependencies are not empty.
    pub fn generate_lesson_helper(
        &self,
        course_manifest: &CourseManifest,
        passage_path: Vec<usize>,
        music_asset: &MusicAsset,
    ) -> Vec<(LessonManifest, Vec<ExerciseManifest>)> {
        // Recursively generate the dependency lessons and IDs.
        let mut lessons = vec![];
        let mut dependency_ids = vec![];
        for (index, sub_passage) in &self.sub_passages {
            // Create the dependency path.
            let dependency_path = Self::new_path(&passage_path, *index);

            // Generate the dependency ID and lessons.
            dependency_ids.push(Self::generate_lesson_id(
                course_manifest,
                dependency_path.clone(),
            ));
            lessons.append(&mut sub_passage.generate_lesson_helper(
                course_manifest,
                dependency_path,
                music_asset,
            ));
        }

        // Create the lesson and exercise manifests for this passage and add them to the list.
        let lesson_manifest = LessonManifest {
            id: Self::generate_lesson_id(course_manifest, passage_path),
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
        self.generate_lesson_helper(course_manifest, vec![0], music_asset)
    }
}

//@<music-piece-config
/// The config to create a course that teaches a piece of music.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[typeshare]
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

#[cfg(test)]
mod test {
    use super::*;

    // Verifies generating a valid exercise asset from a local file.
    #[test]
    fn generate_local_music_asset() {
        let music_asset = MusicAsset::LocalFile("music.pdf".to_string());
        let passage = MusicPassage {
            start: "start".to_string(),
            end: "end".to_string(),
            sub_passages: HashMap::new(),
        };
        let exercise_asset = music_asset.generate_exercise_asset(&passage.start, &passage.end);
        assert_eq!(
            exercise_asset,
            ExerciseAsset::BasicAsset(BasicAsset::InlinedAsset {
                content: indoc! {"
                    Given the following passage from the piece, start by listening to it repeatedly
                    until you can audiate it clearly in your head. You can also attempt to hum or
                    sing it if possible. Then, play the passage on your instrument.

                    - Passage start: start
                    - Passage end: end
                    
                    The file containing the music sheet is located at music.pdf. Relative paths are
                    relative to the working directory.
                "}
                .to_string()
            })
        );
    }

    // Verifies generating a valid exercise asset from a SoundSlice.
    #[test]
    fn generate_sound_slice_asset() {
        let music_asset = MusicAsset::SoundSlice("https://soundslice.com".to_string());
        let passage = MusicPassage {
            start: "start".to_string(),
            end: "end".to_string(),
            sub_passages: HashMap::new(),
        };
        let exercise_asset = music_asset.generate_exercise_asset(&passage.start, &passage.end);
        assert_eq!(
            exercise_asset,
            ExerciseAsset::SoundSliceAsset {
                link: "https://soundslice.com".to_string(),
                description: Some(
                    indoc! {"
                    Given the following passage from the piece, start by listening to it repeatedly
                    until you can audiate it clearly in your head. You can also attempt to hum or
                    sing it if possible. Then, play the passage on your instrument.

                    - Passage start: start
                    - Passage end: end
                    "}
                    .to_string()
                ),
                backup: None,
            }
        );
    }

    // Verfies generating lesson IDs for a music piece course.
    #[test]
    fn generate_lesson_id() {
        let course_manifest = CourseManifest {
            id: "course".into(),
            name: "Course".to_string(),
            description: None,
            dependencies: vec![],
            metadata: None,
            course_instructions: None,
            course_material: None,
            authors: None,
            generator_config: None,
        };
        assert_eq!(
            MusicPassage::generate_lesson_id(&course_manifest, vec![0]),
            "course::0"
        );
        assert_eq!(
            MusicPassage::generate_lesson_id(&course_manifest, vec![0, 1]),
            "course::0::1"
        );
        assert_eq!(
            MusicPassage::generate_lesson_id(&course_manifest, vec![0, 1, 2]),
            "course::0::1::2"
        );
    }

    // Verifies the paths for the sub-passages are created correctly.
    #[test]
    fn new_path() {
        assert_eq!(MusicPassage::new_path(&vec![0], 1), vec![0, 1]);
        assert_eq!(MusicPassage::new_path(&vec![0, 1], 2), vec![0, 1, 2]);
        assert_eq!(MusicPassage::new_path(&vec![0, 1, 2], 3), vec![0, 1, 2, 3]);
    }

    // Verifies generating lessons for a music piece course.
    #[test]
    fn generate_lessons() {
        let course_manifest = CourseManifest {
            id: "course".into(),
            name: "Course".to_string(),
            description: None,
            dependencies: vec![],
            metadata: None,
            course_instructions: None,
            course_material: None,
            authors: None,
            generator_config: None,
        };
        let music_asset = MusicAsset::LocalFile("music.pdf".to_string());
        let passage = MusicPassage {
            start: "start 0".to_string(),
            end: "end 0".to_string(),
            sub_passages: HashMap::from([(
                0,
                MusicPassage {
                    start: "start 0::0".to_string(),
                    end: "end 0::0".to_string(),
                    sub_passages: HashMap::new(),
                },
            )]),
        };
        let lessons = passage.generate_lessons(&course_manifest, &music_asset);
        assert_eq!(lessons.len(), 2);

        let (lesson_manifest, exercise_manifests) = &lessons[1];
        assert_eq!(lesson_manifest.id, "course::0");
        assert_eq!(lesson_manifest.name, "Course");
        assert_eq!(lesson_manifest.description, None);
        assert_eq!(lesson_manifest.course_id, "course");
        assert_eq!(lesson_manifest.dependencies, vec!["course::0::0"]);
        assert_eq!(exercise_manifests.len(), 1);

        let exercise_manifest = &exercise_manifests[0];
        assert_eq!(exercise_manifest.id, "course::0::exercise");
        assert_eq!(exercise_manifest.name, "Course");
        assert_eq!(exercise_manifest.description, None);
        assert_eq!(exercise_manifest.lesson_id, "course::0");
        assert_eq!(exercise_manifest.course_id, "course");
        assert_eq!(
            exercise_manifest.exercise_asset,
            ExerciseAsset::BasicAsset(BasicAsset::InlinedAsset {
                content: indoc! {"
                    Given the following passage from the piece, start by listening to it repeatedly
                    until you can audiate it clearly in your head. You can also attempt to hum or
                    sing it if possible. Then, play the passage on your instrument.

                    - Passage start: start 0
                    - Passage end: end 0
                    
                    The file containing the music sheet is located at music.pdf. Relative paths are
                    relative to the working directory.
                "}
                .to_string()
            })
        );

        let (lesson_manifest, exercise_manifests) = &lessons[0];
        assert_eq!(lesson_manifest.id, "course::0::0");
        assert_eq!(lesson_manifest.name, "Course");
        assert_eq!(lesson_manifest.description, None);
        assert_eq!(lesson_manifest.course_id, "course");
        assert!(lesson_manifest.dependencies.is_empty());
        assert_eq!(exercise_manifests.len(), 1);

        let exercise_manifest = &exercise_manifests[0];
        assert_eq!(exercise_manifest.id, "course::0::0::exercise");
        assert_eq!(exercise_manifest.name, "Course");
        assert_eq!(exercise_manifest.description, None);
        assert_eq!(exercise_manifest.lesson_id, "course::0::0");
        assert_eq!(exercise_manifest.course_id, "course");
        assert_eq!(
            exercise_manifest.exercise_asset,
            ExerciseAsset::BasicAsset(BasicAsset::InlinedAsset {
                content: indoc! {"
                    Given the following passage from the piece, start by listening to it repeatedly
                    until you can audiate it clearly in your head. You can also attempt to hum or
                    sing it if possible. Then, play the passage on your instrument.

                    - Passage start: start 0::0
                    - Passage end: end 0::0
                    
                    The file containing the music sheet is located at music.pdf. Relative paths are
                    relative to the working directory.
                "}
                .to_string()
            })
        );
    }
}
