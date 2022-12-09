//! Contains constants used by the Trane improvisation courses.

use indoc::indoc;
use lazy_static::lazy_static;
use ustr::Ustr;

/// The description of the singing lesson.
pub const SINGING_DESCRIPTION: &str = indoc! {"
    Listen to, audiate, and sing the passage.
    Refer to the lesson instructions for more details.
"};

/// The description of the rhythm lesson.
pub const RHYTHM_DESCRIPTION: &str = indoc! {"
    Sight-sing or use your instrument to improvise using the rhythm of the passage.
    Refer to the lesson instructions for more details.
"};

/// The description of the melody lesson.
pub const MELODY_DESCRIPTION: &str = indoc! {"
    Sight-sing or use your instrument to improvise using the melody of the passage.
    Refer to the lesson instructions for more details.
"};

/// The description of the basic harmony lesson.
pub const BASIC_HARMONY_DESCRIPTION: &str = indoc! {"
    Sight-sing or use your instrument to improvise using the basic harmony of the passage.
    Refer to the lesson instructions for more details.
"};

/// The description of the advanced harmony lesson.
pub const ADVANCED_HARMONY_DESCRIPTION: &str = indoc! {"
    Sight-sing or use your instrument to improvise using all the harmony of the passage.
"};

/// The description of the mastery lesson.
pub const MASTERY_DESCRIPTION: &str = indoc! {"
    Sight-sing or use your instrument to improvise using all the melodic, rhythmic, and
    harmonic elements of the passage.
    Refer to the lesson instructions for more details.
"};

/// The metadata key indicating the lesson belongs to a Trane improvisation course.
pub const COURSE_METADATA: &str = "trane_improvisation";

/// The metadata key indicating the type of the improvisation lesson.
pub const LESSON_METADATA: &str = "trane_improvisation_lesson";

/// The metadata key indicating the key of the improvisation lesson.
pub const KEY_METADATA: &str = "key";

/// The metadata key indicating the instrument of the improvisation lesson.
pub const INSTRUMENT_METADATA: &str = "instrument";

lazy_static! {
    /// The instructions for the singing lessons.
    pub static ref SINGING_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        This step involves listening to the musical passage, audiating it in your head,
        and then singing it. You should sing the passage as accurately as possible, but
        it's not required that you use solfege syllables or numbers to identify the notes.

        This step does not contain specific lessons for each key. You should choose a
        random key each time you perform this exercise. No improvisation is required
        at this point.
    "});

    /// The instructions for the rhythm lessons.
    pub static ref RHYTHM_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        This step involves sight-singing or the stated instrument to improvise using the
        rhythm of the passage.

        When sight-singing, you can use a simple rhytm syllable system or a more complex
        one (e.g the Kodaly system).
    "});

    /// The instructions for the melody lessons.
    pub static ref MELODY_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        This step involves sight-singing or the stated instrument to improvise using the
        melody of the passage.

        Use your prefered sight-singing system (refer to the course instructions). When
        using your instrument, you should sing along.
    "});

    /// The instructions for the basic harmony lessons.
    pub static ref BASIC_HARMONY_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        This step involves sight-singing or the stated instrument to improvise using the
        basic harmony of the passage. The basic harmony consists of the main chord tones
        of each chord in the progression.

        Use your prefered sight-singing system (refer to the course instructions). When
        using your instrument, you should sing along.
    "});

    /// The instructions for the basic harmony lessons.
    pub static ref ADVANCED_HARMONY_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        This step involves sight-singing or the stated instrument to improvise using all
        the harmony of the passage, including tones in the scale or mode that are not the
        chord tones as well as chromatic notes. 

        Use your prefered sight-singing system (refer to the course instructions). When
        using your instrument, you should sing along.
    "});

    /// The instructions for the rhythm lessons.
    pub static ref MASTERY_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        Using all you have learned in the previous lessons, select a key at random and
        improvise using all the melodic, rhythmic, and harmonic elements of the passage.
    "});
}