//! Defines a special course to teach improvisation based on a set of musical passages.
//!  
//! The improvisation course generator creates a course that teaches the user how to improvise based
//! on a set of musical passages. The passages are provided by the user, and the rhythmic, melodic
//! and harmonic elements of each passage are used to generate a series of lessons for each key and
//! for all the instruments the user selects.

mod constants;

use anyhow::{anyhow, bail, Result};
use indoc::formatdoc;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
};
use ustr::Ustr;

use super::*;
use crate::data::{
    music::notes::Note, BasicAsset, CourseManifest, ExerciseAsset, ExerciseManifest, ExerciseType,
    GenerateManifests, GeneratedCourse, LessonManifest, UserPreferences,
};
use constants::*;

//@<improvisation-passage
/// A single musical passage to be used in an improvisation course. A course can contain multiple
/// passages but all of those passages are assumed to have the same key or mode.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ImprovisationPassage {
    /// A unique ID to identify this passage. This ID is used to generate the IDs of the exercises
    /// which use this passage.
    pub id: String,

    /// The path to the file containing the passage.
    pub path: String,
}
//>@improvisation-passage

impl ImprovisationPassage {
    /// Generates an exercise asset for this passage with the given description.
    fn generate_exercise_asset(&self, description: &str) -> ExerciseAsset {
        ExerciseAsset::BasicAsset(BasicAsset::InlinedUniqueAsset {
            content: formatdoc! {"
                {}

                You should perform this exercise in the key and instrument (or sight-singing)
                stated in the lesson name, if any.

                The file containing the music sheet for this exercise is located at {}.
                Relative paths are relative to the root of the library.
            ", description, self.path,
            }
            .into(),
        })
    }
}

//@<improvisation-config
/// The configuration for creating a new improvisation course.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ImprovisationConfig {
    /// The dependencies on other improvisation courses. Specifying these dependencies here instead
    /// of the [CourseManifest](crate::data::CourseManifest) allows Trane to generate more
    /// fine-grained dependencies.
    #[serde(default)]
    pub improvisation_dependencies: Vec<Ustr>,

    /// If true, the course contains passages only have rhythm and contain no melodic nor harmonic
    /// information. Lessons to learn the melody and harmony of the passages will not be generated.
    #[serde(default)]
    pub rhythm_only: bool,

    /// The directory where the passages are stored. The name of each file (minus the extension)
    /// will be used to generate the ID for each exercise. Thus, each of those IDs must be unique.
    /// For example, files `passage.pdf` and `passage.ly` break this rule, even though they have
    /// unique file names.
    ///
    /// The directory can be written relative to the root of the course or as an absolute path. The
    /// first option is recommended.
    pub passage_directory: String,

    /// The list of file extensions that are allowed for the passages. For example, `["pdf", "ly"]`.
    /// All other files will be ignored.
    pub file_extensions: Vec<String>,
}
//>@improvisation-config

//@<improvisation-preferences
/// Settings for generating a new improvisation course that are specific to a user.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ImprovisationPreferences {
    /// The list of instruments the user wants to practice.
    #[serde(default)]
    pub instruments: Vec<Instrument>,

    /// The list of instruments that only use rhythm. Exercises for these instruments will only
    /// show up in the rhythm lessons.
    #[serde(default)]
    pub rhythm_instruments: Vec<Instrument>,
}
//>@improvisation-preferences

impl ImprovisationPreferences {
    /// Returns the list of instruments the user can practice in the rhythm lessons. A value of None
    /// represents the voice lessons which must be mastered before practicing specific instruments.
    fn rhythm_lesson_instruments(&self) -> Vec<Option<&Instrument>> {
        // Combine `None` with the list of instruments and rhythm-only instruments.
        let mut rhythm_instruments: Vec<Option<&Instrument>> = self
            .instruments
            .iter()
            .chain(self.rhythm_instruments.iter())
            .map(Some)
            .collect();
        rhythm_instruments.push(None);
        rhythm_instruments
    }

    /// Returns the list of instruments that the user can practice during a lesson (except for the
    /// rhythm lessons as explained in `rhythm_lesson_instruments`). A value of None represents the
    /// voice lessons which must be mastered before practicing specific instruments.
    fn lesson_instruments(&self) -> Vec<Option<&Instrument>> {
        // Combine `None` with the list of instruments.
        let mut lesson_instruments: Vec<Option<&Instrument>> =
            self.instruments.iter().map(Some).collect();
        lesson_instruments.push(None);
        lesson_instruments
    }
}

impl ImprovisationConfig {
    /// Returns the ID for a given exercise given the lesson ID and the exercise index.
    fn exercise_id(&self, lesson_id: Ustr, passage_id: &str) -> Ustr {
        Ustr::from(&format!("{lesson_id}::{passage_id}"))
    }

    /// Returns the ID of the singing lesson for the given course.
    fn singing_lesson_id(&self, course_id: Ustr) -> Ustr {
        Ustr::from(&format!("{course_id}::singing"))
    }

    /// Generates a singing exercises for the given passage.
    fn generate_singing_exercise(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        passage: &ImprovisationPassage,
    ) -> ExerciseManifest {
        ExerciseManifest {
            id: self.exercise_id(lesson_id, &passage.id),
            lesson_id,
            course_id: course_manifest.id,
            name: format!("{} - Singing", course_manifest.name),
            description: None,
            exercise_type: ExerciseType::Procedural,
            exercise_asset: passage.generate_exercise_asset(SINGING_DESCRIPTION),
        }
    }

    /// Generates the singing lesson for this course.
    fn generate_singing_lesson(
        &self,
        course_manifest: &CourseManifest,
        passages: &[ImprovisationPassage],
    ) -> Vec<(LessonManifest, Vec<ExerciseManifest>)> {
        // Generate the lesson manifest. The lesson depends on the singing lessons of all the other
        // improvisation courses listed as dependencies.
        let dependencies = self
            .improvisation_dependencies
            .iter()
            .map(|id| format!("{id}::singing").into())
            .collect();
        let lesson_manifest = LessonManifest {
            id: self.singing_lesson_id(course_manifest.id),
            course_id: course_manifest.id,
            name: format!("{} - Singing", course_manifest.name),
            description: Some(SINGING_DESCRIPTION.to_string()),
            dependencies,
            metadata: Some(BTreeMap::from([
                (LESSON_METADATA.to_string(), vec!["singing".to_string()]),
                (INSTRUMENT_METADATA.to_string(), vec!["voice".to_string()]),
            ])),
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *SINGING_INSTRUCTIONS,
            }),
            lesson_material: None,
        };

        // Generate an exercise for each passage.
        let exercises = passages
            .iter()
            .map(|passage| {
                self.generate_singing_exercise(course_manifest, lesson_manifest.id, passage)
            })
            .collect::<Vec<_>>();
        vec![(lesson_manifest, exercises)]
    }

    /// Returns the ID of the rhythm lesson for the given course and instrument.
    fn rhythm_lesson_id(&self, course_id: Ustr, instrument: Option<&Instrument>) -> Ustr {
        match instrument {
            Some(instrument) => Ustr::from(&format!("{}::rhythm::{}", course_id, instrument.id)),
            None => Ustr::from(&format!("{course_id}::rhythm")),
        }
    }

    /// Generates a rhythm exercise for the given instrument and passage.
    fn generate_rhythm_exercise(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        instrument: Option<&Instrument>,
        passage: &ImprovisationPassage,
    ) -> ExerciseManifest {
        // Generate the exercise name.
        let exercise_name = match instrument {
            Some(instrument) => format!("{} - Rhythm - {}", course_manifest.name, instrument.name),
            None => format!("{} - Rhythm - Sight-singing", course_manifest.name),
        };

        // Generate the exercise manifest.
        ExerciseManifest {
            id: self.exercise_id(lesson_id, &passage.id),
            lesson_id,
            course_id: course_manifest.id,
            name: exercise_name,
            description: None,
            exercise_type: ExerciseType::Procedural,
            exercise_asset: passage.generate_exercise_asset(RHYTHM_DESCRIPTION),
        }
    }

    /// Generates the rhythm lesson for the given instrument.
    fn generate_rhythm_lesson(
        &self,
        course_manifest: &CourseManifest,
        instrument: Option<&Instrument>,
        passages: &[ImprovisationPassage],
    ) -> (LessonManifest, Vec<ExerciseManifest>) {
        // Generate the lesson ID and name.
        let lesson_id = self.rhythm_lesson_id(course_manifest.id, instrument);
        let lesson_name = match instrument {
            Some(instrument) => format!("{} - Rhythm - {}", course_manifest.name, instrument.name),
            None => format!("{} - Rhythm - Sight-singing", course_manifest.name),
        };

        // Declare the dependencies of this lesson.
        let lesson_dependencies = match instrument {
            Some(_) => vec![self.rhythm_lesson_id(course_manifest.id, None)],
            None => vec![self.singing_lesson_id(course_manifest.id)],
        };

        // Generate the lesson manifest.
        let instrument_id = match instrument {
            Some(instrument) => instrument.id.to_string(),
            None => "voice".to_string(),
        };
        let lesson_manifest = LessonManifest {
            id: lesson_id,
            course_id: course_manifest.id,
            name: lesson_name,
            description: Some(RHYTHM_DESCRIPTION.to_string()),
            dependencies: lesson_dependencies,
            metadata: Some(BTreeMap::from([
                (LESSON_METADATA.to_string(), vec!["rhythm".to_string()]),
                (INSTRUMENT_METADATA.to_string(), vec![instrument_id]),
            ])),
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *RHYTHM_INSTRUCTIONS,
            }),
            lesson_material: None,
        };

        // Generate an exercise for each passage.
        let exercises = passages
            .iter()
            .map(|passage| {
                self.generate_rhythm_exercise(
                    course_manifest,
                    lesson_manifest.id,
                    instrument,
                    passage,
                )
            })
            .collect::<Vec<_>>();
        (lesson_manifest, exercises)
    }

    /// Generates all the rhythm lessons for this course.
    fn generate_rhythm_lessons(
        &self,
        course_manifest: &CourseManifest,
        preferences: &ImprovisationPreferences,
        passages: &[ImprovisationPassage],
    ) -> Vec<(LessonManifest, Vec<ExerciseManifest>)> {
        // Generate a lesson for each instrument.
        let lesson_instruments = preferences.rhythm_lesson_instruments();
        let lessons = lesson_instruments
            .iter()
            .map(|instrument| self.generate_rhythm_lesson(course_manifest, *instrument, passages))
            .collect::<Vec<_>>();
        lessons
    }

    /// Returns the ID of the melody lesson for the given course, key, and instrument.
    fn melody_lesson_id(
        &self,
        course_id: Ustr,
        key: Note,
        instrument: Option<&Instrument>,
    ) -> Ustr {
        match instrument {
            None => Ustr::from(&format!("{}::melody::{}", course_id, key.to_string())),
            Some(instrument) => Ustr::from(&format!(
                "{}::melody::{}::{}",
                course_id,
                key.to_string(),
                instrument.id
            )),
        }
    }

    /// Generates a melody exercise for the given key, instrument, and passage.
    fn generate_melody_exercise(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        key: Note,
        instrument: Option<&Instrument>,
        passage: &ImprovisationPassage,
    ) -> ExerciseManifest {
        // Generate the exercise name.
        let exercise_name = match instrument {
            Some(instrument) => format!(
                "{} - Melody - Key of {} - {}",
                course_manifest.name,
                key.to_string(),
                instrument.name
            ),
            None => format!(
                "{} - Melody - Key of {} - Sight-singing",
                course_manifest.name,
                key.to_string()
            ),
        };

        // Generate the exercise manifest.
        ExerciseManifest {
            id: self.exercise_id(lesson_id, &passage.id),
            lesson_id,
            course_id: course_manifest.id,
            name: exercise_name,
            description: None,
            exercise_type: ExerciseType::Procedural,
            exercise_asset: passage.generate_exercise_asset(MELODY_DESCRIPTION),
        }
    }

    /// Generates the melody lesson for the given key and instrument.
    fn generate_melody_lesson(
        &self,
        course_manifest: &CourseManifest,
        key: Note,
        instrument: Option<&Instrument>,
        passages: &[ImprovisationPassage],
    ) -> (LessonManifest, Vec<ExerciseManifest>) {
        // Generate the lesson ID and name.
        let lesson_id = self.melody_lesson_id(course_manifest.id, key, instrument);
        let lesson_name = match instrument {
            Some(instrument) => format!(
                "{} - Melody - Key of {} - {}",
                course_manifest.name,
                key.to_string(),
                instrument.name
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
        let instrument_id = match instrument {
            Some(instrument) => instrument.id.to_string(),
            None => "voice".to_string(),
        };
        let lesson_manifest = LessonManifest {
            id: lesson_id,
            course_id: course_manifest.id,
            name: lesson_name,
            description: Some(MELODY_DESCRIPTION.to_string()),
            dependencies: lesson_dependencies,
            metadata: Some(BTreeMap::from([
                (LESSON_METADATA.to_string(), vec!["melody".to_string()]),
                (INSTRUMENT_METADATA.to_string(), vec![instrument_id]),
                (KEY_METADATA.to_string(), vec![key.to_string()]),
            ])),
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *MELODY_INSTRUCTIONS,
            }),
            lesson_material: None,
        };

        // Generate an exercise for each passage.
        let exercises = passages
            .iter()
            .map(|passage| {
                self.generate_melody_exercise(
                    course_manifest,
                    lesson_manifest.id,
                    key,
                    instrument,
                    passage,
                )
            })
            .collect::<Vec<_>>();
        (lesson_manifest, exercises)
    }

    /// Generates all the melody lessons for the given course.
    fn generate_melody_lessons(
        &self,
        course_manifest: &CourseManifest,
        preferences: &ImprovisationPreferences,
        passages: &[ImprovisationPassage],
    ) -> Vec<(LessonManifest, Vec<ExerciseManifest>)> {
        // Get a list of all keys and instruments.
        let all_keys = Note::all_keys(false);
        let lesson_instruments = preferences.lesson_instruments();

        // Generate a lesson for each key and instrument pair.
        all_keys
            .iter()
            .flat_map(|key| {
                lesson_instruments.iter().map(|instrument| {
                    self.generate_melody_lesson(course_manifest, *key, *instrument, passages)
                })
            })
            .collect::<Vec<_>>()
    }

    /// Returns the ID of the basic harmony lesson for the given course, key, and instrument.
    fn basic_harmony_lesson_id(
        &self,
        course_id: Ustr,
        key: Note,
        instrument: Option<&Instrument>,
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
                instrument.id
            )),
        }
    }

    /// Generates the basic harmony lesson for the given key, instrument, and passage.
    fn generate_basic_harmony_exercise(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        key: Note,
        instrument: Option<&Instrument>,
        passage: &ImprovisationPassage,
    ) -> ExerciseManifest {
        // Generate the exercise name.
        let exercise_name = match instrument {
            Some(instrument) => format!(
                "{} - Basic Harmony - Key of {} - {}",
                course_manifest.name,
                key.to_string(),
                instrument.name
            ),
            None => format!(
                "{} - Basic Harmony - Key of {} - Sight-singing",
                course_manifest.name,
                key.to_string()
            ),
        };

        // Generate the exercise manifest.
        ExerciseManifest {
            id: self.exercise_id(lesson_id, &passage.id),
            lesson_id,
            course_id: course_manifest.id,
            name: exercise_name,
            description: None,
            exercise_type: ExerciseType::Procedural,
            exercise_asset: passage.generate_exercise_asset(BASIC_HARMONY_DESCRIPTION),
        }
    }

    /// Generates the basic harmony lesson for the given key and instrument.
    fn generate_basic_harmony_lesson(
        &self,
        course_manifest: &CourseManifest,
        key: Note,
        instrument: Option<&Instrument>,
        passages: &[ImprovisationPassage],
    ) -> (LessonManifest, Vec<ExerciseManifest>) {
        // Generate the lesson ID and name.
        let lesson_id = self.basic_harmony_lesson_id(course_manifest.id, key, instrument);
        let lesson_name = match instrument {
            Some(instrument) => format!(
                "{} - Basic Harmony - Key of {} - {}",
                course_manifest.name,
                key.to_string(),
                instrument.name
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
        let instrument_id = match instrument {
            Some(instrument) => instrument.id.to_string(),
            None => "voice".to_string(),
        };
        let lesson_manifest = LessonManifest {
            id: lesson_id,
            course_id: course_manifest.id,
            name: lesson_name,
            description: Some(BASIC_HARMONY_DESCRIPTION.to_string()),
            dependencies: lesson_dependencies,
            metadata: Some(BTreeMap::from([
                (
                    LESSON_METADATA.to_string(),
                    vec!["basic_harmony".to_string()],
                ),
                (INSTRUMENT_METADATA.to_string(), vec![instrument_id]),
                (KEY_METADATA.to_string(), vec![key.to_string()]),
            ])),
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *BASIC_HARMONY_INSTRUCTIONS,
            }),
            lesson_material: None,
        };

        // Generate an exercise for each passage.
        let exercises = passages
            .iter()
            .map(|passage| {
                self.generate_basic_harmony_exercise(
                    course_manifest,
                    lesson_manifest.id,
                    key,
                    instrument,
                    passage,
                )
            })
            .collect::<Vec<_>>();
        (lesson_manifest, exercises)
    }

    /// Generates all basic harmony lessons for the given course.
    fn generate_basic_harmony_lessons(
        &self,
        course_manifest: &CourseManifest,
        preferences: &ImprovisationPreferences,
        passages: &[ImprovisationPassage],
    ) -> Vec<(LessonManifest, Vec<ExerciseManifest>)> {
        // Get all keys and instruments.
        let all_keys = Note::all_keys(false);
        let lesson_instruments = preferences.lesson_instruments();

        // Generate a lesson for each key and instrument pair.
        all_keys
            .iter()
            .flat_map(|key| {
                lesson_instruments.iter().map(|instrument| {
                    self.generate_basic_harmony_lesson(course_manifest, *key, *instrument, passages)
                })
            })
            .collect::<Vec<_>>()
    }

    /// Returns the ID of the advanced harmony lesson for the given course, key, and instrument.
    fn advanced_harmony_lesson_id(
        &self,
        course_id: Ustr,
        key: Note,
        instrument: Option<&Instrument>,
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
                instrument.id
            )),
        }
    }

    /// Generates the advanced harmony lesson for the given key, instrument, and passage.
    fn generate_advanced_harmony_exercise(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        key: Note,
        instrument: Option<&Instrument>,
        passage: &ImprovisationPassage,
    ) -> ExerciseManifest {
        // Generate the exercise name.
        let exercise_name = match instrument {
            Some(instrument) => format!(
                "{} - Advanced Harmony - Key of {} - {}",
                course_manifest.name,
                key.to_string(),
                instrument.name
            ),
            None => format!(
                "{} - Advanced Harmony - Key of {}",
                course_manifest.name,
                key.to_string()
            ),
        };

        // Generate the exercise manifest.
        ExerciseManifest {
            id: self.exercise_id(lesson_id, &passage.id),
            lesson_id,
            course_id: course_manifest.id,
            name: exercise_name,
            description: None,
            exercise_type: ExerciseType::Procedural,
            exercise_asset: passage.generate_exercise_asset(ADVANCED_HARMONY_DESCRIPTION),
        }
    }

    /// Generates the advanced harmony lesson for the given key and instrument.
    fn generate_advanced_harmony_lesson(
        &self,
        course_manifest: &CourseManifest,
        key: Note,
        instrument: Option<&Instrument>,
        passages: &[ImprovisationPassage],
    ) -> (LessonManifest, Vec<ExerciseManifest>) {
        let lesson_id = self.advanced_harmony_lesson_id(course_manifest.id, key, instrument);
        let lesson_name = match instrument {
            Some(instrument) => format!(
                "{} - Advanced Harmony - Key of {} - {}",
                course_manifest.name,
                key.to_string(),
                instrument.name
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
        let instrument_id = match instrument {
            Some(instrument) => instrument.id.to_string(),
            None => "voice".to_string(),
        };
        let lesson_manifest = LessonManifest {
            id: lesson_id,
            course_id: course_manifest.id,
            name: lesson_name,
            description: Some(ADVANCED_HARMONY_DESCRIPTION.to_string()),
            dependencies: lesson_dependencies,
            metadata: Some(BTreeMap::from([
                (
                    LESSON_METADATA.to_string(),
                    vec!["advanced_harmony".to_string()],
                ),
                (INSTRUMENT_METADATA.to_string(), vec![instrument_id]),
                (KEY_METADATA.to_string(), vec![key.to_string()]),
            ])),
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *ADVANCED_HARMONY_INSTRUCTIONS,
            }),
            lesson_material: None,
        };

        // Generate an exercise for each passage.
        let exercises = passages
            .iter()
            .map(|passage| {
                self.generate_advanced_harmony_exercise(
                    course_manifest,
                    lesson_manifest.id,
                    key,
                    instrument,
                    passage,
                )
            })
            .collect::<Vec<_>>();
        (lesson_manifest, exercises)
    }

    /// Generates all the advanced harmony lessons for the given course.
    fn generate_advanced_harmony_lessons(
        &self,
        course_manifest: &CourseManifest,
        preferences: &ImprovisationPreferences,
        passages: &[ImprovisationPassage],
    ) -> Vec<(LessonManifest, Vec<ExerciseManifest>)> {
        // Get all keys and instruments.
        let all_keys = Note::all_keys(false);
        let lesson_instruments = preferences.lesson_instruments();

        // Generate a lesson for each key and instrument pair.
        all_keys
            .iter()
            .flat_map(|key| {
                lesson_instruments.iter().map(|instrument| {
                    self.generate_advanced_harmony_lesson(
                        course_manifest,
                        *key,
                        *instrument,
                        passages,
                    )
                })
            })
            .collect::<Vec<_>>()
    }

    /// Returns the ID of the mastery lesson for the given course and instrument.
    fn mastery_lesson_id(&self, course_id: Ustr, instrument: Option<&Instrument>) -> Ustr {
        match instrument {
            Some(instrument) => Ustr::from(&format!("{}::mastery::{}", course_id, instrument.id)),
            None => Ustr::from(&format!("{course_id}::mastery")),
        }
    }

    /// Generates the mastery exercise for the given instrument and passage.
    fn generate_mastery_exercise(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        instrument: Option<&Instrument>,
        passage: &ImprovisationPassage,
    ) -> ExerciseManifest {
        // Generate the exercise name.
        let exercise_name = match instrument {
            Some(instrument) => format!("{} - Mastery - {}", course_manifest.name, instrument.name),
            None => format!("{} - Mastery - Sight-singing", course_manifest.name),
        };

        // Generate the exercise manifest.
        ExerciseManifest {
            id: self.exercise_id(lesson_id, &passage.id),
            lesson_id,
            course_id: course_manifest.id,
            name: exercise_name,
            description: None,
            exercise_type: ExerciseType::Procedural,
            exercise_asset: passage.generate_exercise_asset(MASTERY_DESCRIPTION),
        }
    }

    /// Generates the mastery lesson for the given instrument.
    fn generate_mastery_lesson(
        &self,
        course_manifest: &CourseManifest,
        instrument: Option<&Instrument>,
        passages: &[ImprovisationPassage],
    ) -> (LessonManifest, Vec<ExerciseManifest>) {
        // Generate the lesson ID and name.
        let lesson_id = self.mastery_lesson_id(course_manifest.id, instrument);
        let lesson_name = match instrument {
            Some(instrument) => format!("{} - Mastery - {}", course_manifest.name, instrument.name),
            None => format!("{} - Mastery - Sight-singing", course_manifest.name),
        };

        // The mastery lesson depends on the last rhythm, melody, and harmony lessons as well as the
        // sight-singing mastery lesson if the lesson is for an instrument.
        let last_keys = Note::last_keys_in_circle(false);
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
        let instrument_id = match instrument {
            Some(instrument) => instrument.id.to_string(),
            None => "voice".to_string(),
        };
        let lesson_manifest = LessonManifest {
            id: lesson_id,
            course_id: course_manifest.id,
            name: lesson_name,
            description: Some(MASTERY_DESCRIPTION.to_string()),
            dependencies: lesson_dependencies,
            metadata: Some(BTreeMap::from([
                (LESSON_METADATA.to_string(), vec!["mastery".to_string()]),
                (INSTRUMENT_METADATA.to_string(), vec![instrument_id]),
            ])),
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *MASTERY_INSTRUCTIONS,
            }),
            lesson_material: None,
        };

        // Generate an exercise for each passage.
        let exercises = passages
            .iter()
            .map(|passage| {
                self.generate_mastery_exercise(
                    course_manifest,
                    lesson_manifest.id,
                    instrument,
                    passage,
                )
            })
            .collect::<Vec<_>>();
        (lesson_manifest, exercises)
    }

    /// Generates all the mastery lessons for the given course.
    fn generate_mastery_lessons(
        &self,
        course_manifest: &CourseManifest,
        preferences: &ImprovisationPreferences,
        passages: &[ImprovisationPassage],
    ) -> Vec<(LessonManifest, Vec<ExerciseManifest>)> {
        let lesson_instruments = preferences.lesson_instruments();
        lesson_instruments
            .iter()
            .map(|instrument| self.generate_mastery_lesson(course_manifest, *instrument, passages))
            .collect::<Vec<_>>()
    }

    /// Extracts the passage ID from the given file name.
    fn extract_passage_id(file_name: &str) -> String {
        // The passage ID is the file name without the final extension. If the file has no
        // extension, the entire file name is used as the ID.
        file_name
            .rsplitn(2, '.')
            .last()
            .unwrap_or(file_name)
            .to_string()
    }

    /// Reads all the files in the passage directory to generate the list of all the passages
    /// included in the course.
    fn open_passage_directory(&self, course_root: &Path) -> Result<Vec<ImprovisationPassage>> {
        // Create the list of passages and a set of seen passage IDs to detect duplicates.
        let mut passages = Vec::new();
        let mut seen_passage_ids = HashSet::new();

        // Read all the files in the passage directory.
        let passage_dir = course_root.join(&self.passage_directory);
        for entry in std::fs::read_dir(passage_dir)? {
            // Only files inside the passage directory are considered.
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }

            // Extract the file name from the entry and use it to generate the passage ID.
            let path = entry.path();
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| anyhow!("Failed to get the file name"))? // grcov-excl-line
                .to_string();
            let passage_id = Self::extract_passage_id(&file_name);

            // Ignore files that don't have a valid extension.
            let valid_extension = self
                .file_extensions
                .iter()
                .any(|extension| file_name.ends_with(&format!(".{extension}")));
            if !valid_extension {
                continue;
            }

            // Fail if the passage ID has already been seen.
            if seen_passage_ids.contains(&passage_id) {
                bail!("Duplicate passage ID: {}", passage_id);
            }
            seen_passage_ids.insert(passage_id.clone());

            // Create the improvisation passage and add it to the list. It's ok to unwrap here as
            // an invalid file name would have been caught above.
            let passage = ImprovisationPassage {
                id: passage_id,
                path: entry.path().as_os_str().to_str().unwrap().to_string(),
            };
            passages.push(passage);
        }
        Ok(passages)
    }

    /// Generates the manifests, but only for the rhythm lessons.
    fn generate_rhtyhm_only_manifests(
        &self,
        course_manifest: &CourseManifest,
        preferences: &ImprovisationPreferences,
        passages: Vec<ImprovisationPassage>,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        Ok(vec![
            self.generate_singing_lesson(course_manifest, &passages),
            self.generate_rhythm_lessons(course_manifest, preferences, &passages),
        ]
        .into_iter()
        .flatten()
        .collect())
    }

    /// Generates the manifests for all the rhythm, melody, and harmony lessons.
    fn generate_all_manifests(
        &self,
        course_manifest: &CourseManifest,
        preferences: &ImprovisationPreferences,
        passages: Vec<ImprovisationPassage>,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        Ok(vec![
            self.generate_singing_lesson(course_manifest, &passages),
            self.generate_rhythm_lessons(course_manifest, preferences, &passages),
            self.generate_melody_lessons(course_manifest, preferences, &passages),
            self.generate_basic_harmony_lessons(course_manifest, preferences, &passages),
            self.generate_advanced_harmony_lessons(course_manifest, preferences, &passages),
            self.generate_mastery_lessons(course_manifest, preferences, &passages),
        ]
        .into_iter()
        .flatten()
        .collect())
    }
}

impl GenerateManifests for ImprovisationConfig {
    fn generate_manifests(
        &self,
        course_root: &Path,
        course_manifest: &CourseManifest,
        preferences: &UserPreferences,
    ) -> Result<GeneratedCourse> {
        // Get the user's preferences for this course or use the default preferences if none are
        // specified.
        let default_preferences = ImprovisationPreferences::default();
        let preferences = match &preferences.improvisation {
            Some(preferences) => preferences,
            None => &default_preferences,
        };

        // Read the passages from the passage directory.
        let passages = self.open_passage_directory(course_root)?;

        // Generate the lesson and exercise manifests.
        let lessons = if self.rhythm_only {
            self.generate_rhtyhm_only_manifests(course_manifest, preferences, passages)?
        } else {
            self.generate_all_manifests(course_manifest, preferences, passages)?
        };

        // Update the course's metadata and instructions.
        let mut metadata = course_manifest.metadata.clone().unwrap_or_default();
        metadata.insert(COURSE_METADATA.to_string(), vec!["true".to_string()]);
        let instructions = if course_manifest.course_instructions.is_none() {
            Some(BasicAsset::InlinedUniqueAsset {
                content: *COURSE_INSTRUCTIONS,
            })
        } else {
            None
        };

        Ok(GeneratedCourse {
            lessons,
            updated_metadata: Some(metadata),
            updated_instructions: instructions,
        })
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use indoc::indoc;
    use lazy_static::lazy_static;
    use std::fs::{self, File};
    use ustr::Ustr;

    use super::*;
    use crate::data::{
        music::notes::Note, BasicAsset, CourseGenerator, CourseManifest, GenerateManifests,
        UserPreferences,
    };

    /// A key to use for testing.
    const TEST_KEY: Note = Note::C;

    lazy_static! {
        /// An instrument to use for testing.
        static ref TEST_INSTRUMENT: Instrument = Instrument{
            name: "Test Instrument".to_string(),
            id: "test_instrument".to_string(),
        };

        /// A rhythm-only instrument to use for testing.
        static ref TEST_RHYTHM_ONLY_INSTRUMENT: Instrument = Instrument{
            name: "Test Rhythm-Only Instrument".to_string(),
            id: "test_rhythm_only_instrument".to_string(),
        };

        static ref TEST_PREFERENCES: ImprovisationPreferences = ImprovisationPreferences{
            instruments: vec![TEST_INSTRUMENT.clone()],
            rhythm_instruments: vec![TEST_RHYTHM_ONLY_INSTRUMENT.clone()],
        };
    }

    /// Verifies that the instructions for the course are not replaced if they are already set.
    #[test]
    fn do_not_replace_existing_instructions() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        fs::create_dir(temp_dir.path().join("passages"))?;

        let course_generator = CourseGenerator::Improvisation(ImprovisationConfig {
            rhythm_only: false,
            improvisation_dependencies: vec![],
            passage_directory: "passages".to_string(),
            file_extensions: vec!["ly".to_string()],
        });
        let course_manifest = CourseManifest {
            id: Ustr::from("testID"),
            name: "Test".to_string(),
            description: None,
            dependencies: vec![],
            authors: None,
            metadata: None,
            course_instructions: Some(BasicAsset::InlinedAsset {
                content: "test".to_string(),
            }),
            course_material: None,
            generator_config: Some(course_generator.clone()),
        };
        let preferences = UserPreferences::default();
        let generated_course =
            course_generator.generate_manifests(temp_dir.path(), &course_manifest, &preferences)?;
        assert!(generated_course.updated_instructions.is_none());
        Ok(())
    }

    /// Verifies cloning a passage. Done so that the auto-generated trait implementation is included
    /// in the code coverage reports.
    #[test]
    fn passage_clone() {
        let passage = super::ImprovisationPassage {
            id: "test".to_string(),
            path: "test".to_string(),
        };
        let passage_clone = passage.clone();
        assert_eq!(passage.id, passage_clone.id);
        assert_eq!(passage.path, passage_clone.path);
    }

    /// Verifies extracting the passage ID from a file name.
    #[test]
    fn extract_passage_id() {
        assert_eq!(ImprovisationConfig::extract_passage_id("a"), "a");
        assert_eq!(ImprovisationConfig::extract_passage_id("a.b"), "a");
        assert_eq!(ImprovisationConfig::extract_passage_id("a.b.c"), "a.b");
        assert_eq!(ImprovisationConfig::extract_passage_id("."), "");
        assert_eq!(ImprovisationConfig::extract_passage_id("..."), "..");
        assert_eq!(ImprovisationConfig::extract_passage_id(""), "");
    }

    /// Verifies creating an exercise ID.
    #[test]
    fn exercise_id() {
        let config = ImprovisationConfig::default();
        assert_eq!(config.exercise_id("a".into(), "b"), "a::b");
    }

    /// Verifies creating the singing lesson ID.
    #[test]
    fn singing_lesson_id() {
        let config = ImprovisationConfig::default();
        assert_eq!(config.singing_lesson_id("a".into()), "a::singing");
    }

    /// Verifies creating the rhythm lesson IDs.
    #[test]
    fn rhythm_lesson_id() {
        let config = ImprovisationConfig::default();
        assert_eq!(config.rhythm_lesson_id("a".into(), None), "a::rhythm");
        assert_eq!(
            config.rhythm_lesson_id("a".into(), Some(&TEST_INSTRUMENT)),
            "a::rhythm::test_instrument"
        );
    }

    /// Verifies creating the melody lesson IDs.
    #[test]
    fn melody_lesson_id() {
        let config = ImprovisationConfig::default();
        assert_eq!(
            config.melody_lesson_id("a".into(), TEST_KEY, None),
            "a::melody::C"
        );
        assert_eq!(
            config.melody_lesson_id("a".into(), TEST_KEY, Some(&TEST_INSTRUMENT)),
            "a::melody::C::test_instrument"
        );
    }

    /// Verifies creating the basic harmony lesson IDs.
    #[test]
    fn basic_harmony_lesson_id() {
        let config = ImprovisationConfig::default();
        assert_eq!(
            config.basic_harmony_lesson_id("a".into(), TEST_KEY, None),
            "a::basic_harmony::C"
        );
        assert_eq!(
            config.basic_harmony_lesson_id("a".into(), TEST_KEY, Some(&TEST_INSTRUMENT)),
            "a::basic_harmony::C::test_instrument"
        );
    }

    /// Verifies creating the advanced harmony lesson IDs.
    #[test]
    fn advanced_harmony_lesson_id() {
        let config = ImprovisationConfig::default();
        assert_eq!(
            config.advanced_harmony_lesson_id("a".into(), TEST_KEY, None),
            "a::advanced_harmony::C"
        );
        assert_eq!(
            config.advanced_harmony_lesson_id("a".into(), TEST_KEY, Some(&TEST_INSTRUMENT)),
            "a::advanced_harmony::C::test_instrument"
        );
    }

    /// Verifies creating the mastery lesson ID.
    #[test]
    fn mastery_lesson_id() {
        let config = ImprovisationConfig::default();
        assert_eq!(config.mastery_lesson_id("a".into(), None), "a::mastery");
        assert_eq!(
            config.mastery_lesson_id("a".into(), Some(&TEST_INSTRUMENT)),
            "a::mastery::test_instrument"
        );
    }

    /// Verifies the correct instruments are selected for the rhythm lesson.
    #[test]
    fn rhythm_lesson_instruments() {
        let instruments = TEST_PREFERENCES.rhythm_lesson_instruments();
        let expected_instruments = vec![
            Some(&*TEST_INSTRUMENT),
            Some(&*TEST_RHYTHM_ONLY_INSTRUMENT),
            None,
        ];
        assert_eq!(instruments, expected_instruments);
    }

    /// Verifies the correct instruments are selected for all the other lessons.
    #[test]
    fn lesson_instruments() {
        let instruments = TEST_PREFERENCES.lesson_instruments();
        let expected_instruments = vec![Some(&*TEST_INSTRUMENT), None];
        assert_eq!(instruments, expected_instruments);
    }

    /// Verifies generating an exercise asset from an improvisation passage.
    #[test]
    fn generate_exercise_asset() {
        let passage = super::ImprovisationPassage {
            id: "test".to_string(),
            path: "test".to_string(),
        };
        let asset = passage.generate_exercise_asset("My description");
        let expected_asset = ExerciseAsset::BasicAsset(BasicAsset::InlinedUniqueAsset {
            content: indoc! {"
                My description

                You should perform this exercise in the key and instrument (or sight-singing)
                stated in the lesson name, if any.

                The file containing the music sheet for this exercise is located at test.
                Relative paths are relative to the root of the library.
            "}
            .into(),
        });
        assert!(matches!(
            asset,
            ExerciseAsset::BasicAsset(BasicAsset::InlinedUniqueAsset { .. })
        ));
        assert_eq!(asset, expected_asset);
    }

    /// Verifies that the passages from directory are correctly generated.
    #[test]
    fn open_passage_directory() -> Result<()> {
        // Create a course with a couple of passages.
        let temp_dir = tempfile::tempdir()?;
        let passages_dir = temp_dir.path().join("passages");
        fs::create_dir(&passages_dir)?;
        let passage1 = passages_dir.join("passage1.ly");
        let passage2 = passages_dir.join("passage2.pdf");
        File::create(&passage1)?;
        File::create(&passage2)?;

        // Create a passage with an invalid extension. It should be ignored.
        let passage3 = passages_dir.join("passage1.md");
        File::create(&passage3)?;

        let mut config = ImprovisationConfig::default();
        config.passage_directory = passages_dir.to_str().unwrap().into();
        config.file_extensions = vec!["ly".into(), "pdf".into()];
        let passages = config.open_passage_directory(temp_dir.path())?;

        assert_eq!(passages.len(), 2);
        assert!(passages.contains(&ImprovisationPassage {
            id: "passage1".into(),
            path: passage1.to_str().unwrap().into(),
        }));
        assert!(passages.contains(&ImprovisationPassage {
            id: "passage2".into(),
            path: passage2.to_str().unwrap().into(),
        }));
        Ok(())
    }
}
