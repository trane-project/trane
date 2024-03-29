//! Contains utilities to generate music related courses.

use strum::Display;

pub mod circle_fifths;

/// Common metadata keys for all music courses and lessons.
#[derive(Display)]
#[strum(serialize_all = "snake_case")]
#[allow(missing_docs)]
pub enum MusicMetadata {
    Instrument,
    Key,
    MusicalConcept,
    MusicalSkill,
    ScaleType,
}
