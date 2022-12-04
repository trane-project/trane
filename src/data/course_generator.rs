use anyhow::Result;
use serde::{Deserialize, Serialize};
use ustr::Ustr;

use crate::data::{ExerciseManifest, GenerateManifests, LessonManifest};

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

/// The mode of the musical passages in a Trane improvisation course. This mode will be used to
/// generate lessons to practice each exercise in all keys, starting from the key with zero flats or
/// sharps, and introducing the keys with additional flats/sharps one at a time.
///
/// Major and Minor correspond to the Ionian and Aeolian modes, respectively.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Mode {
    Ionian,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Aeolian,
    Locrian,
}

/// The configuration for creating a new improvisation course.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TraneImprovisation {
    /// The dependencies on other Trane improvisation courses. Specifying these dependencies here
    /// instead of the [CourseManifest](data::CourseManifest) allows Trane to generate more
    /// fine-grained dependencies.
    pub improvisation_dependencies: Vec<Ustr>,

    /// The passages to be used in the course.
    pub passages: Vec<ImprovisationPassage>,

    /// The mode of all the passages in the course. This value is optional and if not provided, the
    /// lesson to practice each exercise in a different key will not be generated. For most material
    /// that has a tonal center, this value should be provided.
    pub mode: Option<Mode>,

    /// If true, the course contains passages that concern only rhythm. Lessons to learn the melody
    /// and harmony of the passages will not be generated. The mode of the course will be ignored.
    pub rhythm_only: bool,
}

impl TraneImprovisation {
    fn generate_rhtyhm_only_manifests(
        &self,
        course_id: Ustr,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        unimplemented!()
    }

    fn generate_all_manifests(
        &self,
        course_id: Ustr,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        unimplemented!()
    }
}

impl GenerateManifests for TraneImprovisation {
    fn generate_manifests(
        &self,
        course_id: Ustr,
    ) -> Result<Vec<(LessonManifest, Vec<ExerciseManifest>)>> {
        if self.rhythm_only {
            self.generate_rhtyhm_only_manifests(course_id)
        } else {
            self.generate_all_manifests(course_id)
        }
    }
}
