//! Contains functions for working with the circle of fifths.

use crate::data::music::notes::Note;

// TODO: option to not include enharmonic keys.
impl Note {
    /// Returns all the notes in the circle of fifths.
    pub fn all_keys() -> Vec<Note> {
        vec![
            // Key with no sharps or flats.
            Note::C,
            // Keys with at least one sharp.
            Note::G,
            Note::D,
            Note::A,
            Note::E,
            Note::B,
            Note::F_SHARP,
            Note::C_SHARP,
            // Keys with at least one flat.
            Note::F,
            Note::B_FLAT,
            Note::E_FLAT,
            Note::A_FLAT,
            Note::D_FLAT,
            Note::G_FLAT,
            Note::C_FLAT,
        ]
    }

    /// Returns the note obtained by moving clockwise through the circle of fifths.
    pub fn clockwise(&self) -> Option<Note> {
        match *self {
            Note::C => Some(Note::G),
            Note::G => Some(Note::D),
            Note::D => Some(Note::A),
            Note::A => Some(Note::E),
            Note::E => Some(Note::B),
            Note::B => Some(Note::F_SHARP),
            Note::F_SHARP => Some(Note::C_SHARP),
            Note::C_SHARP => None,
            
            Note::F => Some(Note::C),
            Note::B_FLAT => Some(Note::F),
            Note::E_FLAT => Some(Note::B_FLAT),
            Note::A_FLAT => Some(Note::E_FLAT),
            Note::D_FLAT => Some(Note::A_FLAT),
            Note::G_FLAT => Some(Note::D_FLAT),
            Note::C_FLAT => Some(Note::G_FLAT),
            _ => None,
        }
    }

    /// Returns the note obtained by moving counter-clockwise through the circle of fifths.
    pub fn counter_clockwise(&self) -> Option<Note> {
        match *self {
            Note::C => Some(Note::F),
            Note::F => Some(Note::B_FLAT),
            Note::B_FLAT => Some(Note::E_FLAT),
            Note::E_FLAT => Some(Note::A_FLAT),
            Note::A_FLAT => Some(Note::D_FLAT),
            Note::D_FLAT => Some(Note::G_FLAT),
            Note::G_FLAT => Some(Note::C_FLAT),
            Note::C_FLAT => None,

            Note::G => Some(Note::C),
            Note::D => Some(Note::G),
            Note::A => Some(Note::D),
            Note::E => Some(Note::A),
            Note::B => Some(Note::E),
            Note::F_SHARP => Some(Note::B),
            Note::C_SHARP => Some(Note::F_SHARP),
            _ => None,
        }
    }

    /// Returns the previous key in the circle of fifths, that is, the key with one fewer sharp or
    /// flat.
    pub fn previous_key_in_circle(&self) -> Option<Note> {
        match *self {
            // The key of C is always the first key in the circle of fifths from which traversal
            // should start, so it has no previous key.
            Note::C => None,

            // The keys with at least one sharp.
            Note::G => Some(Note::C),
            Note::D => Some(Note::G),
            Note::A => Some(Note::D),
            Note::E => Some(Note::A),
            Note::B => Some(Note::E),
            Note::F_SHARP => Some(Note::B),
            Note::C_SHARP => Some(Note::F_SHARP),

            // The keys with at least one flat.
            Note::F => Some(Note::C),
            Note::B_FLAT => Some(Note::F),
            Note::E_FLAT => Some(Note::B_FLAT),
            Note::A_FLAT => Some(Note::E_FLAT),
            Note::D_FLAT => Some(Note::A_FLAT),
            Note::G_FLAT => Some(Note::D_FLAT),
            Note::C_FLAT => Some(Note::G_FLAT),

            // Return None for any other note.
            _ => None,
        }
    }

    /// Returns the last keys accessible by traversing the circle of fifths in clockwise and
    /// counter-clockwise directions.
    pub fn last_keys_in_circle() -> Vec<Note> {
        vec![Note::C_SHARP, Note::C_FLAT]
    }
}
