//! Contains constants used by the transcription courses.

use indoc::indoc;
use lazy_static::lazy_static;
use ustr::Ustr;

// TODO: fill in values for these constants.

/// The description of the singing lesson.
pub const SINGING_DESCRIPTION: &str = indoc! {"
"};

/// The description of the advanced singing lesson.
pub const ADVANCED_SINGING_DESCRIPTION: &str = indoc! {"
"};

/// The description of the transcription lesson.
pub const TRANSCRIPTION_DESCRIPTION: &str = indoc! {"
"};

/// The description of the advanced transcription lesson.
pub const ADVANCED_TRANSCRIPTION_DESCRIPTION: &str = indoc! {"
"};

/// The metadata key indicating the lesson belongs to an transcription course.
pub const COURSE_METADATA: &str = "transcription";

/// The metadata key indicating the type of the transcription lesson.
pub const LESSON_METADATA: &str = "transcription_lesson";

/// The metadata key indicating the instrument of the improvisation lesson.
pub const INSTRUMENT_METADATA: &str = "instrument";

lazy_static! {
    /// The instructions for the transcription course.
    pub static ref COURSE_INSTRUCTIONS: Ustr = Ustr::from(include_str!("course_instructions.md"));

    /// The instructions for the singing lessons.
    pub static ref SINGING_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
    "});

    /// The instructions for the advanced singing lessons.
    pub static ref ADVANCED_SINGING_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
    "});

    /// The instructions for the transcription lessons.
    pub static ref TRANSCRIPTION_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
    "});

    /// The instructions for the advanced transcription lessons.
    pub static ref ADVANCED_TRANSCRIPTION_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
    "});
}
