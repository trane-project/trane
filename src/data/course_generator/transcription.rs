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

use anyhow::{anyhow, bail, Context, Result};
use indoc::formatdoc;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::File,
    io::BufReader,
    path::Path,
};
use ustr::Ustr;

use super::*;
use crate::data::{
    BasicAsset, CourseGenerator, CourseManifest, ExerciseAsset, ExerciseManifest, ExerciseType,
    GenerateManifests, GeneratedCourse, LessonManifest, UserPreferences,
};
use constants::*;

/// An asset used for the transcription course generator.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum TranscriptionAsset {
    /// A track of recorded music that is not included along with the course. Used to reference
    /// commercial music for which there is no legal way to distribute the audio.
    Track {
        /// A unique short ID for the asset. This value will be used to generate the exercise IDs.
        short_id: String,

        /// The name of the track to use for transcription.
        track_name: String,

        /// The name of the artist(s) who performs the track.
        artist_name: String,

        /// The name of the album in which the track appears.
        album_name: String,

        /// A link to an external copy (e.g. youtube video) of the track.
        #[serde(default)]
        external_link: Option<String>,
    },
}

impl TranscriptionAsset {
    /// Returns the short ID of the asset, which wil be used to generate the exercise IDs.
    pub fn short_id(&self) -> &str {
        match self {
            TranscriptionAsset::Track { short_id, .. } => short_id,
        }
    }
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
    fn generate_exercise_asset(
        &self,
        description: &str,
        start: &str,
        end: &str,
        instrument: Option<&Instrument>,
    ) -> ExerciseAsset {
        let instrument_instruction = match instrument {
            Some(instrument) => format!(
                "\nTranscribe the passage using the instrument: {}.\n",
                instrument.name
            ),
            None => "".into(),
        };
        match &self.asset {
            TranscriptionAsset::Track {
                track_name,
                artist_name,
                album_name,
                external_link,
                ..
            } => ExerciseAsset::BasicAsset(BasicAsset::InlinedUniqueAsset {
                content: formatdoc! {"
                    {}

                    The passage to transcribe is the following:
                        - Track name: {}
                        - Artist name: {}
                        - Album name: {}
                        - External link: {}
                        - Passage interval: {} - {}
                    {}",
                    description, track_name, artist_name, album_name,
                    external_link.as_deref().unwrap_or(""), start, end, instrument_instruction
                }
                .into(),
            }),
        }
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

/// Settings for generating a new transcription course that are specific to a user.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TranscriptionPreferences {
    /// The list of instruments the user wants to practice.
    #[serde(default)]
    pub instruments: Vec<Instrument>,
}

/// The configuration used to generate a transcription course.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TranscriptionConfig {
    /// The dependencies on other transcription courses. Specifying these dependencies here instead
    /// of the [CourseManifest](crate::data::CourseManifest) allows Trane to generate more
    /// fine-grained dependencies.
    #[serde(default)]
    pub transcription_dependencies: Vec<Ustr>,

    /// The directory where the passages are stored as JSON files whose contents are serialized
    /// [TranscriptionPassages] objects.
    ///
    /// The directory can be written relative to the root of the course or as an absolute path. The
    /// first option is recommended.
    pub passage_directory: String,

    /// If true, the course will skip the advanced singing and transcription lessons. This is useful
    /// when there are copies of the same recording for every key, which makes the need for the
    /// advanced lessons obsolete.
    #[serde(default)]
    pub skip_advanced_lessons: bool,
}

impl TranscriptionConfig {
    /// Returns the ID for a given exercise given the lesson ID and the exercise index.
    fn exercise_id(lesson_id: &Ustr, asset_id: &str, passage_id: usize) -> Ustr {
        Ustr::from(&format!("{lesson_id}::{asset_id}::{passage_id}"))
    }

    /// Returns the ID of the singing lesson for the given course.
    fn singing_lesson_id(course_id: &Ustr) -> Ustr {
        Ustr::from(&format!("{course_id}::singing"))
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
                id: Self::exercise_id(&lesson_id, passages.asset.short_id(), *passage_id),
                lesson_id,
                course_id: course_manifest.id,
                name: format!("{} - Singing", course_manifest.name),
                description: None,
                exercise_type: ExerciseType::Procedural,
                exercise_asset: passages.generate_exercise_asset(
                    SINGING_DESCRIPTION,
                    start,
                    end,
                    None,
                ),
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
            .transcription_dependencies
            .iter()
            .map(|id| format!("{id}::singing").into())
            .collect();
        let lesson_manifest = LessonManifest {
            id: Self::singing_lesson_id(&course_manifest.id),
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
    fn advanced_singing_lesson_id(course_id: &Ustr) -> Ustr {
        Ustr::from(&format!("{course_id}::advanced_singing"))
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
                id: Self::exercise_id(&lesson_id, passages.asset.short_id(), *passage_id),
                lesson_id,
                course_id: course_manifest.id,
                name: format!("{} - Advanced Singing", course_manifest.name),
                description: None,
                exercise_type: ExerciseType::Procedural,
                exercise_asset: passages.generate_exercise_asset(
                    ADVANCED_SINGING_DESCRIPTION,
                    start,
                    end,
                    None,
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
            id: Self::advanced_singing_lesson_id(&course_manifest.id),
            course_id: course_manifest.id,
            name: format!("{} - Advanced Singing", course_manifest.name),
            description: Some(ADVANCED_SINGING_DESCRIPTION.to_string()),
            dependencies: vec![Self::singing_lesson_id(&course_manifest.id)],
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
    fn transcription_lesson_id(course_id: &Ustr, instrument: &Instrument) -> Ustr {
        format!("{}::transcription::{}", course_id, instrument.id).into()
    }

    /// Generates the transcription exercises for the given passages.
    fn generate_transcription_exercises(
        &self,
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        passages: &TranscriptionPassages,
        instrument: &Instrument,
    ) -> Vec<ExerciseManifest> {
        passages
            .intervals
            .iter()
            .map(|(passage_id, (start, end))| ExerciseManifest {
                id: Self::exercise_id(&lesson_id, passages.asset.short_id(), *passage_id),
                lesson_id,
                course_id: course_manifest.id,
                name: format!(
                    "{} - Transcription - {}",
                    course_manifest.name, instrument.name
                ),
                description: None,
                exercise_type: ExerciseType::Procedural,
                exercise_asset: passages.generate_exercise_asset(
                    TRANSCRIPTION_DESCRIPTION,
                    start,
                    end,
                    Some(instrument),
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
            id: Self::transcription_lesson_id(&course_manifest.id, instrument),
            course_id: course_manifest.id,
            name: format!(
                "{} - Transcription - {}",
                course_manifest.name, instrument.name
            ),
            description: Some(TRANSCRIPTION_DESCRIPTION.to_string()),
            dependencies: vec![Self::singing_lesson_id(&course_manifest.id)],
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
    fn advanced_transcription_lesson_id(course_id: &Ustr, instrument: &Instrument) -> Ustr {
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
                id: Self::exercise_id(&lesson_id, passages.asset.short_id(), *passage_id),
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
                    Some(insturment),
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
            id: Self::advanced_transcription_lesson_id(&course_manifest.id, instrument),
            course_id: course_manifest.id,
            name: format!(
                "{} - Advanced Transcription - {}",
                course_manifest.name, instrument.name
            ),
            description: Some(ADVANCED_TRANSCRIPTION_DESCRIPTION.to_string()),
            dependencies: vec![
                Self::transcription_lesson_id(&course_manifest.id, instrument),
                Self::advanced_singing_lesson_id(&course_manifest.id),
            ],
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
        // Keep track of all the discovered passage IDs to detect duplicates.
        let mut passages = Vec::new();
        let mut seen_ids = HashSet::new();

        // Read all the files in the passage directory.
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

            // Open the file and parse it as a [TranscriptionPassages] object. Check for duplicate
            // short IDs.
            let passage = TranscriptionPassages::open(&path)?;
            let short_id = passage.asset.short_id();
            if seen_ids.contains(short_id) {
                bail!("Duplicate passage ID: {}", short_id);
            } else {
                seen_ids.insert(short_id.to_string());
            }
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
        let skip_advanced_lessons = if let Some(CourseGenerator::Transcription(config)) =
            &course_manifest.generator_config
        {
            config.skip_advanced_lessons
        } else {
            false // grcov-excl-line: This line should be unreachable.
        };

        if skip_advanced_lessons {
            Ok(vec![
                vec![self.generate_singing_lesson(course_manifest, &passages)],
                self.generate_transcription_lessons(course_manifest, preferences, &passages),
            ]
            .into_iter()
            .flatten()
            .collect())
        } else {
            Ok(vec![
                vec![self.generate_singing_lesson(course_manifest, &passages)],
                vec![self.generate_advanced_singing_lesson(course_manifest, &passages)],
                self.generate_transcription_lessons(course_manifest, preferences, &passages),
                self.generate_advanced_transcription_lessons(
                    course_manifest,
                    preferences,
                    &passages,
                ),
            ]
            .into_iter()
            .flatten()
            .collect())
        }
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

#[cfg(test)]
mod test {
    use anyhow::Result;
    use indoc::indoc;
    use std::{fs, io::Write};

    use crate::data::CourseGenerator;

    use super::*;

    /// Verifies generating IDs for the exercises in the course.
    #[test]
    fn exercise_id() {
        let lesson_id = Ustr::from("lesson_id");
        let asset_id = "asset_id";
        let passage_id = 2;
        assert_eq!(
            TranscriptionConfig::exercise_id(&lesson_id, &asset_id, passage_id),
            Ustr::from("lesson_id::asset_id::2")
        );
    }

    /// Verifies generating the lesson ID for the singing lesson.
    #[test]
    fn singing_lesson_id() {
        let course_id = Ustr::from("course_id");
        assert_eq!(
            TranscriptionConfig::singing_lesson_id(&course_id),
            Ustr::from("course_id::singing")
        );
    }

    /// Verifies generating the lesson ID for the advanced singing lesson.
    #[test]
    fn advanced_singing_lesson_id() {
        let course_id = Ustr::from("course_id");
        assert_eq!(
            TranscriptionConfig::advanced_singing_lesson_id(&course_id),
            Ustr::from("course_id::advanced_singing")
        );
    }

    /// Verifies generating the lesson ID for the transcription lesson.
    #[test]
    fn transcription_lesson_id() {
        let course_id = Ustr::from("course_id");
        let instrument = Instrument {
            name: "Piano".into(),
            id: "piano".into(),
        };
        assert_eq!(
            TranscriptionConfig::transcription_lesson_id(&course_id, &instrument),
            Ustr::from("course_id::transcription::piano"),
        );
    }

    /// Verifies generating the lesson ID for the advanced transcription lesson.
    #[test]
    fn advanced_transcription_lesson_id() {
        let course_id = Ustr::from("course_id");
        let instrument = Instrument {
            name: "Piano".into(),
            id: "piano".into(),
        };
        assert_eq!(
            TranscriptionConfig::advanced_transcription_lesson_id(&course_id, &instrument),
            Ustr::from("course_id::advanced_transcription::piano"),
        );
    }

    /// Verifies generating the asset for an exercise in the course.
    #[test]
    fn generate_exercise_asset() {
        let passages = TranscriptionPassages {
            asset: TranscriptionAsset::Track {
                short_id: "track".into(),
                track_name: "Track".into(),
                artist_name: "Artist".into(),
                album_name: "Album".into(),
                external_link: Some("https://example.com".into()),
            },
            intervals: HashMap::from([
                (1, ("0:00".into(), "0:01".into())),
                (2, ("0:01".into(), "0:02".into())),
            ]),
        };

        // Generate the asset when an instrument is specified.
        let instrument = Instrument {
            name: "Piano".into(),
            id: "piano".into(),
        };
        let exercise_asset =
            passages.generate_exercise_asset("My description", "0:00", "0:01", Some(&instrument));
        let expected_asset = ExerciseAsset::BasicAsset(BasicAsset::InlinedUniqueAsset {
            content: indoc! {"
                My description

                The passage to transcribe is the following:
                    - Track name: Track
                    - Artist name: Artist
                    - Album name: Album
                    - External link: https://example.com
                    - Passage interval: 0:00 - 0:01

                Transcribe the passage using the instrument: Piano.
            "}
            .into(),
        });
        assert_eq!(exercise_asset, expected_asset);

        // Generate the asset when an instrument is not specified.
        let exercise_asset =
            passages.generate_exercise_asset("My description", "0:00", "0:01", None);
        let expected_asset = ExerciseAsset::BasicAsset(BasicAsset::InlinedUniqueAsset {
            content: indoc! {"
                My description

                The passage to transcribe is the following:
                    - Track name: Track
                    - Artist name: Artist
                    - Album name: Album
                    - External link: https://example.com
                    - Passage interval: 0:00 - 0:01
            "}
            .into(),
        });
        assert_eq!(exercise_asset, expected_asset);
    }

    /// Verifies opening the passage directory.
    #[test]
    fn open_passage_directory() -> Result<()> {
        // Create the passages directory.
        let temp_dir = tempfile::tempdir()?;
        let passages_dir = temp_dir.path().join("passages");
        fs::create_dir(&passages_dir)?;

        // Write some test passages to the directory.
        let passages1 = TranscriptionPassages {
            asset: TranscriptionAsset::Track {
                short_id: "track1".into(),
                track_name: "Track 1".into(),
                artist_name: "Artist 1".into(),
                album_name: "Album 1".into(),
                external_link: None,
            },
            intervals: HashMap::from([(1, ("0:00".into(), "0:01".into()))]),
        };
        let passages2 = TranscriptionPassages {
            asset: TranscriptionAsset::Track {
                short_id: "track2".into(),
                track_name: "Track 2".into(),
                artist_name: "Artist 2".into(),
                album_name: "Album 2".into(),
                external_link: None,
            },
            intervals: HashMap::from([(1, ("0:00".into(), "0:01".into()))]),
        };
        File::create(passages_dir.join("passages1.json"))?
            .write_all(serde_json::to_string_pretty(&passages1).unwrap().as_bytes())?;
        File::create(passages_dir.join("passages2.json"))?
            .write_all(serde_json::to_string_pretty(&passages2).unwrap().as_bytes())?;

        // Open the passages directory and verify the passages.
        let config = TranscriptionConfig {
            passage_directory: "passages".into(),
            transcription_dependencies: vec![],
            skip_advanced_lessons: false,
        };
        let passages = config.open_passage_directory(&temp_dir.path())?;
        assert_eq!(2, passages.len());

        Ok(())
    }

    /// Verifies that opening the passage directory fails if there are passages with duplicate IDs.
    #[test]
    fn open_passage_directory_duplicate() -> Result<()> {
        // Create the passages directory.
        let temp_dir = tempfile::tempdir()?;
        let passages_dir = temp_dir.path().join("passages");
        fs::create_dir(&passages_dir)?;

        // Write some test passages to the directory. The passages have duplicate IDs.
        let passages1 = TranscriptionPassages {
            asset: TranscriptionAsset::Track {
                short_id: "track1".into(),
                track_name: "Track 1".into(),
                artist_name: "Artist 1".into(),
                album_name: "Album 1".into(),
                external_link: None,
            },
            intervals: HashMap::from([(1, ("0:00".into(), "0:01".into()))]),
        };
        let passages2 = TranscriptionPassages {
            asset: TranscriptionAsset::Track {
                short_id: "track1".into(),
                track_name: "Track 2".into(),
                artist_name: "Artist 2".into(),
                album_name: "Album 2".into(),
                external_link: None,
            },
            intervals: HashMap::from([(1, ("0:00".into(), "0:01".into()))]),
        };
        File::create(passages_dir.join("passages1.json"))?
            .write_all(serde_json::to_string_pretty(&passages1).unwrap().as_bytes())?;
        File::create(passages_dir.join("passages2.json"))?
            .write_all(serde_json::to_string_pretty(&passages2).unwrap().as_bytes())?;

        // Open the passages directory and verify the method fails.
        let config = TranscriptionConfig {
            passage_directory: "passages".into(),
            transcription_dependencies: vec![],
            skip_advanced_lessons: false,
        };
        let result = config.open_passage_directory(&temp_dir.path());
        assert!(result.is_err());
        Ok(())
    }

    /// Verifies that opening the passage directory fails if the directory does not exist.
    #[test]
    fn open_passage_directory_bad_directory() -> Result<()> {
        // Create the course directory but not the passages directory.
        let temp_dir = tempfile::tempdir()?;

        // Open the passages directory and verify the method fails.
        let config = TranscriptionConfig {
            passage_directory: "passages".into(),
            transcription_dependencies: vec![],
            skip_advanced_lessons: false,
        };
        let result = config.open_passage_directory(&temp_dir.path());
        assert!(result.is_err());
        Ok(())
    }

    /// Verifies cloning a transcription asset. Done so that the auto-generated trait implementation
    /// is included in the code coverage reports.
    #[test]
    fn asset_clone() {
        let asset = super::TranscriptionAsset::Track {
            short_id: "id".into(),
            track_name: "Track".into(),
            artist_name: "Artist".into(),
            album_name: "Album".into(),
            external_link: Some("https://example.com".into()),
        };
        let asset_clone = asset.clone();
        assert_eq!(asset, asset_clone);
    }

    /// Verifies cloning transcription passages. Done so that the auto-generated trait
    /// implementation is included in the code coverage reports.
    #[test]
    fn passages_clone() {
        let passages = TranscriptionPassages {
            asset: TranscriptionAsset::Track {
                short_id: "id".into(),
                track_name: "Track".into(),
                artist_name: "Artist".into(),
                album_name: "Album".into(),
                external_link: Some("https://example.com".into()),
            },
            intervals: HashMap::from([(1, ("0:00".into(), "0:01".into()))]),
        };
        let passages_clone = passages.clone();
        assert_eq!(passages, passages_clone);
    }

    /// Verifies that the instructions for the course are not replaced if they are already set.
    #[test]
    fn do_not_replace_existing_instructions() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        fs::create_dir(temp_dir.path().join("passages"))?;
        let course_generator = CourseGenerator::Transcription(TranscriptionConfig {
            transcription_dependencies: vec![],
            passage_directory: "passages".to_string(),
            skip_advanced_lessons: false,
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
}
