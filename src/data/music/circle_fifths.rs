use crate::data::music::notes::Note;

impl Note {
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
}
