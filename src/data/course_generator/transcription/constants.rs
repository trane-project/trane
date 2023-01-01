//! Contains constants used by the transcription courses.

use indoc::indoc;
use lazy_static::lazy_static;
use ustr::Ustr;

/// The description of the singing lesson.
pub const SINGING_DESCRIPTION: &str = indoc! {"
    Repeatedly listen to the passage until you can audiate and sing it clearly.
    Refer to the lesson instructions for more details.
"};

/// The description of the advanced singing lesson.
pub const ADVANCED_SINGING_DESCRIPTION: &str = indoc! {"
    Repeatedly listen to the passage until you can audiate and sing it clearly.

    Same as the singing exercise but transpose the passage up or down a random
    number of semitones.
"};

/// The description of the transcription lesson.
pub const TRANSCRIPTION_DESCRIPTION: &str = indoc! {"
    Using the stated instrument, play the passage back on your instrument and use
    it as a basis for improvising. Focus on different elements of the passage each
    time you do this exercise.
"};

/// The description of the advanced transcription lesson.
pub const ADVANCED_TRANSCRIPTION_DESCRIPTION: &str = indoc! {"
    Using the stated instrument, play the passage back on your instrument and use
    it as a basis for improvising. Focus on different elements of the passage each
    time you do this exercise.
    
    Same as the transcription exercise but transpose the passage up or down a random
    number of semitones.
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
        First listen to the musical passage until you can audiate it clearly in your head. Then sing
        the passage as accurately as possible. It's not required that you use solfege syllables
        or numbers, but you can do so if you recognize the key and tones of the passage.

        Listen to the passage as-is, without transposing it up or down. There's no need to play on
        your instrument or write anything down, but you are free to do so if you wish.
    "});

    /// The instructions for the advanced singing lessons.
    pub static ref ADVANCED_SINGING_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        Transpose the passage a random number of semitones. Then listen to the musical passage until
        you can audiate it clearly in your head. Then sing the passage as accurately as possible.
        It's not required that you use solfege syllables or numbers, but you can do so if you
        recognize the key and tones of the passage.
        
        There's no need to play on your instrument or write anything down, but you are free to do so
        if you wish.
    "});

    /// The instructions for the transcription lessons.
    pub static ref TRANSCRIPTION_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        With the passage now internalized in your ear, try to play it back on your instrument. This
        step is not about accurately reproducing the passage, but rather about extracting some
        elements from it and using them as a basis for improvisation. For example, you might become
        interested in playing with the drumming in one session and with the harmonies played by a
        piano in another.
    "});

    /// The instructions for the advanced transcription lessons.
    pub static ref ADVANCED_TRANSCRIPTION_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        With the transponsed passage now internalized in your ear, transpose the passage a random
        number of semitones and try to play it back on your instrument. This step is not about
        accurately reproducing the passage, but rather about extracting some elements from it and
        using them as a basis for improvisation
    "});
}
