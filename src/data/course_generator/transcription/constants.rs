//! Contains constants used by the transcription courses.

use lazy_static::lazy_static;
use ustr::Ustr;

/// The metadata key indicating the lesson belongs to an improvisation course.
pub const COURSE_METADATA: &str = "improvisation";

/// The metadata key indicating the type of the improvisation lesson.
pub const LESSON_METADATA: &str = "improvisation_lesson";

/// The metadata key indicating the key of the improvisation lesson.
pub const KEY_METADATA: &str = "key";

/// The metadata key indicating the instrument of the improvisation lesson.
pub const INSTRUMENT_METADATA: &str = "instrument";

lazy_static! {
    /// The instructions for the transcription course.
    pub static ref COURSE_INSTRUCTIONS: Ustr = Ustr::from(include_str!("course_instructions.md"));
}
