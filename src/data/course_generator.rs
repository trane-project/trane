use anyhow::Result;
use serde::{Deserialize, Serialize};
use ustr::Ustr;

use crate::data::{
    music::{modes::Mode, notes::Note},
    CourseGeneratorUserConfig, CourseManifest, ExerciseAsset, ExerciseManifest, ExerciseType,
    GenerateManifests, LessonManifest,
};

/// A single musical passage to be used in a Trane improvisation course. A course can contain
/// multiple passages but all of those passages are assumed to have the same key or mode.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ImprovisationPassage {
    /// The link to a SoundSlice page that contains the passage to be played.
    pub soundslice_link: String,

    /// An optional path to a MusicXML file that contains the passage to be played. This file should
    /// contain the same passage as the SoundSlice link.
    pub music_xml_file: Option<String>,
}

/// The configuration for creating a new improvisation course.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TraneImprovisationConfig {
    /// The dependencies on other Trane improvisation courses. Specifying these dependencies here
    /// instead of the [CourseManifest](data::CourseManifest) allows Trane to generate more
    /// fine-grained dependencies.
    pub improvisation_dependencies: Vec<Ustr>,

    /// The mode of all the passages in the course. This value is optional and if not provided, the
    /// lesson to practice each exercise in a different key will not be generated. For most material
    /// that has a tonal center, this value should be provided.
    pub mode: Option<Mode>,

    /// If true, the course contains passages that concern only rhythm. Lessons to learn the melody
    /// and harmony of the passages will not be generated. The mode of the course will be ignored.
    pub rhythm_only: bool,

    /// The passages to be used in the course.
    pub passages: Vec<ImprovisationPassage>,
}

/// Settings for generating a new improvisation course that are specific to a user.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TraneImprovisationUserConfig {
    /// The list of instruments the user wants to practice.
    pub instruments: Vec<String>,
}

impl TraneImprovisationConfig {
    fn singing_lesson_id(&self, course_id: Ustr, key: Option<Note>) -> Ustr {
        match key {
            None => Ustr::from(&format!("{}::singing", course_id)),
            Some(key) => Ustr::from(&format!("{}::singing::{}", course_id, key.to_string())),
        }
    }

    fn singing_exercise_id(&self, lesson_id: Ustr, exercise_index: usize) -> Ustr {
        Ustr::from(&format!("{}::exercise_{}", lesson_id, exercise_index))
    }

    fn generate_singing_exercise(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        key: Option<Note>,
        passage: (usize, &ImprovisationPassage),
    ) -> Result<ExerciseManifest> {
        let exercise_name = match key {
            None => format!("{} - Singing", course_manifest.name),
            Some(key) => format!(
                "{} - Singing - {} Major (or equivalent)",
                course_manifest.name,
                key.to_string()
            ),
        };

        Ok(ExerciseManifest {
            id: self.singing_exercise_id(lesson_id, passage.0),
            lesson_id,
            course_id: course_manifest.id,
            name: exercise_name,
            description: None,
            exercise_type: ExerciseType::Procedural,
            exercise_asset: ExerciseAsset::SoundSliceAsset {
                link: passage.1.soundslice_link.clone(),
                description: None,
                backup: passage.1.music_xml_file.clone(),
            },
        })
    }

    fn generate_singing_lesson(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationUserConfig,
        key: Option<Note>,
        dummy_lesson: bool,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        if dummy_lesson {
            let lesson_manifest = LessonManifest {
                id: self.singing_lesson_id(course_manifest.id, None),
                course_id: course_manifest.id,
                name: format!("{} - Singing", course_manifest.name),
                description: Some("Singing".to_string()),
                dependencies: vec![],
                metadata: None,
                lesson_instructions: None,
                lesson_material: None,
            };
            return Ok(vec![(lesson_manifest, vec![])]);
        }

        unimplemented!()
    }

    fn generate_singing_lessons(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationUserConfig,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        unimplemented!()
    }

    fn rhythm_lesson_id(&self, course_id: Ustr) -> Ustr {
        Ustr::from(&format!("{}::rhythm", course_id))
    }

    fn generate_rhythm_lessons(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationUserConfig,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        unimplemented!()
    }

    fn melody_lesson_id(&self, course_id: Ustr, key: Option<Note>) -> Ustr {
        match key {
            None => Ustr::from(&format!("{}::melody", course_id)),
            Some(key) => Ustr::from(&format!("{}::melody::{}", course_id, key.to_string())),
        }
    }

    fn generate_melody_lessons(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationUserConfig,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        unimplemented!()
    }

    fn basic_harmony_lesson_id(&self, course_id: Ustr, key: Option<Note>) -> Ustr {
        match key {
            None => Ustr::from(&Ustr::from(&format!("{}::basic_harmony", course_id))),
            Some(key) => Ustr::from(&format!(
                "{}::basic_harmony::{}",
                course_id,
                key.to_string()
            )),
        }
    }

    fn generate_basic_harmony_lessons(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationUserConfig,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        unimplemented!()
    }

    fn advanced_harmony_lesson_id(&self, course_id: Ustr, key: Option<Note>) -> Ustr {
        match key {
            None => Ustr::from(&Ustr::from(&format!("{}::advanced_harmony", course_id))),
            Some(key) => Ustr::from(&format!(
                "{}::advanced_harmony::{}",
                course_id,
                key.to_string()
            )),
        }
    }

    fn generate_advanced_harmony_lessons(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationUserConfig,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        unimplemented!()
    }

    fn mastery_lesson_id(&self, course_id: Ustr, key: Option<Note>) -> Ustr {
        match key {
            None => Ustr::from(&Ustr::from(&format!("{}::mastery", course_id))),
            Some(key) => Ustr::from(&format!("{}::mastery::{}", course_id, key.to_string())),
        }
    }

    fn generate_mastery_lessons(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationUserConfig,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        unimplemented!()
    }

    fn generate_rhtyhm_only_manifests(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationUserConfig,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        Ok(vec![
            self.generate_singing_lessons(course_manifest, user_config)?,
            self.generate_rhythm_lessons(course_manifest, user_config)?,
        ]
        .into_iter()
        .flatten()
        .collect())
    }

    fn generate_all_manifests(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationUserConfig,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        Ok(vec![
            self.generate_singing_lessons(course_manifest, user_config)?,
            self.generate_rhythm_lessons(course_manifest, user_config)?,
            self.generate_melody_lessons(course_manifest, user_config)?,
            self.generate_basic_harmony_lessons(course_manifest, user_config)?,
            self.generate_advanced_harmony_lessons(course_manifest, user_config)?,
            self.generate_mastery_lessons(course_manifest, user_config)?,
        ]
        .into_iter()
        .flatten()
        .collect())
    }
}

impl GenerateManifests for TraneImprovisationConfig {
    fn generate_manifests(
        &self,
        course_manifest: &CourseManifest,
        user_config: &CourseGeneratorUserConfig,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        match user_config {
            CourseGeneratorUserConfig::TraneImprovisation(user_config) => {
                if self.rhythm_only {
                    self.generate_rhtyhm_only_manifests(course_manifest, user_config)
                } else {
                    self.generate_all_manifests(course_manifest, user_config)
                }
            }
            _ => Err(anyhow::anyhow!("Invalid course generator user config")),
        }
    }
}
