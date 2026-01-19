//! Defines a special course to teach transcription based on a set of musical passages.
//!  
//! The student is expected to listen to the passages to internalize the sounds, and then transcribe
//! the passages to their instruments and use them as a basis for improvisation. This course is
//! meant to replicate the process of listenting and imitation that is at the heart of the
//! transmitions of aural music traditions, such as Jazz.

pub mod constants;

use anyhow::{Context, Result, anyhow, bail};
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

//@<transcription-link
/// A link to an external resource for a transcription asset.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum TranscriptionLink {
    /// A link to a YouTube video.
    YouTube(String),
}

impl TranscriptionLink {
    /// Returns the URL of the link.
    #[must_use]
    pub fn url(&self) -> &str {
        match self {
            TranscriptionLink::YouTube(url) => url,
        }
    }
}
//>@transcription-link

//@<transcription-asset
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
        #[serde(default)]
        artist_name: Option<String>,

        /// The name of the album in which the track appears.
        #[serde(default)]
        album_name: Option<String>,

        /// The duration of the track.
        #[serde(default)]
        duration: Option<String>,

        /// A link to an external copy (e.g., YouTube link) of the track.
        #[serde(default)]
        external_link: Option<TranscriptionLink>,
    },
}
//>@transcription-asset

impl TranscriptionAsset {
    /// Returns the short ID of the asset, which will be used to generate the exercise IDs.
    #[must_use]
    pub fn short_id(&self) -> &str {
        match self {
            TranscriptionAsset::Track { short_id, .. } => short_id,
        }
    }
}

//@<transcription-passages
/// A collection of passages from a track that can be used for a transcription course.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TranscriptionPassages {
    /// The asset to transcribe.
    pub asset: TranscriptionAsset,

    /// The ranges `[start, end]` of the passages to transcribe. Stored as a map mapping a unique ID
    /// to the start and end of the passage. A map is used instead of a list because reordering the
    /// passages would change the resulting exercise IDs.
    ///
    /// If the map is empty, one passage is assumed to cover the entire asset and the ID for the
    /// exercises will not include a passage ID.
    #[serde(default)]
    pub intervals: HashMap<usize, (String, String)>,
}
//>@transcription-passages

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
            None => String::new(),
        };
        match &self.asset {
            TranscriptionAsset::Track {
                track_name,
                artist_name,
                album_name,
                external_link,
                duration,
                ..
            } => ExerciseAsset::TranscriptionAsset {
                content: formatdoc! {"
                    {}

                    The passage to transcribe is the following:
                        - Track name: {}
                        - Artist name: {}
                        - Album name: {}
                        - Track duration: {}
                        - External link: {}
                        - Passage interval: {} - {}
                    {}",
                    description, track_name, artist_name.as_deref().unwrap_or(""),
                    album_name.as_deref().unwrap_or(""), duration.as_deref().unwrap_or(""),
                    external_link.as_ref().map_or("", |l| l.url()), start, end,
                    instrument_instruction
                },
                external_link: external_link.clone(),
            },
        }
    }
}

impl TranscriptionPassages {
    fn open(path: &Path) -> Result<Self> {
        let file =
            File::open(path).context(format!("cannot open passage file {}", path.display()))?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader)
            .context(format!("cannot parse passage file {}", path.display()))
    }
}

//@<transcription-preferences
/// Settings for generating a new transcription course that are specific to a user.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct TranscriptionPreferences {
    /// The list of instruments the user wants to practice. Note that changing the instrument ID
    /// will change the IDs of the exercises and lose your progress, so it should be chosen
    /// carefully before you start practicing.
    #[serde(default)]
    pub instruments: Vec<Instrument>,

    /// A path used to download transcription assets to the local filesystem. If not specified,
    /// assets cannot be downloaded.
    #[serde(default)]
    pub download_path: Option<String>,

    /// An alias for the download path. This is useful when the download path is a long path or it's
    /// accessible from multiple locations (e.g., Windows and a terminal running on the WSL). Only
    /// used to present the download path to the user.
    #[serde(default)]
    pub download_path_alias: Option<String>,
}
//>@transcription-preferences

//@<transcription-config
/// The configuration used to generate a transcription course.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TranscriptionConfig {
    /// The dependencies on other transcription courses. Specifying these dependencies here instead
    /// of the [CourseManifest] allows Trane to generate more fine-grained dependencies.
    #[serde(default)]
    pub transcription_dependencies: Vec<Ustr>,

    /// The directory where the passages are stored as JSON files whose contents are serialized
    /// [TranscriptionPassages] objects.
    ///
    /// The directory can be written relative to the root of the course or as an absolute path. The
    /// first option is recommended. An empty value will safely default to not reading any files.
    #[serde(default)]
    pub passage_directory: String,

    /// A list of passages to include in the course in addition to the ones in the passage
    /// directory. Useful for adding passages directly in the course manifest.
    #[serde(default)]
    pub inlined_passages: Vec<TranscriptionPassages>,

    /// If true, the course will skip creating the singing lesson. This is useful when the course
    /// contains backing tracks that have no melodies, for example. Both the singing and the
    /// advanced singing lessons will be skipped. Because other transcription courses that depend on
    /// this lesson will use the singing lesson to create the dependency, the lesson will be
    /// created, but will be empty.
    #[serde(default)]
    pub skip_singing_lessons: bool,

    /// If true, the course will skip the advanced singing and transcription lessons. This is useful
    /// when there are copies of the same recording for every key, which makes the need for the
    /// advanced lessons obsolete.
    #[serde(default)]
    pub skip_advanced_lessons: bool,
}
//>@transcription-config

impl TranscriptionConfig {
    /// Returns the ID for a given exercise given the lesson ID and the exercise index.
    fn exercise_id(lesson_id: Ustr, asset_id: &str, passage_id: Option<usize>) -> Ustr {
        match passage_id {
            Some(passage_id) => Ustr::from(&format!("{lesson_id}::{asset_id}::{passage_id}")),
            None => Ustr::from(&format!("{lesson_id}::{asset_id}")),
        }
    }

    /// Returns the ID of the singing lesson for the given course.
    fn singing_lesson_id(course_id: Ustr) -> Ustr {
        Ustr::from(&format!("{course_id}::singing"))
    }

    /// Generates the singing exercises for the given passages.
    fn generate_singing_exercises(
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        passages: &TranscriptionPassages,
    ) -> Vec<ExerciseManifest> {
        // Generate the default exercise if no passages are provided.
        if passages.intervals.is_empty() {
            return vec![ExerciseManifest {
                id: Self::exercise_id(lesson_id, passages.asset.short_id(), None),
                lesson_id,
                course_id: course_manifest.id,
                name: format!("{} - Singing", course_manifest.name),
                description: None,
                exercise_type: ExerciseType::Procedural,
                exercise_asset: passages.generate_exercise_asset(
                    SINGING_DESCRIPTION,
                    "Start of passage",
                    "End of passage",
                    None,
                ),
            }];
        }

        // Generate an exercise for each passage.
        passages
            .intervals
            .iter()
            .map(|(passage_id, (start, end))| ExerciseManifest {
                id: Self::exercise_id(lesson_id, passages.asset.short_id(), Some(*passage_id)),
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
        skip_singing_lessons: bool,
    ) -> (LessonManifest, Vec<ExerciseManifest>) {
        // Generate the lesson manifest. The lesson depends on the singing lessons of all the other
        // transcription courses listed as dependencies.
        let dependencies = self
            .transcription_dependencies
            .iter()
            .map(|id| format!("{id}::singing").into())
            .collect();
        let lesson_manifest = LessonManifest {
            id: Self::singing_lesson_id(course_manifest.id),
            course_id: course_manifest.id,
            name: format!("{} - Singing", course_manifest.name),
            description: Some(SINGING_DESCRIPTION.to_string()),
            dependencies,
            superseded: vec![],
            metadata: Some(BTreeMap::from([
                (LESSON_METADATA.to_string(), vec!["singing".to_string()]),
                (INSTRUMENT_METADATA.to_string(), vec!["voice".to_string()]),
            ])),
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *SINGING_INSTRUCTIONS,
            }),
            lesson_material: None,
            default_exercise_type: None,
        };

        // If the course is configured to skip the singing lessons, return the lesson manifest with
        // no exercises. The lesson is still needed to correctly generate the dependencies for other
        // transcription courses that depend on this course.
        if skip_singing_lessons {
            return (lesson_manifest, Vec::new());
        }

        // Generate an exercise for each passage.
        let exercises = passages
            .iter()
            .flat_map(|passages| {
                Self::generate_singing_exercises(course_manifest, lesson_manifest.id, passages)
            })
            .collect::<Vec<_>>();
        (lesson_manifest, exercises)
    }

    /// Returns the ID of the singing lesson for the given course.
    fn advanced_singing_lesson_id(course_id: Ustr) -> Ustr {
        Ustr::from(&format!("{course_id}::advanced_singing"))
    }

    /// Generates the advanced singing exercises for the given passages.
    fn generate_advanced_singing_exercises(
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        passages: &TranscriptionPassages,
    ) -> Vec<ExerciseManifest> {
        // Generate the default exercise if no passages are provided.
        if passages.intervals.is_empty() {
            return vec![ExerciseManifest {
                id: Self::exercise_id(lesson_id, passages.asset.short_id(), None),
                lesson_id,
                course_id: course_manifest.id,
                name: format!("{} - Advanced Singing", course_manifest.name),
                description: None,
                exercise_type: ExerciseType::Procedural,
                exercise_asset: passages.generate_exercise_asset(
                    ADVANCED_SINGING_DESCRIPTION,
                    "Start of passage",
                    "End of passage",
                    None,
                ),
            }];
        }

        // Generate an exercise for each passage.
        passages
            .intervals
            .iter()
            .map(|(passage_id, (start, end))| ExerciseManifest {
                id: Self::exercise_id(lesson_id, passages.asset.short_id(), Some(*passage_id)),
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
        course_manifest: &CourseManifest,
        passages: &[TranscriptionPassages],
        skip_singing_lessons: bool,
    ) -> (LessonManifest, Vec<ExerciseManifest>) {
        // Generate the lesson manifest. The lesson depends on the singing lesson.
        let lesson_manifest = LessonManifest {
            id: Self::advanced_singing_lesson_id(course_manifest.id),
            course_id: course_manifest.id,
            name: format!("{} - Advanced Singing", course_manifest.name),
            description: Some(ADVANCED_SINGING_DESCRIPTION.to_string()),
            dependencies: vec![Self::singing_lesson_id(course_manifest.id)],
            superseded: vec![Self::singing_lesson_id(course_manifest.id)],
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
            default_exercise_type: None,
        };

        // If the course is configured to skip the singing lessons, return the lesson manifest with
        // no exercises. The lesson is still needed to correctly generate the dependencies for other
        // transcription courses that depend on this course.
        if skip_singing_lessons {
            return (lesson_manifest, Vec::new());
        }

        // Generate an exercise for each passage.
        let exercises = passages
            .iter()
            .flat_map(|passages| {
                Self::generate_advanced_singing_exercises(
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
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        passages: &TranscriptionPassages,
        instrument: &Instrument,
    ) -> Vec<ExerciseManifest> {
        // Generate the default exercise if no passages are provided.
        if passages.intervals.is_empty() {
            return vec![ExerciseManifest {
                id: Self::exercise_id(lesson_id, passages.asset.short_id(), None),
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
                    "Start of passage",
                    "End of passage",
                    Some(instrument),
                ),
            }];
        }

        // Generate an exercise for each passage.
        passages
            .intervals
            .iter()
            .map(|(passage_id, (start, end))| ExerciseManifest {
                id: Self::exercise_id(lesson_id, passages.asset.short_id(), Some(*passage_id)),
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
        // Generate the lesson manifest. The lesson depends on the singing lesson and the
        // transcription lessons of the courses listed in the transcription dependencies.
        let mut dependencies: Vec<Ustr> = self
            .transcription_dependencies
            .iter()
            .map(|id| format!("{id}::transcription::{}", instrument.id).into())
            .collect();
        dependencies.push(Self::singing_lesson_id(course_manifest.id));
        let lesson_manifest = LessonManifest {
            id: Self::transcription_lesson_id(course_manifest.id, instrument),
            course_id: course_manifest.id,
            name: format!(
                "{} - Transcription - {}",
                course_manifest.name, instrument.name
            ),
            description: Some(TRANSCRIPTION_DESCRIPTION.to_string()),
            dependencies,
            superseded: vec![],
            metadata: Some(BTreeMap::from([
                (
                    LESSON_METADATA.to_string(),
                    vec!["transcription".to_string()],
                ),
                (INSTRUMENT_METADATA.to_string(), vec![instrument.id.clone()]),
            ])),
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *TRANSCRIPTION_INSTRUCTIONS,
            }),
            lesson_material: None,
            default_exercise_type: None,
        };

        // Generate an exercise for each passage.
        let exercises = passages
            .iter()
            .flat_map(|passages| {
                Self::generate_transcription_exercises(
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
        course_manifest: &CourseManifest,
        lesson_id: Ustr,
        passages: &TranscriptionPassages,
        insturment: &Instrument,
    ) -> Vec<ExerciseManifest> {
        // Generate the default exercise if no passages are provided.
        if passages.intervals.is_empty() {
            return vec![ExerciseManifest {
                id: Self::exercise_id(lesson_id, passages.asset.short_id(), None),
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
                    "Start of passage",
                    "End of passage",
                    Some(insturment),
                ),
            }];
        }

        // Generate an exercise for each passage.
        passages
            .intervals
            .iter()
            .map(|(passage_id, (start, end))| ExerciseManifest {
                id: Self::exercise_id(lesson_id, passages.asset.short_id(), Some(*passage_id)),
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
        course_manifest: &CourseManifest,
        passages: &[TranscriptionPassages],
        instrument: &Instrument,
    ) -> (LessonManifest, Vec<ExerciseManifest>) {
        // Generate the lesson manifest. The lesson depends on the advanced singing lesson and the
        // transcription lesson for the instrument.
        let lesson_manifest = LessonManifest {
            id: Self::advanced_transcription_lesson_id(course_manifest.id, instrument),
            course_id: course_manifest.id,
            name: format!(
                "{} - Advanced Transcription - {}",
                course_manifest.name, instrument.name
            ),
            description: Some(ADVANCED_TRANSCRIPTION_DESCRIPTION.to_string()),
            dependencies: vec![
                Self::transcription_lesson_id(course_manifest.id, instrument),
                Self::advanced_singing_lesson_id(course_manifest.id),
            ],
            superseded: vec![Self::transcription_lesson_id(
                course_manifest.id,
                instrument,
            )],
            metadata: Some(BTreeMap::from([
                (
                    LESSON_METADATA.to_string(),
                    vec!["advanced_transcription".to_string()],
                ),
                (INSTRUMENT_METADATA.to_string(), vec![instrument.id.clone()]),
            ])),
            lesson_instructions: Some(BasicAsset::InlinedUniqueAsset {
                content: *ADVANCED_TRANSCRIPTION_INSTRUCTIONS,
            }),
            lesson_material: None,
            default_exercise_type: None,
        };

        // Generate exercises for each passage.
        let exercises = passages
            .iter()
            .flat_map(|passages| {
                Self::generate_advanced_transcription_exercises(
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
        course_manifest: &CourseManifest,
        preferences: &TranscriptionPreferences,
        passages: &[TranscriptionPassages],
    ) -> Vec<(LessonManifest, Vec<ExerciseManifest>)> {
        preferences
            .instruments
            .iter()
            .map(|instrument| {
                Self::generate_advanced_transcription_lesson(course_manifest, passages, instrument)
            })
            .collect()
    }

    /// Reads all the files in the passage directory to generate the list of all the passages
    /// included in the course.
    fn open_passage_directory(&self, course_root: &Path) -> Result<Vec<TranscriptionPassages>> {
        // Do not attempt to open the passage directory if the value is empty.
        if self.passage_directory.is_empty() {
            return Ok(Vec::new());
        }

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
                .ok_or(anyhow!("Failed to get the file name"))?
                .to_string();

            // Ignore any non-JSON files.
            if !Path::new(&file_name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
            {
                continue;
            }

            // Open the file and parse it as a `TranscriptionPassages` object. Check for duplicate
            // short IDs.
            let passage = TranscriptionPassages::open(&path)?;
            let short_id = passage.asset.short_id();
            if seen_ids.contains(short_id) {
                bail!("Duplicate passage ID: {short_id}");
            }
            seen_ids.insert(short_id.to_string());
            passages.push(passage);
        }
        Ok(passages)
    }

    /// Generates all the lesson and exercise manifests for the course.
    fn generate_lesson_manifests(
        &self,
        course_manifest: &CourseManifest,
        preferences: &TranscriptionPreferences,
        passages: &[TranscriptionPassages],
    ) -> Vec<(LessonManifest, Vec<ExerciseManifest>)> {
        let mut skip_singing_lessons = false;
        let mut skip_advanced_lessons = false;
        if let Some(CourseGenerator::Transcription(config)) = &course_manifest.generator_config {
            skip_singing_lessons = config.skip_singing_lessons;
            skip_advanced_lessons = config.skip_advanced_lessons;
        }

        if skip_advanced_lessons {
            vec![
                vec![self.generate_singing_lesson(course_manifest, passages, skip_singing_lessons)],
                self.generate_transcription_lessons(course_manifest, preferences, passages),
            ]
            .into_iter()
            .flatten()
            .collect()
        } else {
            vec![
                vec![self.generate_singing_lesson(course_manifest, passages, skip_singing_lessons)],
                vec![Self::generate_advanced_singing_lesson(
                    course_manifest,
                    passages,
                    skip_singing_lessons,
                )],
                self.generate_transcription_lessons(course_manifest, preferences, passages),
                Self::generate_advanced_transcription_lessons(
                    course_manifest,
                    preferences,
                    passages,
                ),
            ]
            .into_iter()
            .flatten()
            .collect()
        }
    }

    /// Takes the current course metadata as input and returns updated metadata with information
    /// about the transcription course.
    fn generate_course_metadata(
        metadata: Option<&BTreeMap<String, Vec<String>>>,
        passages: &[TranscriptionPassages],
    ) -> BTreeMap<String, Vec<String>> {
        // Insert metadata to indicate this is a transcription course.
        let mut metadata = metadata.cloned().unwrap_or_default();
        metadata.insert(COURSE_METADATA.to_string(), vec!["true".to_string()]);

        // Insert metadata to add all the artists from the passages.
        for passages in passages {
            if let TranscriptionAsset::Track {
                artist_name: Some(artist_name),
                ..
            } = &passages.asset
            {
                metadata
                    .entry(ARTIST_METADATA.to_string())
                    .or_default()
                    .push(artist_name.clone());
            }
        }

        // Do the same with all the albums.
        for passages in passages {
            if let TranscriptionAsset::Track {
                album_name: Some(album_name),
                ..
            } = &passages.asset
            {
                metadata
                    .entry(ALBUM_METADATA.to_string())
                    .or_default()
                    .push(album_name.clone());
            }
        }

        metadata
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

        // Read the passages from the passage directory and add the inlined passages. Then generate
        // the lesson and exercise manifests.
        let mut passages = self.open_passage_directory(course_root)?;
        passages.extend(self.inlined_passages.clone());
        let lessons = self.generate_lesson_manifests(course_manifest, preferences, &passages);

        // Update the course's metadata and instructions.
        let metadata = Self::generate_course_metadata(course_manifest.metadata.as_ref(), &passages);
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
#[cfg_attr(coverage, coverage(off))]
mod test {
    use anyhow::Result;
    use indoc::indoc;
    use std::{fs, io::Write};

    use crate::data::CourseGenerator;

    use super::*;

    /// Verifies generating IDs for the exercises in the course.
    #[test]
    fn exercise_id() {
        // Generate the ID for an exercise with a passage ID.
        let lesson_id = Ustr::from("lesson_id");
        let asset_id = "asset_id";
        let passage_id = 2;
        assert_eq!(
            TranscriptionConfig::exercise_id(lesson_id, asset_id, Some(passage_id)),
            Ustr::from("lesson_id::asset_id::2")
        );

        // Generate the ID for an exercise with the default passage.
        assert_eq!(
            TranscriptionConfig::exercise_id(lesson_id, asset_id, None),
            Ustr::from("lesson_id::asset_id")
        );
    }

    /// Verifies generating the lesson ID for the singing lesson.
    #[test]
    fn singing_lesson_id() {
        let course_id = Ustr::from("course_id");
        assert_eq!(
            TranscriptionConfig::singing_lesson_id(course_id),
            Ustr::from("course_id::singing")
        );
    }

    /// Verifies generating the lesson ID for the advanced singing lesson.
    #[test]
    fn advanced_singing_lesson_id() {
        let course_id = Ustr::from("course_id");
        assert_eq!(
            TranscriptionConfig::advanced_singing_lesson_id(course_id),
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
            TranscriptionConfig::transcription_lesson_id(course_id, &instrument),
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
            TranscriptionConfig::advanced_transcription_lesson_id(course_id, &instrument),
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
                artist_name: Some("Artist".into()),
                album_name: Some("Album".into()),
                duration: Some("1:30".into()),
                external_link: Some(TranscriptionLink::YouTube("https://example.com".into())),
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
        let expected_asset = ExerciseAsset::TranscriptionAsset {
            content: indoc! {"
                My description

                The passage to transcribe is the following:
                    - Track name: Track
                    - Artist name: Artist
                    - Album name: Album
                    - Track duration: 1:30
                    - External link: https://example.com
                    - Passage interval: 0:00 - 0:01

                Transcribe the passage using the instrument: Piano.
            "}
            .into(),
            external_link: Some(TranscriptionLink::YouTube("https://example.com".into())),
        };
        assert_eq!(exercise_asset, expected_asset);

        // Generate the asset when an instrument is not specified.
        let exercise_asset =
            passages.generate_exercise_asset("My description", "0:00", "0:01", None);
        let expected_asset = ExerciseAsset::TranscriptionAsset {
            content: indoc! {"
                My description

                The passage to transcribe is the following:
                    - Track name: Track
                    - Artist name: Artist
                    - Album name: Album
                    - Track duration: 1:30
                    - External link: https://example.com
                    - Passage interval: 0:00 - 0:01
            "}
            .into(),
            external_link: Some(TranscriptionLink::YouTube("https://example.com".into())),
        };
        assert_eq!(exercise_asset, expected_asset);
    }

    /// Verifies creating the course based on the passages in the passage directory.
    #[test]
    fn open_passage_directory() -> Result<()> {
        // Create the `passages` directory.
        let temp_dir = tempfile::tempdir()?;
        let passages_dir = temp_dir.path().join("passages");
        fs::create_dir(&passages_dir)?;

        // Write some test passages to the directory.
        let passages1 = TranscriptionPassages {
            asset: TranscriptionAsset::Track {
                short_id: "track1".into(),
                track_name: "Track 1".into(),
                artist_name: Some("Artist 1".into()),
                album_name: Some("Album 1".into()),
                duration: Some("1:30".into()),
                external_link: None,
            },
            intervals: HashMap::from([(1, ("0:00".into(), "0:01".into()))]),
        };
        let passages2 = TranscriptionPassages {
            asset: TranscriptionAsset::Track {
                short_id: "track2".into(),
                track_name: "Track 2".into(),
                artist_name: Some("Artist 2".into()),
                album_name: Some("Album 2".into()),
                duration: Some("1:30".into()),
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
            inlined_passages: vec![],
            transcription_dependencies: vec![],
            skip_singing_lessons: false,
            skip_advanced_lessons: false,
        };
        let passages = config.open_passage_directory(temp_dir.path())?;
        assert_eq!(2, passages.len());

        Ok(())
    }

    /// Verifies creating the course based on an empty passage directory.
    #[test]
    fn open_passage_directory_empty() -> Result<()> {
        // Create the `passages` directory.
        let temp_dir = tempfile::tempdir()?;

        // Open the empty passages directory and verify there are no passages.
        let config = TranscriptionConfig {
            passage_directory: String::new(),
            inlined_passages: vec![],
            transcription_dependencies: vec![],
            skip_singing_lessons: false,
            skip_advanced_lessons: false,
        };
        let passages = config.open_passage_directory(temp_dir.path())?;
        assert!(passages.is_empty());

        Ok(())
    }

    /// Verifies that opening the passage directory fails if there are passages with duplicate IDs.
    #[test]
    fn open_passage_directory_duplicate() -> Result<()> {
        // Create the `passages` directory.
        let temp_dir = tempfile::tempdir()?;
        let passages_dir = temp_dir.path().join("passages");
        fs::create_dir(&passages_dir)?;

        // Write some test passages to the directory. The passages have duplicate IDs.
        let passages1 = TranscriptionPassages {
            asset: TranscriptionAsset::Track {
                short_id: "track1".into(),
                track_name: "Track 1".into(),
                artist_name: Some("Artist 1".into()),
                album_name: Some("Album 1".into()),
                duration: Some("1:30".into()),
                external_link: None,
            },
            intervals: HashMap::from([(1, ("0:00".into(), "0:01".into()))]),
        };
        let passages2 = TranscriptionPassages {
            asset: TranscriptionAsset::Track {
                short_id: "track1".into(),
                track_name: "Track 2".into(),
                artist_name: Some("Artist 2".into()),
                album_name: Some("Album 2".into()),
                duration: Some("1:30".into()),
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
            inlined_passages: vec![],
            transcription_dependencies: vec![],
            skip_singing_lessons: false,
            skip_advanced_lessons: false,
        };
        let result = config.open_passage_directory(temp_dir.path());
        assert!(result.is_err());
        Ok(())
    }

    /// Verifies that opening the passage directory fails if the directory does not exist.
    #[test]
    fn open_passage_directory_bad_directory() -> Result<()> {
        // Create the course directory but not the `passages` directory.
        let temp_dir = tempfile::tempdir()?;

        // Open the passages directory and verify the method fails.
        let config = TranscriptionConfig {
            passage_directory: "passages".into(),
            inlined_passages: vec![],
            transcription_dependencies: vec![],
            skip_singing_lessons: false,
            skip_advanced_lessons: false,
        };
        let result = config.open_passage_directory(temp_dir.path());
        assert!(result.is_err());
        Ok(())
    }

    /// Verifies that the instructions for the course are not replaced if they are already set.
    #[test]
    fn do_not_replace_existing_instructions() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        fs::create_dir(temp_dir.path().join("passages"))?;
        let course_generator = CourseGenerator::Transcription(TranscriptionConfig {
            transcription_dependencies: vec![],
            passage_directory: "passages".to_string(),
            inlined_passages: vec![],
            skip_singing_lessons: false,
            skip_advanced_lessons: false,
        });

        let course_manifest = CourseManifest {
            id: Ustr::from("testID"),
            name: "Test".to_string(),
            description: None,
            dependencies: vec![],
            superseded: vec![],
            authors: None,
            metadata: None,
            course_instructions: Some(BasicAsset::InlinedAsset {
                content: "test".to_string(),
            }),
            course_material: None,
            default_exercise_type: None,
            generator_config: Some(course_generator.clone()),
        };
        let preferences = UserPreferences::default();
        let generated_course =
            course_generator.generate_manifests(temp_dir.path(), &course_manifest, &preferences)?;
        assert!(generated_course.updated_instructions.is_none());
        Ok(())
    }

    /// Verifies that the artists and albums are added to the metadata.
    #[test]
    fn add_artist_and_album_metadata() -> Result<()> {
        // Create a course with a couple of tracks with artist and album names.
        let temp_dir = tempfile::tempdir()?;
        fs::create_dir(temp_dir.path().join("passages"))?;
        let course_generator = CourseGenerator::Transcription(TranscriptionConfig {
            transcription_dependencies: vec![],
            passage_directory: "passages".to_string(),
            inlined_passages: vec![
                TranscriptionPassages {
                    asset: TranscriptionAsset::Track {
                        short_id: "track1".into(),
                        track_name: "Track 1".into(),
                        artist_name: Some("Artist 1".into()),
                        album_name: Some("Album 1".into()),
                        duration: None,
                        external_link: None,
                    },
                    intervals: HashMap::new(),
                },
                TranscriptionPassages {
                    asset: TranscriptionAsset::Track {
                        short_id: "track2".into(),
                        track_name: "Track 2".into(),
                        artist_name: Some("Artist 2".into()),
                        album_name: Some("Album 2".into()),
                        duration: None,
                        external_link: None,
                    },
                    intervals: HashMap::new(),
                },
            ],
            skip_singing_lessons: false,
            skip_advanced_lessons: false,
        });
        let course_manifest = CourseManifest {
            id: Ustr::from("testID"),
            name: "Test".to_string(),
            description: None,
            dependencies: vec![],
            superseded: vec![],
            authors: None,
            metadata: None,
            course_instructions: Some(BasicAsset::InlinedAsset {
                content: "test".to_string(),
            }),
            course_material: None,
            default_exercise_type: None,
            generator_config: Some(course_generator.clone()),
        };

        // Create the course and verifies the metadata is correct.
        let preferences = UserPreferences::default();
        let generated_course =
            course_generator.generate_manifests(temp_dir.path(), &course_manifest, &preferences)?;
        assert_eq!(
            generated_course
                .updated_metadata
                .as_ref()
                .unwrap()
                .get(ARTIST_METADATA),
            Some(&vec!["Artist 1".to_string(), "Artist 2".to_string()])
        );
        assert_eq!(
            generated_course
                .updated_metadata
                .as_ref()
                .unwrap()
                .get(ALBUM_METADATA),
            Some(&vec!["Album 1".to_string(), "Album 2".to_string()])
        );
        Ok(())
    }

    /// Verifies cloning a transcription asset. Done so that the auto-generated trait implementation
    /// is included in the code coverage reports.
    #[test]
    fn asset_clone() {
        let asset = super::TranscriptionAsset::Track {
            short_id: "id".into(),
            track_name: "Track".into(),
            artist_name: Some("Artist".into()),
            album_name: Some("Album".into()),
            duration: Some("1:30".into()),
            external_link: Some(TranscriptionLink::YouTube("https://example.com".into())),
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
                artist_name: Some("Artist".into()),
                album_name: Some("Album".into()),
                duration: Some("1:30".into()),
                external_link: Some(TranscriptionLink::YouTube("https://example.com".into())),
            },
            intervals: HashMap::from([(1, ("0:00".into(), "0:01".into()))]),
        };
        let passages_clone = passages.clone();
        assert_eq!(passages, passages_clone);
    }
}
