//! Defines a special course to teach transcription based on a set of musical passages.
//!  
//! This course generator is similar to the improvisation course generator, but the passages are
//! provided as actual musical recordings instead of music sheet. The student is expected to listen
//! to the passages to internalize the sounds, and then transcribe the passages to their instruments
//! and use them as a basis for improvisation. It is not required to use solfege syllables or
//! numbers nor to notate the passages. This course is meant to replicate the process of listenting
//! and imitation that is used in traditional music education and eventually became the method on
//! which Jazz was aurally transmitted.

pub mod constants;

use anyhow::{anyhow, Context, Result};
use indoc::formatdoc;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::BufReader,
    path::Path,
};
use ustr::Ustr;

use crate::data::{
    BasicAsset, CourseManifest, ExerciseAsset, ExerciseManifest, ExerciseType, GenerateManifests,
    GeneratedCourse, LessonManifest, UserPreferences,
};
use constants::*;

/// An asset used for the transcription course generator.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TranscriptionAsset {
    // TODO: check short ID uniqueness.
    /// A unique short ID for the asset. This value will be used to generate the exercise IDs.
    pub id: String,

    /// The name of the track to use for transcription.
    pub track_name: String,

    /// The name of the artist(s) who performs the track.
    pub artist_name: String,

    /// The name of the album in which the track appears.
    pub album_name: String,

    /// A link to an external copy (e.g. youtube video) of the track.
    pub external_link: Option<String>,
}

/// A collection of passages from a track that can be used for a transcription course.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TranscriptionPassages {
    /// The asset to transcribe.
    pub asset: TranscriptionAsset,

    /// The ranges `[start, end]` of the passages to transcribe. Stored as a map maping a unique ID
    /// to the start and end of the passage. A map is used to get the indices instead of getting
    /// them from a vector because reordering the passages would change the resulting exercise IDs.
    pub intervals: HashMap<usize, (String, String)>,
}

impl TranscriptionPassages {
    /// Generates the exercise assets for these passages with the given description.
    fn generate_exercise_asset(&self, description: &str, start: &str, end: &str) -> ExerciseAsset {
        // TODO: ensure id is valid.
        ExerciseAsset::BasicAsset(BasicAsset::InlinedUniqueAsset {
            content: formatdoc! {"
                {}

                The passage to transcribe is the following:
                    - Track name: {}
                    - Artist name: {}
                    - Album name: {}
                    - External link: {}
                    - Passage interval: {} - {}
                ",
                description, self.asset.track_name, self.asset.artist_name, self.asset.album_name,
                self.asset.external_link.as_deref().unwrap_or(""), start, end
            }
            .into(),
        })
    }
}

impl TranscriptionPassages {
    fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| anyhow!("cannot open knowledge base file {}", path.display()))?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader)
            .with_context(|| anyhow!("cannot parse knowledge base file {}", path.display()))
    }
}

/// Describes an instrument that can be used to practice in a transcription course.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Instrument {
    /// The name of the instrument. For example, "Tenor Saxophone".
    pub name: String,

    /// An ID for this instrument used to generate lesson IDs. For example, "tenor_saxophone".
    pub id: String,
}

/// Settings for generating a new transcription course that are specific to a user.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TranscriptionPreferences {
    /// The list of instruments the user wants to practice.
    pub instruments: Vec<Instrument>,
}

/// The configuration used to generate a transcription course.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TranscriptionConfig {
    /// The dependencies on other transcription courses. Specifying these dependencies here instead
    /// of the [CourseManifest](crate::data::CourseManifest) allows Trane to generate more
    /// fine-grained dependencies.
    pub improvisation_dependencies: Vec<Ustr>,

    /// The directory where the passages are stored as JSON files whose contents are serialized
    /// [TranscriptionPassages] objects. The name of each JSON file (minus the extension) will be
    /// used to generate the ID for each exercise.
    ///
    /// The directory can be written relative to the root of the course or as an absolute path. The
    /// first option is recommended.
    pub passage_directory: String,
}

impl TranscriptionConfig {
    /// Returns the ID for a given exercise given the lesson ID and the exercise index.
    fn exercise_id(&self, lesson_id: Ustr, asset_id: &str, passage_id: usize) -> Ustr {
        Ustr::from(&format!("{}::{}::{}", lesson_id, asset_id, passage_id))
    }

    /// Returns the ID of the singing lesson for the given course.
    fn singing_lesson_id(course_id: Ustr) -> Ustr {
        Ustr::from(&format!("{}::singing", course_id))
    }

    /// Generates the singing exercises for the given passages.
    fn generate_singing_exercises(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        passages: &TranscriptionPassages,
    ) -> Vec<ExerciseManifest> {
        passages
            .intervals
            .iter()
            .map(|(passage_id, (start, end))| ExerciseManifest {
                id: self.exercise_id(lesson_id, &passages.asset.id, *passage_id),
                lesson_id,
                course_id: course_manifest.id,
                name: format!("{} - Singing", course_manifest.name),
                description: None,
                exercise_type: ExerciseType::Procedural,
                exercise_asset: passages.generate_exercise_asset(SINGING_DESCRIPTION, start, end),
            })
            .collect()
    }

    /// Generates the lesson and exercise manifests for the singing lesson.
    fn generate_singing_lesson(
        &self,
        course_manifest: &CourseManifest,
        passages: &[TranscriptionPassages],
    ) -> (LessonManifest, Vec<ExerciseManifest>) {
        // Generate the lesson manifest. The lesson depends on the singing lessons of all the other
        // transcription courses listed as dependencies.
        let dependencies = self
            .improvisation_dependencies
            .iter()
            .map(|id| format!("{}::singing", id).into())
            .collect();
        let lesson_manifest = LessonManifest {
            id: Self::singing_lesson_id(course_manifest.id),
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
            .flat_map(|passages| {
                self.generate_singing_exercises(course_manifest, lesson_manifest.id, passages)
            })
            .collect::<Vec<_>>();
        (lesson_manifest, exercises)
    }

    /// Returns the ID of the singing lesson for the given course.
    fn advanced_singing_lesson_id(course_id: Ustr) -> Ustr {
        Ustr::from(&format!("{}::advanced_singing", course_id))
    }

    /// Generates the advanced singing exercises for the given passages.
    fn generate_advanced_singing_exercises(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        passages: &TranscriptionPassages,
    ) -> Vec<ExerciseManifest> {
        passages
            .intervals
            .iter()
            .map(|(passage_id, (start, end))| ExerciseManifest {
                id: self.exercise_id(lesson_id, &passages.asset.id, *passage_id),
                lesson_id,
                course_id: course_manifest.id,
                name: format!("{} - Advanced Singing", course_manifest.name),
                description: None,
                exercise_type: ExerciseType::Procedural,
                exercise_asset: passages.generate_exercise_asset(
                    ADVANCED_SINGING_DESCRIPTION,
                    start,
                    end,
                ),
            })
            .collect()
    }

    /// Generates the lesson and exercise manifests for the advanced singing lesson.
    fn generate_advanced_singing_lesson(
        &self,
        course_manifest: &CourseManifest,
        passages: &[TranscriptionPassages],
    ) -> (LessonManifest, Vec<ExerciseManifest>) {
        // Generate the lesson manifest. The lesson depends on the singing lesson.
        let lesson_manifest = LessonManifest {
            id: Self::advanced_singing_lesson_id(course_manifest.id),
            course_id: course_manifest.id,
            name: format!("{} - Advanced Singing", course_manifest.name),
            description: Some(ADVANCED_SINGING_DESCRIPTION.to_string()),
            dependencies: vec![Self::singing_lesson_id(course_manifest.id)],
            metadata: Some(BTreeMap::from([
                (
                    LESSON_METADATA.to_string(),
                    vec!["advanced_singing".to_string()],
                ),
                (INSTRUMENT_METADATA.to_string(), vec!["voice".to_string()]),
            ])),
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *ADVANCED_SINGING_INSTRUCTIONS,
            }),
            lesson_material: None,
        };

        // Generate an exercise for each passage.
        let exercises = passages
            .iter()
            .flat_map(|passages| {
                self.generate_advanced_singing_exercises(
                    course_manifest,
                    lesson_manifest.id,
                    passages,
                )
            })
            .collect::<Vec<_>>();
        (lesson_manifest, exercises)
    }

    /// Returns the ID of the transcription lesson for the given course and instrument.
    fn transcription_lesson_id(course_id: Ustr, instrument: &Instrument) -> Ustr {
        format!("{}::transcription::{}", course_id, instrument.id).into()
    }

    /// Generates the transcription exercises for the given passages.
    fn generate_transcription_exercises(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        passages: &TranscriptionPassages,
        insturment: &Instrument,
    ) -> Vec<ExerciseManifest> {
        passages
            .intervals
            .iter()
            .map(|(passage_id, (start, end))| ExerciseManifest {
                id: self.exercise_id(lesson_id, &passages.asset.id, *passage_id),
                lesson_id,
                course_id: course_manifest.id,
                name: format!(
                    "{} - Transcription - {}",
                    course_manifest.name, insturment.name
                ),
                description: None,
                exercise_type: ExerciseType::Procedural,
                exercise_asset: passages.generate_exercise_asset(
                    TRANSCRIPTION_DESCRIPTION,
                    start,
                    end,
                ),
            })
            .collect()
    }

    /// Generates the lesson and exercise manifests for the transcription lesson with the given
    /// instrument.
    fn generate_transcription_lesson(
        &self,
        course_manifest: &CourseManifest,
        passages: &[TranscriptionPassages],
        instrument: &Instrument,
    ) -> (LessonManifest, Vec<ExerciseManifest>) {
        // Generate the lesson manifest. The lesson depends on the singing lesson.
        let lesson_manifest = LessonManifest {
            id: Self::transcription_lesson_id(course_manifest.id, instrument),
            course_id: course_manifest.id,
            name: format!(
                "{} - Transcription - {}",
                course_manifest.name, instrument.name
            ),
            description: Some(TRANSCRIPTION_DESCRIPTION.to_string()),
            dependencies: vec![Self::singing_lesson_id(course_manifest.id)],
            metadata: Some(BTreeMap::from([
                (
                    LESSON_METADATA.to_string(),
                    vec!["transcription".to_string()],
                ),
                (
                    INSTRUMENT_METADATA.to_string(),
                    vec![instrument.id.to_string()],
                ),
            ])),
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *TRANSCRIPTION_INSTRUCTIONS,
            }),
            lesson_material: None,
        };

        // Generate an exercise for each passage.
        let exercises = passages
            .iter()
            .flat_map(|passages| {
                self.generate_transcription_exercises(
                    course_manifest,
                    lesson_manifest.id,
                    passages,
                    instrument,
                )
            })
            .collect::<Vec<_>>();
        (lesson_manifest, exercises)
    }

    /// Generates the lesson and exercise manifests for the transcription lessons.
    fn generate_transcription_lessons(
        &self,
        course_manifest: &CourseManifest,
        preferences: &TranscriptionPreferences,
        passages: &[TranscriptionPassages],
    ) -> Vec<(LessonManifest, Vec<ExerciseManifest>)> {
        preferences
            .instruments
            .iter()
            .map(|instrument| {
                self.generate_transcription_lesson(course_manifest, passages, instrument)
            })
            .collect()
    }

    /// Returns the ID of the advanced transcription lesson for the given course and instrument.
    fn advanced_transcription_lesson_id(course_id: Ustr, instrument: &Instrument) -> Ustr {
        format!("{}::advanced_transcription::{}", course_id, instrument.id).into()
    }

    /// Generates the advanced transcription exercises for the given passages.
    fn generate_advanced_transcription_exercises(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        passages: &TranscriptionPassages,
        insturment: &Instrument,
    ) -> Vec<ExerciseManifest> {
        passages
            .intervals
            .iter()
            .map(|(passage_id, (start, end))| ExerciseManifest {
                id: self.exercise_id(lesson_id, &passages.asset.id, *passage_id),
                lesson_id,
                course_id: course_manifest.id,
                name: format!(
                    "{} - Advanced Transcription - {}",
                    course_manifest.name, insturment.name
                ),
                description: None,
                exercise_type: ExerciseType::Procedural,
                exercise_asset: passages.generate_exercise_asset(
                    ADVANCED_TRANSCRIPTION_DESCRIPTION,
                    start,
                    end,
                ),
            })
            .collect()
    }

    /// Generates the lesson and exercise manifests for the advanced transcription lesson with the
    /// given instrument.
    fn generate_advanced_transcription_lesson(
        &self,
        course_manifest: &CourseManifest,
        passages: &[TranscriptionPassages],
        instrument: &Instrument,
    ) -> (LessonManifest, Vec<ExerciseManifest>) {
        // Generate the lesson manifest. The lesson depends on the advanced singing lesson.
        let lesson_manifest = LessonManifest {
            id: Self::advanced_transcription_lesson_id(course_manifest.id, instrument),
            course_id: course_manifest.id,
            name: format!(
                "{} - Advanced Transcription - {}",
                course_manifest.name, instrument.name
            ),
            description: Some(ADVANCED_TRANSCRIPTION_DESCRIPTION.to_string()),
            dependencies: vec![Self::advanced_singing_lesson_id(course_manifest.id)],
            metadata: Some(BTreeMap::from([
                (
                    LESSON_METADATA.to_string(),
                    vec!["advanced_transcription".to_string()],
                ),
                (
                    INSTRUMENT_METADATA.to_string(),
                    vec![instrument.id.to_string()],
                ),
            ])),
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *ADVANCED_TRANSCRIPTION_INSTRUCTIONS,
            }),
            lesson_material: None,
        };

        // Generate exercises for each passage.
        let exercises = passages
            .iter()
            .flat_map(|passages| {
                self.generate_advanced_transcription_exercises(
                    course_manifest,
                    lesson_manifest.id,
                    passages,
                    instrument,
                )
            })
            .collect::<Vec<_>>();
        (lesson_manifest, exercises)
    }

    /// Generates the lesson and exercise manifests for the advanced transcription lessons.
    fn generate_advanced_transcription_lessons(
        &self,
        course_manifest: &CourseManifest,
        preferences: &TranscriptionPreferences,
        passages: &[TranscriptionPassages],
    ) -> Vec<(LessonManifest, Vec<ExerciseManifest>)> {
        preferences
            .instruments
            .iter()
            .map(|instrument| {
                self.generate_advanced_transcription_lesson(course_manifest, passages, instrument)
            })
            .collect()
    }

    /// Reads all the files in the passage directory to generate the list of all the passages
    /// included in the course.
    fn open_passage_directory(&self, course_root: &Path) -> Result<Vec<TranscriptionPassages>> {
        // Read all the files in the passage directory.
        let mut passages = Vec::new();
        let passage_dir = course_root.join(&self.passage_directory);
        for entry in std::fs::read_dir(passage_dir)? {
            // Only files inside the passage directory are considered.
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }

            // Extract the file name from the entry.
            let path = entry.path();
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| anyhow!("Failed to get the file name"))? // grcov-excl-line
                .to_string();

            // Ignore any non-JSON files.
            if !file_name.ends_with(".json") {
                continue;
            }

            // Open the file and parse it as a [TranscriptionPassages] object.
            let passage = TranscriptionPassages::open(&path)?;
            passages.push(passage);
        }
        Ok(passages)
    }

    /// Generates all the lesson and exercise manifests for the course.
    fn generate_lesson_manifests(
        &self,
        course_manifest: &CourseManifest,
        preferences: &TranscriptionPreferences,
        passages: Vec<TranscriptionPassages>,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        Ok(vec![
            vec![self.generate_singing_lesson(course_manifest, &passages)],
            vec![self.generate_advanced_singing_lesson(course_manifest, &passages)],
            self.generate_transcription_lessons(course_manifest, preferences, &passages),
            self.generate_advanced_transcription_lessons(course_manifest, preferences, &passages),
        ]
        .into_iter()
        .flatten()
        .collect())
    }
}

impl GenerateManifests for TranscriptionConfig {
    fn generate_manifests(
        &self,
        course_root: &Path,
        course_manifest: &CourseManifest,
        preferences: &UserPreferences,
    ) -> Result<GeneratedCourse> {
        // Get the user's preferences for this course or use the default preferences if none are
        // specified.
        let default_preferences = TranscriptionPreferences::default();
        let preferences = match &preferences.transcription {
            Some(preferences) => preferences,
            None => &default_preferences,
        };

        // Read the passages from the passage directory and generate the lesson and exercise
        // manifests.
        let passages = self.open_passage_directory(course_root)?;
        let lessons = self.generate_lesson_manifests(course_manifest, preferences, passages)?;

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
