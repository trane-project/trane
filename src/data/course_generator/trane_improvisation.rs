//! Defines a special course to teach improvisation based on a set of musical passages.
//!  
//! The Trane improvisation course generator creates a course that teaches the user how to improvise
//! based on a set of musical passages. The passages are provided by the user, and the rhythmic,
//! melodic and harmonic elements of each passage are used to generate a series of lessons for each
//! key and for all the instruments the user selects.

mod constants;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use ustr::Ustr;

use crate::data::{
    course_generator::trane_improvisation::constants::*, music::notes::Note, BasicAsset,
    CourseGeneratorPreferences, CourseManifest, ExerciseAsset, ExerciseManifest, ExerciseType,
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
    /// instead of the [CourseManifest](crate::data::CourseManifest) allows Trane to generate more
    /// fine-grained dependencies.
    pub improvisation_dependencies: Vec<Ustr>,

    /// If true, the course contains passages that concern only rhythm. Lessons to learn the melody
    /// and harmony of the passages will not be generated. The mode of the course will be ignored.
    pub rhythm_only: bool,

    /// The passages to be used in the course.
    pub passages: HashMap<usize, ImprovisationPassage>,
}

/// Settings for generating a new improvisation course that are specific to a user.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TraneImprovisationPreferences {
    /// The list of instruments the user wants to practice.
    pub instruments: Vec<String>,
}

impl TraneImprovisationConfig {
    /// Returns the ID for a given exercise given the lesson ID and the exercise index.
    fn exercise_id(&self, lesson_id: Ustr, exercise_index: usize) -> Ustr {
        Ustr::from(&format!("{}::exercise_{}", lesson_id, exercise_index))
    }

    /// Returns the list of all instruments that the user can practice. A value of None represents
    /// the voice lessons which must be mastered before practicing specific instruments.
    fn all_instruments(user_config: &TraneImprovisationPreferences) -> Result<Vec<Option<&str>>> {
        let mut all_instuments: Vec<Option<&str>> = user_config
            .instruments
            .iter()
            .map(|s| Some(s.as_str()))
            .collect();
        all_instuments.push(None);
        Ok(all_instuments)
    }

    /// Returns the ID of the singing lesson for the given course.
    fn singing_lesson_id(&self, course_id: Ustr) -> Ustr {
        Ustr::from(&format!("{}::singing", course_id))
    }

    /// Generates a singing exercises for the given passage.
    fn generate_singing_exercise(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        passage: (usize, &ImprovisationPassage),
    ) -> Result<ExerciseManifest> {
        Ok(ExerciseManifest {
            id: self.exercise_id(lesson_id, passage.0),
            lesson_id,
            course_id: course_manifest.id,
            name: format!("{} - Singing", course_manifest.name),
            description: Some(SINGING_DESCRIPTION.to_string()),
            exercise_type: ExerciseType::Procedural,
            exercise_asset: ExerciseAsset::SoundSliceAsset {
                link: passage.1.soundslice_link.clone(),
                description: None,
                backup: passage.1.music_xml_file.clone(),
            },
        })
    }

    /// Generates the singing lesson for this course.
    fn generate_singing_lesson(
        &self,
        course_manifest: &CourseManifest,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        // Generate the lesson manifest.
        let lesson_manifest = LessonManifest {
            id: self.singing_lesson_id(course_manifest.id),
            course_id: course_manifest.id,
            name: format!("{} - Singing", course_manifest.name),
            description: Some(SINGING_DESCRIPTION.to_string()),
            dependencies: vec![],
            metadata: Some(BTreeMap::from([
                (COURSE_METADATA.to_string(), vec!["true".to_string()]),
                (LESSON_METADATA.to_string(), vec!["singing".to_string()]),
                (INSTRUMENT_METADATA.to_string(), vec!["voice".to_string()]),
            ])),
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *SINGING_INSTRUCTIONS,
            }),
            lesson_material: None,
        };

        // Generate an exercise for each passage.
        let exercises = self
            .passages
            .iter()
            .map(|(index, passage)| {
                self.generate_singing_exercise(
                    course_manifest,
                    lesson_manifest.id,
                    (*index, passage),
                )
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(vec![(lesson_manifest, exercises)])
    }

    /// Returns the ID of the rhythm lesson for the given course and instrument.
    fn rhythm_lesson_id(&self, course_id: Ustr, instrument: Option<&str>) -> Ustr {
        match instrument {
            Some(instrument) => Ustr::from(&format!("{}::rhythm::{}", course_id, instrument)),
            None => Ustr::from(&format!("{}::rhythm", course_id)),
        }
    }

    /// Generates a rhythm exercise for the given instrument and passage.
    fn generate_rhythm_exercise(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        instrument: Option<&str>,
        passage: (usize, &ImprovisationPassage),
    ) -> Result<ExerciseManifest> {
        // Generate the exercise name.
        let exercise_name = match instrument {
            Some(instrument) => format!("{} - Rhythm - {}", course_manifest.name, instrument),
            None => format!("{} - Rhythm - Sight-singing", course_manifest.name),
        };

        // Generate the exercise manifest.
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

    /// Generates the rhythm lesson for the given instrument.
    fn generate_rhythm_lesson(
        &self,
        course_manifest: &CourseManifest,
        instrument: Option<&str>,
    ) -> Result<(LessonManifest, Vec<ExerciseManifest>)> {
        // Generate the lesson ID and name.
        let lesson_id = self.rhythm_lesson_id(course_manifest.id, instrument);
        let lesson_name = match instrument {
            Some(instrument) => format!("{} - Rhythm - {}", course_manifest.name, instrument),
            None => format!("{} - Rhythm - Sight-singing", course_manifest.name),
        };

        // Declare the dependencies of this lesson.
        let lesson_dependencies = match instrument {
            Some(_) => vec![self.rhythm_lesson_id(course_manifest.id, None)],
            None => vec![self.singing_lesson_id(course_manifest.id)],
        };

        // Generate the lesson manifest.
        let instrument_name = match instrument {
            Some(instrument) => instrument.to_string(),
            None => "voice".to_string(),
        };
        let lesson_manifest = LessonManifest {
            id: lesson_id,
            course_id: course_manifest.id,
            name: lesson_name,
            description: Some(RHYTHM_DESCRIPTION.to_string()),
            dependencies: lesson_dependencies,
            metadata: Some(BTreeMap::from([
                (COURSE_METADATA.to_string(), vec!["true".to_string()]),
                (LESSON_METADATA.to_string(), vec!["rhythm".to_string()]),
                (INSTRUMENT_METADATA.to_string(), vec![instrument_name]),
            ])),
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *RHYTHM_INSTRUCTIONS,
            }),
            lesson_material: None,
        };

        // Generate an exercise for each passage.
        let exercises = self
            .passages
            .iter()
            .map(|(index, passage)| {
                self.generate_rhythm_exercise(
                    course_manifest,
                    lesson_manifest.id,
                    instrument,
                    (*index, passage),
                )
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((lesson_manifest, exercises))
    }

    /// Generates all the rhythm lessons for this course.
    fn generate_rhythm_lessons(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationPreferences,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        // Generate a lesson for each instrument.
        let all_instruments = Self::all_instruments(user_config)?;
        let lessons = all_instruments
            .iter()
            .map(|instrument| self.generate_rhythm_lesson(course_manifest, *instrument))
            .collect::<Result<Vec<_>>>()?;
        Ok(lessons)
    }

    /// Returns the ID of the melody lesson for the given course, key, and instrument.
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

    /// Generates a melody exercise for the given key, instrument, and passage.
    fn generate_melody_exercise(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        key: Note,
        instrument: Option<&str>,
        passage: (usize, &ImprovisationPassage),
    ) -> Result<ExerciseManifest> {
        // Generate the exercise name.
        let exercise_name = match instrument {
            Some(instrument) => format!(
                "{} - Melody - Key of {} - {}",
                course_manifest.name,
                key.to_string(),
                instrument
            ),
            None => format!(
                "{} - Melody - Key of {} - Sight-singing",
                course_manifest.name,
                key.to_string()
            ),
        };

        // Generate the exercise manifest.
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

    /// Generates the melody lesson for the given key and instrument.
    fn generate_melody_lesson(
        &self,
        course_manifest: &CourseManifest,
        key: Note,
        instrument: Option<&str>,
    ) -> Result<(LessonManifest, Vec<ExerciseManifest>)> {
        // Generate the lesson ID and name.
        let lesson_id = self.melody_lesson_id(course_manifest.id, key, instrument);
        let lesson_name = match instrument {
            Some(instrument) => format!(
                "{} - Melody - Key of {} - {}",
                course_manifest.name,
                key.to_string(),
                instrument
            ),
            None => format!(
                "{} - Melody - Key of {} - Sight-singing",
                course_manifest.name,
                key.to_string()
            ),
        };

        // Declare the lesson dependencies based on the previous key in the circle of fifths and the
        // instrument.
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

        // Generate the lesson manifest.
        let instrument_name = match instrument {
            Some(instrument) => instrument.to_string(),
            None => "voice".to_string(),
        };
        let lesson_manifest = LessonManifest {
            id: lesson_id,
            course_id: course_manifest.id,
            name: lesson_name,
            description: Some(MELODY_DESCRIPTION.to_string()),
            dependencies: lesson_dependencies,
            metadata: Some(BTreeMap::from([
                (COURSE_METADATA.to_string(), vec!["true".to_string()]),
                (LESSON_METADATA.to_string(), vec!["melody".to_string()]),
                (INSTRUMENT_METADATA.to_string(), vec![instrument_name]),
                (KEY_METADATA.to_string(), vec![key.to_string()]),
            ])),
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *MELODY_INSTRUCTIONS,
            }),
            lesson_material: None,
        };

        // Generate an exercise for each passage.
        let exercises = self
            .passages
            .iter()
            .map(|(index, passage)| {
                self.generate_melody_exercise(
                    course_manifest,
                    lesson_manifest.id,
                    key,
                    instrument,
                    (*index, passage),
                )
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((lesson_manifest, exercises))
    }

    /// Generates all the melody lessons for the given course.
    fn generate_melody_lessons(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationPreferences,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        // Get a list of all keys and instruments.
        let all_keys = Note::all_keys(false);
        let all_instruments = Self::all_instruments(user_config)?;

        // Generate a lesson for each key and instrument pair.
        all_keys
            .iter()
            .flat_map(|key| {
                all_instruments.iter().map(|instrument| {
                    self.generate_melody_lesson(course_manifest, *key, *instrument)
                })
            })
            .collect::<Result<Vec<_>>>()
    }

    /// Returns the ID of the basic harmony lesson for the given course, key, and instrument.
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

    /// Generates the basic harmony lesson for the given key, instrument, and passage.
    fn generate_basic_harmony_exercise(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        key: Note,
        instrument: Option<&str>,
        passage: (usize, &ImprovisationPassage),
    ) -> Result<ExerciseManifest> {
        // Generate the exercise name.
        let exercise_name = match instrument {
            Some(instrument) => format!(
                "{} - Basic Harmony - Key of {} - {}",
                course_manifest.name,
                key.to_string(),
                instrument
            ),
            None => format!(
                "{} - Basic Harmony - Key of {} - Sight-singing",
                course_manifest.name,
                key.to_string()
            ),
        };

        // Generate the exercise manifest.
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

    /// Generates the basic harmony lesson for the given key and instrument.
    fn generate_basic_harmony_lesson(
        &self,
        course_manifest: &CourseManifest,
        key: Note,
        instrument: Option<&str>,
    ) -> Result<(LessonManifest, Vec<ExerciseManifest>)> {
        // Generate the lesson ID and name.
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

        // Declare the lesson dependencies based on the previous key in the circle of fifths and the
        // instrument.
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

        // Generate the lesson manifest.
        let instrument_name = match instrument {
            Some(instrument) => instrument.to_string(),
            None => "voice".to_string(),
        };
        let lesson_manifest = LessonManifest {
            id: lesson_id,
            course_id: course_manifest.id,
            name: lesson_name,
            description: Some(BASIC_HARMONY_DESCRIPTION.to_string()),
            dependencies: lesson_dependencies,
            metadata: Some(BTreeMap::from([
                (COURSE_METADATA.to_string(), vec!["true".to_string()]),
                (
                    LESSON_METADATA.to_string(),
                    vec!["basic_harmony".to_string()],
                ),
                (INSTRUMENT_METADATA.to_string(), vec![instrument_name]),
                (KEY_METADATA.to_string(), vec![key.to_string()]),
            ])),
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *BASIC_HARMONY_INSTRUCTIONS,
            }),
            lesson_material: None,
        };

        // Generate an exercise for each passage.
        let exercises = self
            .passages
            .iter()
            .map(|(index, passage)| {
                self.generate_basic_harmony_exercise(
                    course_manifest,
                    lesson_manifest.id,
                    key,
                    instrument,
                    (*index, passage),
                )
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((lesson_manifest, exercises))
    }

    /// Generates all basic harmony lessons for the given course.
    fn generate_basic_harmony_lessons(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationPreferences,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        // Get all keys and instruments.
        let all_keys = Note::all_keys(false);
        let all_instruments = Self::all_instruments(user_config)?;

        // Generate a lesson for each key and instrument pair.
        all_keys
            .iter()
            .flat_map(|key| {
                all_instruments.iter().map(|instrument| {
                    self.generate_basic_harmony_lesson(course_manifest, *key, *instrument)
                })
            })
            .collect::<Result<Vec<_>>>()
    }

    /// Returns the ID of the advanced harmony lesson for the given course, key, and instrument.
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

    /// Generates the advanced harmony lesson for the given key, instrument, and passage.
    fn generate_advanced_harmony_exercise(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        key: Note,
        instrument: Option<&str>,
        passage: (usize, &ImprovisationPassage),
    ) -> Result<ExerciseManifest> {
        // Generate the exercise name.
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

        // Generate the exercise manifest.
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

    /// Generates the advanced harmony lesson for the given key and instrument.
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
                "{} - Advanced Harmony - Key of {} - Sight-singing",
                course_manifest.name,
                key.to_string()
            ),
        };

        // Declare the lesson dependencies based on the previous key in the circle of fifths and the
        // instrument.
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

        // Generate the lesson manifest.
        let instrument_name = match instrument {
            Some(instrument) => instrument.to_string(),
            None => "voice".to_string(),
        };
        let lesson_manifest = LessonManifest {
            id: lesson_id,
            course_id: course_manifest.id,
            name: lesson_name,
            description: Some(ADVANCED_HARMONY_DESCRIPTION.to_string()),
            dependencies: lesson_dependencies,
            metadata: Some(BTreeMap::from([
                (COURSE_METADATA.to_string(), vec!["true".to_string()]),
                (
                    LESSON_METADATA.to_string(),
                    vec!["advanced_harmony".to_string()],
                ),
                (INSTRUMENT_METADATA.to_string(), vec![instrument_name]),
                (KEY_METADATA.to_string(), vec![key.to_string()]),
            ])),
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *ADVANCED_HARMONY_INSTRUCTIONS,
            }),
            lesson_material: None,
        };

        // Generate an exercise for each passage.
        let exercises = self
            .passages
            .iter()
            .map(|(index, passage)| {
                self.generate_advanced_harmony_exercise(
                    course_manifest,
                    lesson_manifest.id,
                    key,
                    instrument,
                    (*index, passage),
                )
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((lesson_manifest, exercises))
    }

    /// Generates all the advanced harmony lessons for the given course.
    fn generate_advanced_harmony_lessons(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationPreferences,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        // Get all keys and instruments.
        let all_keys = Note::all_keys(false);
        let all_instruments = Self::all_instruments(user_config)?;

        // Generate a lesson for each key and instrument pair.
        all_keys
            .iter()
            .flat_map(|key| {
                all_instruments.iter().map(|instrument| {
                    self.generate_advanced_harmony_lesson(course_manifest, *key, *instrument)
                })
            })
            .collect::<Result<Vec<_>>>()
    }

    /// Returns the ID of the mastery lesson for the given course and instrument.
    fn mastery_lesson_id(&self, course_id: Ustr, instrument: Option<&str>) -> Ustr {
        match instrument {
            Some(instrument) => Ustr::from(&format!("{}::mastery::{}", course_id, instrument)),
            None => Ustr::from(&format!("{}::mastery", course_id)),
        }
    }

    /// Generates the mastery exercise for the given instrument and passage.
    fn generate_mastery_exercise(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        instrument: Option<&str>,
        passage: (usize, &ImprovisationPassage),
    ) -> Result<ExerciseManifest> {
        // Generate the exercise name.
        let exercise_name = match instrument {
            Some(instrument) => format!("{} - Mastery - {}", course_manifest.name, instrument),
            None => format!("{} - Mastery - Sight-singing", course_manifest.name),
        };

        // Generate the exercise manifest.
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

    /// Generates the mastery lesson for the given instrument.
    fn generate_mastery_lesson(
        &self,
        course_manifest: &CourseManifest,
        instrument: Option<&str>,
    ) -> Result<(LessonManifest, Vec<ExerciseManifest>)> {
        // Generate the lesson ID and name.
        let lesson_id = self.mastery_lesson_id(course_manifest.id, instrument);
        let lesson_name = match instrument {
            Some(instrument) => format!("{} - Mastery - {}", course_manifest.name, instrument),
            None => format!("{} - Mastery - Sight-singing", course_manifest.name),
        };

        // The mastery lesson depends on the last rhythm, melody, and harmony lessons as well as the
        // sight-singing mastery lesson if the lesson is for an instrument.
        let last_keys = Note::last_keys_in_circle();
        let lesson_dependencies = last_keys
            .iter()
            .flat_map(|key| {
                let mut dependencies = vec![
                    self.rhythm_lesson_id(course_manifest.id, instrument),
                    self.melody_lesson_id(course_manifest.id, *key, instrument),
                    self.advanced_harmony_lesson_id(course_manifest.id, *key, instrument),
                ];
                if instrument.is_some() {
                    dependencies.push(self.mastery_lesson_id(course_manifest.id, None))
                }
                dependencies
            })
            .collect::<Vec<_>>();

        // Generate the lesson manifest.
        let instrument_name = match instrument {
            Some(instrument) => instrument.to_string(),
            None => "voice".to_string(),
        };
        let lesson_manifest = LessonManifest {
            id: lesson_id,
            course_id: course_manifest.id,
            name: lesson_name,
            description: Some(MASTERY_DESCRIPTION.to_string()),
            dependencies: lesson_dependencies,
            metadata: Some(BTreeMap::from([
                (COURSE_METADATA.to_string(), vec!["true".to_string()]),
                (LESSON_METADATA.to_string(), vec!["mastery".to_string()]),
                (INSTRUMENT_METADATA.to_string(), vec![instrument_name]),
            ])),
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *MASTERY_INSTRUCTIONS,
            }),
            lesson_material: None,
        };

        // Generate an exercise for each passage.
        let exercises = self
            .passages
            .iter()
            .map(|(index, passage)| {
                self.generate_mastery_exercise(
                    course_manifest,
                    lesson_manifest.id,
                    instrument,
                    (*index, passage),
                )
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((lesson_manifest, exercises))
    }

    /// Generates all the mastery lessons for the given course.
    fn generate_mastery_lessons(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationPreferences,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        let all_instruments = Self::all_instruments(user_config)?;
        let lessons = all_instruments
            .iter()
            .map(|instrument| self.generate_mastery_lesson(course_manifest, *instrument))
            .collect::<Result<Vec<_>>>()?;
        Ok(lessons)
    }

    /// Generates the manifests, but only for the rhythm lessons.
    fn generate_rhtyhm_only_manifests(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationPreferences,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        Ok(vec![
            self.generate_singing_lesson(course_manifest)?,
            self.generate_rhythm_lessons(course_manifest, user_config)?,
        ]
        .into_iter()
        .flatten()
        .collect())
    }

    /// Generates the manifests for all the rhythm, melody, and harmony lessons.
    fn generate_all_manifests(
        &self,
        course_manifest: &CourseManifest,
        user_config: &TraneImprovisationPreferences,
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
        preferences: &CourseGeneratorPreferences,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        let default_preferences = TraneImprovisationPreferences::default();
        let preferences = match &preferences.trane_improvisation {
            Some(preferences) => preferences,
            None => &default_preferences,
        };
        if self.rhythm_only {
            self.generate_rhtyhm_only_manifests(course_manifest, preferences)
        } else {
            self.generate_all_manifests(course_manifest, preferences)
        }
    }
}
