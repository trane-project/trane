use anyhow::Result;
use indoc::indoc;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use ustr::Ustr;

use crate::data::{
    music::notes::Note, BasicAsset, CourseGeneratorUserConfig, CourseManifest, ExerciseAsset,
    ExerciseManifest, ExerciseType, GenerateManifests, LessonManifest,
};

const SINGING_DESCRIPTION: &str = indoc! {"
    Listen to, audiate, and sing the passage. Refer to the lesson instructions for
    more details.
"};

const RHYTHM_DESCRIPTION: &str = indoc! {"
    Sight-sing or use your instrument to improvise using the rhythm of the passage.
"};

const MELODY_DESCRIPTION: &str = indoc! {"
    Sight-sing or use your instrument to improvise using the melody of the passage.
"};

const BASIC_HARMONY_DESCRIPTION: &str = indoc! {"
    Sight-sing or use your instrument to improvise using the basic harmony of the passage.
"};

const ADVANCED_HARMONY_DESCRIPTION: &str = indoc! {"
    Sight-sing or use your instrument to improvise using all the harmony of the passage.
"};

const MASTERY_DESCRIPTION: &str = indoc! {"
    Sight-sing or use your instrument to improvise using all the melodic, rhythmic, and harmonic
    elements of the passage.
"};

lazy_static! {
    /// The instructions for the singing lessons.
    pub static ref SINGING_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        This step involves listening to the musical passage, audiating it in your head,
        and then singing it. You should sing the passage as accurately as possible, but
        it's not required that you use solfege syllables or numbers to identify the notes.

        This step does not require listening to the passage in all keys. You can choose
        the original key of the passage, or any other key that you prefer. No improvisation
        is required in this step.
    "});

    /// The instructions for the rhythm lessons.
    pub static ref RHYTHM_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        This step involves sight-singing or the stated instrument to improvise using the
        rhythm of the passage.

        When sight-singing, you can use a simple rhytm syllable system or a more complex
        one (e.g the Kodaly system).
    "});

    /// The instructions for the melody lessons.
    pub static ref MELODY_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        This step involves sight-singing or the stated instrument to improvise using the
        melody of the passage.

        Use your prefered sight-singing system (refer to the course instructions). When
        using your instrument, you should sing along.
    "});

    /// The instructions for the basic harmony lessons.
    pub static ref BASIC_HARMONY_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        This step involves sight-singing or the stated instrument to improvise using the
        basic harmony of the passage. The basic harmony consists of the main chord tones
        of each chord in the progression.

        Use your prefered sight-singing system (refer to the course instructions). When
        using your instrument, you should sing along.
    "});

    /// The instructions for the basic harmony lessons.
    pub static ref ADVANCED_HARMONY_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        This step involves sight-singing or the stated instrument to improvise using all
        the harmony of the passage, including tones in the scale or mode that are not the
        chord tones as well as chromatic notes. 

        Use your prefered sight-singing system (refer to the course instructions). When
        using your instrument, you should sing along.
    "});

    /// The instructions for the rhythm lessons.
    pub static ref MASTERY_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        Using all you have learned in the previous lessons, select a key at random and
        improvise using all the melodic, rhythmic, and harmonic elements of the passage.
    "});
}

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
    fn exercise_id(&self, lesson_id: Ustr, exercise_index: usize) -> Ustr {
        Ustr::from(&format!("{}::exercise_{}", lesson_id, exercise_index))
    }

    fn all_instruments(user_config: &TraneImprovisationUserConfig) -> Result<Vec<Option<&str>>> {
        let mut all_instuments: Vec<Option<&str>> = user_config
            .instruments
            .iter()
            .map(|s| Some(s.as_str()))
            .collect();
        all_instuments.push(None);
        Ok(all_instuments)
    }

    fn singing_lesson_id(&self, course_id: Ustr) -> Ustr {
        Ustr::from(&format!("{}::singing", course_id))
    }

    fn generate_singing_exercise(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        passage: (usize, &ImprovisationPassage),
    ) -> Result<ExerciseManifest> {
        let exercise_name = format!("{} - Singing", course_manifest.name);

        Ok(ExerciseManifest {
            id: self.exercise_id(lesson_id, passage.0),
            lesson_id,
            course_id: course_manifest.id,
            name: exercise_name,
            description: Some(SINGING_DESCRIPTION.to_string()),
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
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        let lesson_manifest = LessonManifest {
            id: self.singing_lesson_id(course_manifest.id),
            course_id: course_manifest.id,
            name: format!("{} - Singing", course_manifest.name),
            description: Some(SINGING_DESCRIPTION.to_string()),
            dependencies: vec![],
            metadata: None,
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *SINGING_INSTRUCTIONS,
            }),
            lesson_material: None,
        };

        let exercises = self
            .passages
            .iter()
            .enumerate()
            .map(|passage| {
                self.generate_singing_exercise(course_manifest, lesson_manifest.id, passage)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(vec![(lesson_manifest, exercises)])
    }

    fn rhythm_lesson_id(&self, course_id: Ustr, instrument: Option<&str>) -> Ustr {
        match instrument {
            Some(instrument) => Ustr::from(&format!("{}::rhythm::{}", course_id, instrument)),
            None => Ustr::from(&format!("{}::rhythm", course_id)),
        }
    }

    fn generate_rhythm_exercise(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        instrument: Option<&str>,
        passage: (usize, &ImprovisationPassage),
    ) -> Result<ExerciseManifest> {
        let exercise_name = match instrument {
            Some(instrument) => format!("{} - Rhythm - {}", course_manifest.name, instrument),
            None => format!("{} - Rhythm", course_manifest.name),
        };

        Ok(ExerciseManifest {
            id: self.exercise_id(lesson_id, passage.0),
            lesson_id,
            course_id: course_manifest.id,
            name: exercise_name,
            description: Some(RHYTHM_DESCRIPTION.to_string()),
            exercise_type: ExerciseType::Procedural,
            exercise_asset: ExerciseAsset::SoundSliceAsset {
                link: passage.1.soundslice_link.clone(),
                description: None,
                backup: passage.1.music_xml_file.clone(),
            },
        })
    }

    fn generate_rhythm_lesson(
        &self,
        course_manifest: &CourseManifest,
        instrument: Option<&str>,
    ) -> Result<(LessonManifest, Vec<ExerciseManifest>)> {
        let lesson_id = self.rhythm_lesson_id(course_manifest.id, instrument);
        let lesson_name = match instrument {
            Some(instrument) => format!("{} - Rhythm - {}", course_manifest.name, instrument),
            None => format!("{} - Rhythm", course_manifest.name),
        };
        let lesson_dependencies = match instrument {
            Some(_) => vec![self.rhythm_lesson_id(course_manifest.id, None)],
            None => vec![self.singing_lesson_id(course_manifest.id)],
        };

        let lesson_manifest = LessonManifest {
            id: lesson_id,
            course_id: course_manifest.id,
            name: lesson_name,
            description: Some(RHYTHM_DESCRIPTION.to_string()),
            dependencies: lesson_dependencies,
            metadata: None,
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *RHYTHM_INSTRUCTIONS,
            }),
            lesson_material: None,
        };

        let exercises = self
            .passages
            .iter()
            .enumerate()
            .map(|passage| {
                self.generate_rhythm_exercise(
                    course_manifest,
                    lesson_manifest.id,
                    instrument,
                    passage,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((lesson_manifest, exercises))
    }

    fn generate_rhythm_lessons(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationUserConfig,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        let all_instruments = Self::all_instruments(user_config)?;
        let lessons = all_instruments
            .iter()
            .map(|instrument| self.generate_rhythm_lesson(course_manifest, *instrument))
            .collect::<Result<Vec<_>>>()?;
        Ok(lessons)
    }

    fn melody_lesson_id(&self, course_id: Ustr, key: Note, instrument: Option<&str>) -> Ustr {
        match instrument {
            None => Ustr::from(&format!("{}::melody::{}", course_id, key.to_string())),
            Some(instrument) => Ustr::from(&format!(
                "{}::melody::{}::{}",
                course_id,
                key.to_string(),
                instrument
            )),
        }
    }

    fn generate_melody_exercise(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        key: Note,
        instrument: Option<&str>,
        passage: (usize, &ImprovisationPassage),
    ) -> Result<ExerciseManifest> {
        let exercise_name = match instrument {
            Some(instrument) => format!(
                "{} - Melody - Key of {} - {}",
                course_manifest.name,
                key.to_string(),
                instrument
            ),
            None => format!(
                "{} - Melody - Key of {}",
                course_manifest.name,
                key.to_string()
            ),
        };

        Ok(ExerciseManifest {
            id: self.exercise_id(lesson_id, passage.0),
            lesson_id,
            course_id: course_manifest.id,
            name: exercise_name,
            description: Some(MELODY_DESCRIPTION.to_string()),
            exercise_type: ExerciseType::Procedural,
            exercise_asset: ExerciseAsset::SoundSliceAsset {
                link: passage.1.soundslice_link.clone(),
                description: None,
                backup: passage.1.music_xml_file.clone(),
            },
        })
    }

    fn generate_melody_lesson(
        &self,
        course_manifest: &CourseManifest,
        key: Note,
        instrument: Option<&str>,
    ) -> Result<(LessonManifest, Vec<ExerciseManifest>)> {
        let lesson_id = self.melody_lesson_id(course_manifest.id, key, instrument);
        let lesson_name = match instrument {
            Some(instrument) => format!(
                "{} - Melody - Key of {} - {}",
                course_manifest.name,
                key.to_string(),
                instrument
            ),
            None => format!(
                "{} - Melody - Key of {}",
                course_manifest.name,
                key.to_string()
            ),
        };

        let previous_key = key.previous_key_in_circle();
        let lesson_dependencies = match (previous_key, instrument) {
            (None, None) => vec![self.singing_lesson_id(course_manifest.id)],
            (None, Some(_)) => {
                vec![self.melody_lesson_id(course_manifest.id, key, None)]
            }
            (Some(previous_key), None) => {
                vec![self.melody_lesson_id(course_manifest.id, previous_key, None)]
            }
            (Some(previous_key), Some(instrument)) => {
                vec![
                    self.melody_lesson_id(course_manifest.id, previous_key, Some(instrument)),
                    self.melody_lesson_id(course_manifest.id, key, None),
                ]
            }
        };

        let lesson_manifest = LessonManifest {
            id: lesson_id,
            course_id: course_manifest.id,
            name: lesson_name,
            description: Some(MELODY_DESCRIPTION.to_string()),
            dependencies: lesson_dependencies,
            metadata: None,
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *MELODY_INSTRUCTIONS,
            }),
            lesson_material: None,
        };

        let exercises = self
            .passages
            .iter()
            .enumerate()
            .map(|passage| {
                self.generate_melody_exercise(
                    course_manifest,
                    lesson_manifest.id,
                    key,
                    instrument,
                    passage,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((lesson_manifest, exercises))
    }

    fn generate_melody_lessons(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationUserConfig,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        let all_keys = Note::all_keys();
        let all_instruments = Self::all_instruments(user_config)?;
        all_keys
            .iter()
            .zip(all_instruments.iter())
            .map(|(key, instrument)| {
                self.generate_melody_lesson(course_manifest, *key, *instrument)
            })
            .collect::<Result<Vec<_>>>()
    }

    fn basic_harmony_lesson_id(
        &self,
        course_id: Ustr,
        key: Note,
        instrument: Option<&str>,
    ) -> Ustr {
        match instrument {
            None => Ustr::from(&format!(
                "{}::basic_harmony::{}",
                course_id,
                key.to_string()
            )),
            Some(instrument) => Ustr::from(&format!(
                "{}::basic_harmony::{}::{}",
                course_id,
                key.to_string(),
                instrument
            )),
        }
    }

    fn generate_basic_harmony_exercise(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        key: Note,
        instrument: Option<&str>,
        passage: (usize, &ImprovisationPassage),
    ) -> Result<ExerciseManifest> {
        let exercise_name = match instrument {
            Some(instrument) => format!(
                "{} - Basic Harmony - Key of {} - {}",
                course_manifest.name,
                key.to_string(),
                instrument
            ),
            None => format!(
                "{} - Basic Harmony - Key of {}",
                course_manifest.name,
                key.to_string()
            ),
        };

        Ok(ExerciseManifest {
            id: self.exercise_id(lesson_id, passage.0),
            lesson_id,
            course_id: course_manifest.id,
            name: exercise_name,
            description: Some(BASIC_HARMONY_DESCRIPTION.to_string()),
            exercise_type: ExerciseType::Procedural,
            exercise_asset: ExerciseAsset::SoundSliceAsset {
                link: passage.1.soundslice_link.clone(),
                description: None,
                backup: passage.1.music_xml_file.clone(),
            },
        })
    }

    fn generate_basic_harmony_lesson(
        &self,
        course_manifest: &CourseManifest,
        key: Note,
        instrument: Option<&str>,
    ) -> Result<(LessonManifest, Vec<ExerciseManifest>)> {
        let lesson_id = self.melody_lesson_id(course_manifest.id, key, instrument);
        let lesson_name = match instrument {
            Some(instrument) => format!(
                "{} - Basic Harmony - Key of {} - {}",
                course_manifest.name,
                key.to_string(),
                instrument
            ),
            None => format!(
                "{} - Basic Harmony - Key of {}",
                course_manifest.name,
                key.to_string()
            ),
        };

        let previous_key = key.previous_key_in_circle();
        let lesson_dependencies = match (previous_key, instrument) {
            (None, None) => vec![self.singing_lesson_id(course_manifest.id)],
            (None, Some(_)) => {
                vec![self.basic_harmony_lesson_id(course_manifest.id, key, None)]
            }
            (Some(previous_key), None) => {
                vec![self.basic_harmony_lesson_id(course_manifest.id, previous_key, None)]
            }
            (Some(previous_key), Some(instrument)) => {
                vec![
                    self.basic_harmony_lesson_id(
                        course_manifest.id,
                        previous_key,
                        Some(instrument),
                    ),
                    self.basic_harmony_lesson_id(course_manifest.id, key, None),
                ]
            }
        };

        let lesson_manifest = LessonManifest {
            id: lesson_id,
            course_id: course_manifest.id,
            name: lesson_name,
            description: Some(BASIC_HARMONY_DESCRIPTION.to_string()),
            dependencies: lesson_dependencies,
            metadata: None,
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *BASIC_HARMONY_INSTRUCTIONS,
            }),
            lesson_material: None,
        };

        let exercises = self
            .passages
            .iter()
            .enumerate()
            .map(|passage| {
                self.generate_basic_harmony_exercise(
                    course_manifest,
                    lesson_manifest.id,
                    key,
                    instrument,
                    passage,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((lesson_manifest, exercises))
    }

    fn generate_basic_harmony_lessons(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationUserConfig,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        let all_keys = Note::all_keys();
        let all_instruments = Self::all_instruments(user_config)?;
        all_keys
            .iter()
            .zip(all_instruments.iter())
            .map(|(key, instrument)| {
                self.generate_basic_harmony_lesson(course_manifest, *key, *instrument)
            })
            .collect::<Result<Vec<_>>>()
    }

    fn advanced_harmony_lesson_id(
        &self,
        course_id: Ustr,
        key: Note,
        instrument: Option<&str>,
    ) -> Ustr {
        match instrument {
            None => Ustr::from(&format!(
                "{}::advanced_harmony::{}",
                course_id,
                key.to_string()
            )),
            Some(instrument) => Ustr::from(&format!(
                "{}::advanced_harmony::{}::{}",
                course_id,
                key.to_string(),
                instrument
            )),
        }
    }

    fn generate_advanced_harmony_exercise(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        key: Note,
        instrument: Option<&str>,
        passage: (usize, &ImprovisationPassage),
    ) -> Result<ExerciseManifest> {
        let exercise_name = match instrument {
            Some(instrument) => format!(
                "{} - Advanced Harmony - Key of {} - {}",
                course_manifest.name,
                key.to_string(),
                instrument
            ),
            None => format!(
                "{} - Advanced Harmony - Key of {}",
                course_manifest.name,
                key.to_string()
            ),
        };

        Ok(ExerciseManifest {
            id: self.exercise_id(lesson_id, passage.0),
            lesson_id,
            course_id: course_manifest.id,
            name: exercise_name,
            description: Some(ADVANCED_HARMONY_DESCRIPTION.to_string()),
            exercise_type: ExerciseType::Procedural,
            exercise_asset: ExerciseAsset::SoundSliceAsset {
                link: passage.1.soundslice_link.clone(),
                description: None,
                backup: passage.1.music_xml_file.clone(),
            },
        })
    }

    fn generate_advanced_harmony_lesson(
        &self,
        course_manifest: &CourseManifest,
        key: Note,
        instrument: Option<&str>,
    ) -> Result<(LessonManifest, Vec<ExerciseManifest>)> {
        let lesson_id = self.melody_lesson_id(course_manifest.id, key, instrument);
        let lesson_name = match instrument {
            Some(instrument) => format!(
                "{} - Advanced Harmony - Key of {} - {}",
                course_manifest.name,
                key.to_string(),
                instrument
            ),
            None => format!(
                "{} - Advanced Harmony - Key of {}",
                course_manifest.name,
                key.to_string()
            ),
        };

        let previous_key = key.previous_key_in_circle();
        let lesson_dependencies = match (previous_key, instrument) {
            (None, None) => vec![self.basic_harmony_lesson_id(course_manifest.id, key, None)],
            (None, Some(instrument)) => {
                vec![
                    self.basic_harmony_lesson_id(course_manifest.id, key, Some(instrument)),
                    self.advanced_harmony_lesson_id(course_manifest.id, key, None),
                ]
            }
            (Some(previous_key), None) => {
                vec![
                    self.basic_harmony_lesson_id(course_manifest.id, key, None),
                    self.advanced_harmony_lesson_id(course_manifest.id, previous_key, None),
                ]
            }
            (Some(previous_key), Some(instrument)) => {
                vec![
                    self.basic_harmony_lesson_id(course_manifest.id, key, Some(instrument)),
                    self.advanced_harmony_lesson_id(
                        course_manifest.id,
                        previous_key,
                        Some(instrument),
                    ),
                    self.advanced_harmony_lesson_id(course_manifest.id, key, None),
                ]
            }
        };

        let lesson_manifest = LessonManifest {
            id: lesson_id,
            course_id: course_manifest.id,
            name: lesson_name,
            description: Some(ADVANCED_HARMONY_DESCRIPTION.to_string()),
            dependencies: lesson_dependencies,
            metadata: None,
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *ADVANCED_HARMONY_INSTRUCTIONS,
            }),
            lesson_material: None,
        };

        let exercises = self
            .passages
            .iter()
            .enumerate()
            .map(|passage| {
                self.generate_advanced_harmony_exercise(
                    course_manifest,
                    lesson_manifest.id,
                    key,
                    instrument,
                    passage,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((lesson_manifest, exercises))
    }

    fn generate_advanced_harmony_lessons(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationUserConfig,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        let all_keys = Note::all_keys();
        let all_instruments = Self::all_instruments(user_config)?;
        all_keys
            .iter()
            .zip(all_instruments.iter())
            .map(|(key, instrument)| {
                self.generate_advanced_harmony_lesson(course_manifest, *key, *instrument)
            })
            .collect::<Result<Vec<_>>>()
    }

    fn mastery_lesson_id(&self, course_id: Ustr, instrument: Option<&str>) -> Ustr {
        match instrument {
            Some(instrument) => Ustr::from(&format!("{}::mastery::{}", course_id, instrument)),
            None => Ustr::from(&format!("{}::mastery", course_id)),
        }
    }

    fn generate_mastery_exercise(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        instrument: Option<&str>,
        passage: (usize, &ImprovisationPassage),
    ) -> Result<ExerciseManifest> {
        let exercise_name = match instrument {
            Some(instrument) => format!("{} - Mastery - {}", course_manifest.name, instrument),
            None => format!("{} - Mastery", course_manifest.name),
        };

        Ok(ExerciseManifest {
            id: self.exercise_id(lesson_id, passage.0),
            lesson_id,
            course_id: course_manifest.id,
            name: exercise_name,
            description: Some(MASTERY_DESCRIPTION.to_string()),
            exercise_type: ExerciseType::Procedural,
            exercise_asset: ExerciseAsset::SoundSliceAsset {
                link: passage.1.soundslice_link.clone(),
                description: None,
                backup: passage.1.music_xml_file.clone(),
            },
        })
    }

    fn generate_mastery_lesson(
        &self,
        course_manifest: &CourseManifest,
        instrument: Option<&str>,
    ) -> Result<(LessonManifest, Vec<ExerciseManifest>)> {
        let lesson_id = self.mastery_lesson_id(course_manifest.id, instrument);
        let lesson_name = match instrument {
            Some(instrument) => format!("{} - Mastery - {}", course_manifest.name, instrument),
            None => format!("{} - Mastery", course_manifest.name),
        };

        let last_keys = Note::last_keys_in_circle();
        let lesson_dependencies = last_keys
            .iter()
            .flat_map(|key| {
                vec![
                    self.rhythm_lesson_id(course_manifest.id, instrument),
                    self.melody_lesson_id(course_manifest.id, *key, instrument),
                    self.advanced_harmony_lesson_id(course_manifest.id, *key, instrument),
                ]
            })
            .collect::<Vec<_>>();

        let lesson_manifest = LessonManifest {
            id: lesson_id,
            course_id: course_manifest.id,
            name: lesson_name,
            description: Some(MASTERY_DESCRIPTION.to_string()),
            dependencies: lesson_dependencies,
            metadata: None,
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *MASTERY_INSTRUCTIONS,
            }),
            lesson_material: None,
        };

        let exercises = self
            .passages
            .iter()
            .enumerate()
            .map(|passage| {
                self.generate_mastery_exercise(
                    course_manifest,
                    lesson_manifest.id,
                    instrument,
                    passage,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((lesson_manifest, exercises))
    }

    fn generate_mastery_lessons(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationUserConfig,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        let all_instruments = Self::all_instruments(user_config)?;
        let lessons = all_instruments
            .iter()
            .map(|instrument| self.generate_mastery_lesson(course_manifest, *instrument))
            .collect::<Result<Vec<_>>>()?;
        Ok(lessons)
    }

    fn generate_rhtyhm_only_manifests(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationUserConfig,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        Ok(vec![
            self.generate_singing_lesson(course_manifest)?,
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
            self.generate_singing_lesson(course_manifest)?,
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
        }
    }
}
