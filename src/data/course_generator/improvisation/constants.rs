//! Contains constants used by the improvisation courses.

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
    Sing or use your instrument to improvise using the rhythm of the passage.
    Refer to the lesson instructions for more details.
"};

/// The description of the melody lesson.
pub const MELODY_DESCRIPTION: &str = indoc! {"
    Sing or use your instrument to improvise using the melody of the passage.
    Refer to the lesson instructions for more details.
"};

/// The description of the basic harmony lesson.
pub const BASIC_HARMONY_DESCRIPTION: &str = indoc! {"
    Sing or use your instrument to improvise using the basic harmony of the passage.
    Refer to the lesson instructions for more details.
"};

/// The description of the advanced harmony lesson.
pub const ADVANCED_HARMONY_DESCRIPTION: &str = indoc! {"
    Sing or use your instrument to improvise using all the harmony of the passage.
    Refer to the lesson instructions for more details.
"};

/// The description of the mastery lesson.
pub const MASTERY_DESCRIPTION: &str = indoc! {"
    Sing or use your instrument to improvise using all the melodic, rhythmic, and
    harmonic elements of the passage.
    Refer to the lesson instructions for more details.
"};

/// The metadata key indicating the lesson belongs to an improvisation course.
pub const COURSE_METADATA: &str = "improvisation";

/// The metadata key indicating the type of the improvisation lesson.
pub const LESSON_METADATA: &str = "improvisation_lesson";

/// The metadata key indicating the key of the improvisation lesson.
pub const KEY_METADATA: &str = "key";

/// The metadata key indicating the instrument of the improvisation lesson.
pub const INSTRUMENT_METADATA: &str = "instrument";

lazy_static! {
    pub static ref COURSE_INSTRUCTIONS: Ustr = Ustr::from(include_str!("course_instructions.md"));

    /// The instructions for the singing lessons.
    pub static ref SINGING_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        First listen to the musical passage until you can audiate it clearly in your head. Then sing
        the passage as accurately as possible, but it's not required that you use solfege syllables
        or numbers at this stage.

        This step does not contain specific lessons for each key. You should choose a random key
        each time you perform this exercise. No improvisation is required at this point, although
        you are welcome to do so if it comes naturally to you.
    "});

    /// The instructions for the rhythm lessons.
    pub static ref RHYTHM_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        Sing or play your instrument as stated by the lesson name to improvise using the rhythm
        of the passage. If using a pitched instrument, you can improvise using different melodies
        that match the rhythm of the passage.

        When singing, you can use a simple rhytm syllable system or a more complex one
        (e.g the Kodaly system).
    "});

    /// The instructions for the melody lessons.
    pub static ref MELODY_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        Sing or play your instrument as stated by the lesson name to improvise using the melody
        of the passage. This level involves practicing on all keys. Use the key stated in the
        lesson name.

        When singing, use your prefered sight-singing system. When using your instrument,
        you should sing along to reinforce the colors of the different pitches.
    "});

    /// The instructions for the basic harmony lessons.
    pub static ref BASIC_HARMONY_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        Sing or play your instrument as stated by the lesson name to improvise using the basic
        harmony of the passage. The basic harmony consists of the tones in the chords of the
        harmonic progression. This level involves practicing on all keys. Use the key stated in the
        lesson name.

        When singing, use your prefered sight-singing system. When using your instrument,
        you should sing along to reinforce the colors of the different pitches.
    "});

    /// The instructions for the advanced harmony lessons.
    pub static ref ADVANCED_HARMONY_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        Sing or play your instrument as stated by the lesson name to improvise using all the harmony
        of the passage, including tones in the scale or mode that are not the chord tones as well as
        chromatic notes. This level involves practicing on all keys. Use the key stated in the
        lesson name.

        When singing, use your prefered sight-singing system. When using your instrument,
        you should sing along to reinforce the colors of the different pitches.
    "});

    /// The instructions for the rhythm lessons.
    pub static ref MASTERY_INSTRUCTIONS: Ustr = Ustr::from(indoc! {"
        Sing or play the stated instrument to improvise using all the rhythmic, melodic, and
        harmonic elements you have mastered in the previous lessons. There are no individual lessons
        for each key. Instead, you should pick a random key each time you perform this exercise.

        When singing, use your prefered sight-singing system. When using your instrument, you should
        sing along to reinforce the colors of the different pitches.
    "});
}
