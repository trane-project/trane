//! Defines the notes and accidentals for use in generating music courses.

/// Defines the names of the natural notes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum NaturalNote {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
}

impl ToString for NaturalNote {
    fn to_string(&self) -> String {
        match &self {
            NaturalNote::A => "A".to_string(),
            NaturalNote::B => "B".to_string(),
            NaturalNote::C => "C".to_string(),
            NaturalNote::D => "D".to_string(),
            NaturalNote::E => "E".to_string(),
            NaturalNote::F => "F".to_string(),
            NaturalNote::G => "G".to_string(),
        }
    }
}

/// Defines the pitch accidentals that can be applied to a note.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Accidental {
    Natural,
    Flat,
    Sharp,
}

impl ToString for Accidental {
    fn to_string(&self) -> String {
        match &self {
            Accidental::Natural => String::new(),
            Accidental::Flat => "♭".to_string(),
            Accidental::Sharp => "♯".to_string(),
        }
    }
}

/// Defines the union of a natural note and an accidental that describes a note.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Note(pub NaturalNote, pub Accidental);

#[allow(missing_docs)]
impl Note {
    pub const A: Note = Note(NaturalNote::A, Accidental::Natural);
    pub const A_FLAT: Note = Note(NaturalNote::A, Accidental::Flat);
    pub const A_SHARP: Note = Note(NaturalNote::A, Accidental::Sharp);
    pub const B: Note = Note(NaturalNote::B, Accidental::Natural);
    pub const B_FLAT: Note = Note(NaturalNote::B, Accidental::Flat);
    pub const B_SHARP: Note = Note(NaturalNote::B, Accidental::Sharp);
    pub const C: Note = Note(NaturalNote::C, Accidental::Natural);
    pub const C_FLAT: Note = Note(NaturalNote::C, Accidental::Flat);
    pub const C_SHARP: Note = Note(NaturalNote::C, Accidental::Sharp);
    pub const D: Note = Note(NaturalNote::D, Accidental::Natural);
    pub const D_FLAT: Note = Note(NaturalNote::D, Accidental::Flat);
    pub const D_SHARP: Note = Note(NaturalNote::D, Accidental::Sharp);
    pub const E: Note = Note(NaturalNote::E, Accidental::Natural);
    pub const E_FLAT: Note = Note(NaturalNote::E, Accidental::Flat);
    pub const E_SHARP: Note = Note(NaturalNote::E, Accidental::Sharp);
    pub const F: Note = Note(NaturalNote::F, Accidental::Natural);
    pub const F_FLAT: Note = Note(NaturalNote::F, Accidental::Flat);
    pub const F_SHARP: Note = Note(NaturalNote::F, Accidental::Sharp);
    pub const G: Note = Note(NaturalNote::G, Accidental::Natural);
    pub const G_FLAT: Note = Note(NaturalNote::G, Accidental::Flat);
    pub const G_SHARP: Note = Note(NaturalNote::G, Accidental::Sharp);

    /// Returns a representation of the note without Unicode characters for use in directory names
    /// and other contexts where Unicode is harder or impossible to use.
    #[must_use]
    pub fn to_ascii_string(&self) -> String {
        let accidental = match self.1 {
            Accidental::Natural => String::new(),
            Accidental::Flat => "_flat".to_string(),
            Accidental::Sharp => "_sharp".to_string(),
        };
        format!("{}{}", self.0.to_string(), accidental)
    }
}

impl ToString for Note {
    fn to_string(&self) -> String {
        let mut output = self.0.to_string();
        output.push_str(&self.1.to_string());
        output
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Verifies converting a note to a string.
    #[test]
    fn to_string() {
        assert_eq!(NaturalNote::A.to_string(), "A");
        assert_eq!(NaturalNote::B.to_string(), "B");
        assert_eq!(NaturalNote::C.to_string(), "C");
        assert_eq!(NaturalNote::D.to_string(), "D");
        assert_eq!(NaturalNote::E.to_string(), "E");
        assert_eq!(NaturalNote::F.to_string(), "F");
        assert_eq!(NaturalNote::G.to_string(), "G");

        assert_eq!(Note(NaturalNote::A, Accidental::Natural).to_string(), "A");
        assert_eq!(Note(NaturalNote::A, Accidental::Flat).to_string(), "A♭");
        assert_eq!(Note(NaturalNote::A, Accidental::Sharp).to_string(), "A♯");
    }

    /// Verifies converting a note to an ASCII string.
    #[test]
    fn to_ascii_string() {
        assert_eq!(
            Note(NaturalNote::A, Accidental::Natural).to_ascii_string(),
            "A"
        );
        assert_eq!(
            Note(NaturalNote::A, Accidental::Flat).to_ascii_string(),
            "A_flat"
        );
        assert_eq!(
            Note(NaturalNote::A, Accidental::Sharp).to_ascii_string(),
            "A_sharp"
        );
    }

    /// Verifies that notes can be cloned. Done to ensure that the auto-generated trait
    /// implementation is included in the code coverage report.
    #[test]
    fn note_clone() {
        let note = Note(NaturalNote::A, Accidental::Natural);
        let clone = note.clone();
        assert_eq!(note, clone);
    }
}
