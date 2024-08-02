//! Contains constants used by the transcription courses.

use indoc::indoc;
use lazy_static::lazy_static;
use ustr::Ustr;

/// The description of the singing lesson.
pub const SINGING_DESCRIPTION: &str = indoc! {"
    Repeatedly listen to the passage until you can audiate and sing its main elements. You should
    also experiment with singing different melodies over the passage and see what works.

    Refer to the lesson instructions for more details.
"};

/// The description of the advanced singing lesson.
pub const ADVANCED_SINGING_DESCRIPTION: &str = indoc! {"
    Repeatedly listen to the passage until you can audiate and sing it clearly in detail. Same as
    the singing lesson, but the passage should be audiated in more detail and precision, and
    transposed up or down a random number of semitones.

    Refer to the lesson instructions for more details.
"};

/// The description of the transcription lesson.
pub const TRANSCRIPTION_DESCRIPTION: &str = indoc! {"
    Using the stated instrument, play over the passage, using it as a basis for improvising. Playing
    back the exact passage is not required at this stage. Rather, this lesson is about learning to
    navigate the context implied by it.

    Refer to the lesson instructions for more details.
"};

/// The description of the advanced transcription lesson.
pub const ADVANCED_TRANSCRIPTION_DESCRIPTION: &str = indoc! {"
    Using the stated instrument, play over passage back, and use it as a basis for improvising.
    Same as the transcription exercise, but the passage should be played back in more detail and
    precision, and transposed up or down a random number of semitones.

    Refer to the lesson instructions for more details.
"};

/// The metadata key indicating this is a transcription course. Its value should be set to "true".
pub const COURSE_METADATA: &str = "transcription_course";

/// The metadata key indicating the type of the transcription lesson. Its value should be set to
/// "true".
pub const LESSON_METADATA: &str = "transcription_lesson";

/// The metadata key indicating the artists included in the transcription course.
pub const ARTIST_METADATA: &str = "transcription_artist";

/// The metadata key indicating the album included in the transcription course.
pub const ALBUM_METADATA: &str = "transcription_album";

/// The metadata key indicating the instrument of the transcription lesson.
pub const INSTRUMENT_METADATA: &str = "instrument";

lazy_static! {
    /// The instructions for the transcription course.
    pub static ref COURSE_INSTRUCTIONS: Ustr = Ustr::from(include_str!("course_instructions.md"));

    /// The instructions for the singing lessons.
    pub static ref SINGING_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        First listen to the musical passage until you can audiate it clearly in your head. Then sing
        over the passage. At this stage it's not required to be accurate as possible. Rather, learn
        to sing the main elements of the passage and experiment with different melodies over it.
        The goal is to learn to navigate the context implied by the passage.

        There's no need to play on your instrument or write anything down, but you are free to do so
        if you wish.
    "});

    /// The instructions for the advanced singing lessons.
    pub static ref ADVANCED_SINGING_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        Listen to the musical passage until you can audiate it and sing over it like you did in the
        singing lesson. In that lesson, the passage was used as a basis for improvisation. In this
        lesson, the passage should be sung with more detail and precision, and transposed up or down
        a random number of semitones. You should also use solfege syllables or numbers to sing the
        passage.

        There's no need to play on your instrument or write anything down, but you are free to do so
        if you wish.
    "});

    /// The instructions for the transcription lessons.
    pub static ref TRANSCRIPTION_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        With the basic context implied by the passage now internalized in your ear, try to play over
        it using your instrument. The goal at this point is not to accurately reproduce the passage,
        but rather about learning to navigate that context and use it as a basis for improvisation.
        You can focus on different elements or sections each time you practice.

        There's no need to write anything down, but you are free to do so if you wish.
    "});

    /// The instructions for the advanced transcription lessons.
    pub static ref ADVANCED_TRANSCRIPTION_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        At this stage, you can sing and play over the context implied by the passage, and sing it
        with more detail and precision in a variety of keys. It's at this point that you can engage
        in what is traditionally called transcription.

        Play over the passage using your instrument, and try to reproduce it in more detail than
        in the basic transcription lesson. You should also transpose the passage up or down a random
        number of semitones. You should still use the passage as a basis for improvisation, but the
        focus is much narrower than in the basic transcription lesson, and the actual music played
        in the passage take precedence over the context implied by it.

        There's no need to write anything down, but you are free to do so if you wish.
    "});
}
