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
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs::File, io::BufReader, path::Path};
use ustr::Ustr;

use crate::data::{
    BasicAsset, CourseManifest, ExerciseManifest, GenerateManifests, GeneratedCourse,
    LessonManifest, UserPreferences,
};
use constants::*;

/// An asset used for the transcription course generator.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TranscriptionAsset {
    /// The name of the track to use for transcription.
    pub track_name: String,

    /// The name of the artist(s) who performs the track.
    pub artist_name: String,

    /// The name of the album in which the track appears.
    pub album_name: String,

    /// A link to an external copy (e.g. youtube video) of the track.
    pub external_link: Option<String>,
}

/// Passages from a track that can be used for a transcription course.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TranscriptionPassage {
    /// The asset to transcribe.
    pub asset: TranscriptionAsset,

    /// The ranges `[start, end]` of the passages to transcribe. Stored as a tuple of strings.
    pub passage_ranges: Vec<(String, String)>,
}

impl TranscriptionPassage {
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
    /// [TranscriptionPassage] objects. The name of each JSON file (minus the extension) will be
    /// used to generate the ID for each exercise.
    ///
    /// The directory can be written relative to the root of the course or as an absolute path. The
    /// first option is recommended.
    pub passage_directory: String,
}

impl TranscriptionConfig {
    /// Reads all the files in the passage directory to generate the list of all the passages
    /// included in the course.
    fn open_passage_directory(
        &self,
        course_root: &Path,
    ) -> Result<HashMap<String, TranscriptionPassage>> {
        // Read all the files in the passage directory.
        let mut passages = HashMap::new();
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

            // Use the rest of the file name as the passage ID. Then, create the transcription
            // passage and add it to the result.
            let passage_id = file_name.strip_suffix(".json").unwrap_or_default();
            let passage = TranscriptionPassage::open(&path)?;
            passages.insert(passage_id.into(), passage);
        }
        Ok(passages)
    }

    /// Generates the lesson and exercise manifests for the singing lesson.
    fn generate_singing_lesson(
        &self,
        _course_manifest: &CourseManifest,
        _passages: &HashMap<String, TranscriptionPassage>,
    ) -> Vec<(LessonManifest, Vec<ExerciseManifest>)> {
        unimplemented!()
    }

    /// Generates the lesson and exercise manifests for the transcription lessons.
    fn generate_transcription_lessons(
        &self,
        _course_manifest: &CourseManifest,
        _preferences: &TranscriptionPreferences,
        _passages: &HashMap<String, TranscriptionPassage>,
    ) -> Vec<(LessonManifest, Vec<ExerciseManifest>)> {
        unimplemented!()
    }

    /// Generates the lesson and exercise manifests for the advanced singing lesson.
    fn generate_advanced_singing_lesson(
        &self,
        _course_manifest: &CourseManifest,
        _passages: &HashMap<String, TranscriptionPassage>,
    ) -> Vec<(LessonManifest, Vec<ExerciseManifest>)> {
        unimplemented!()
    }

    /// Generates the lesson and exercise manifests for the advanced transcription lessons.
    fn generate_advanced_transcription_lessons(
        &self,
        _course_manifest: &CourseManifest,
        _preferences: &TranscriptionPreferences,
        _passages: &HashMap<String, TranscriptionPassage>,
    ) -> Vec<(LessonManifest, Vec<ExerciseManifest>)> {
        unimplemented!()
    }

    /// Generates all the lesson and exercise manifests for the course.
    fn generate_lesson_manifests(
        &self,
        course_manifest: &CourseManifest,
        preferences: &TranscriptionPreferences,
        passages: HashMap<String, TranscriptionPassage>,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        Ok(vec![
            self.generate_singing_lesson(course_manifest, &passages),
            self.generate_advanced_singing_lesson(course_manifest, &passages),
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
