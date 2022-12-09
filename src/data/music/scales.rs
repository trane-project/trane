//! Defines musical scales for use in generating music courses.

use anyhow::{anyhow, Result};

use crate::data::music::intervals::*;
use crate::data::music::notes::*;

/// Defines a tonal scale.
#[derive(Clone, Debug)]
pub struct Scale {
    /// The tonic of the scale.
    pub tonic: Note,

    /// The notes which form the scale in the correct order.
    pub notes: Vec<Note>,
}

/// Defines a type of scale.
#[derive(Clone, Copy, Debug)]
#[allow(missing_docs)]
pub enum ScaleType {
    Major,
    Minor,
    MajorPentatonic,
    MinorPentatonic,
}

impl ToString for ScaleType {
    fn to_string(&self) -> String {
        match self {
            ScaleType::Major => "Major".to_string(),
            ScaleType::Minor => "Minor".to_string(),
            ScaleType::MajorPentatonic => "Major Pentatonic".to_string(),
            ScaleType::MinorPentatonic => "Minor Pentatonic".to_string(),
        }
    }
}

impl Note {
    /// Returns the note that is the relative minor of the given major key.
    pub fn relative_minor(&self) -> Result<Note> {
        match *self {
            Note::A => Ok(Note::F_SHARP),
            Note::A_FLAT => Ok(Note::F),
            Note::B => Ok(Note::G_SHARP),
            Note::B_FLAT => Ok(Note::G),
            Note::C => Ok(Note::A),
            Note::C_FLAT => Ok(Note::A_FLAT),
            Note::C_SHARP => Ok(Note::A_SHARP),
            Note::D => Ok(Note::B),
            Note::D_FLAT => Ok(Note::B_FLAT),
            Note::E => Ok(Note::C_SHARP),
            Note::E_FLAT => Ok(Note::C),
            Note::F => Ok(Note::D),
            Note::F_SHARP => Ok(Note::D_SHARP),
            Note::G => Ok(Note::E),
            Note::G_FLAT => Ok(Note::E_FLAT),
            _ => Err(anyhow!(
                "relative minor not found for note {}",
                self.to_string()
            )),
        }
    }

    /// Returns the note that is the relative major of the given minor key.
    pub fn relative_major(&self) -> Result<Note> {
        match *self {
            Note::A => Ok(Note::C),
            Note::A_FLAT => Ok(Note::C_FLAT),
            Note::A_SHARP => Ok(Note::C_SHARP),
            Note::B => Ok(Note::D),
            Note::B_FLAT => Ok(Note::D_FLAT),
            Note::C => Ok(Note::E_FLAT),
            Note::C_SHARP => Ok(Note::E),
            Note::D => Ok(Note::F),
            Note::D_SHARP => Ok(Note::F_SHARP),
            Note::E => Ok(Note::G),
            Note::E_FLAT => Ok(Note::G_FLAT),
            Note::F => Ok(Note::A_FLAT),
            Note::F_SHARP => Ok(Note::A),
            Note::G => Ok(Note::B_FLAT),
            Note::G_SHARP => Ok(Note::B),
            _ => Err(anyhow!(
                "relative major not found for note {}",
                self.to_string()
            )),
        }
    }
}

impl ScaleType {
    /// Returns a scale of the given type and tonic.
    pub fn notes(&self, tonic: Note) -> Result<Scale> {
        match &self {
            ScaleType::Major => match tonic {
                // A – B – C♯ – D – E – F♯ – G♯
                Note::A => Ok(Scale {
                    tonic,
                    notes: vec![
                        Note::A,
                        Note::B,
                        Note::C_SHARP,
                        Note::D,
                        Note::E,
                        Note::F_SHARP,
                        Note::G_SHARP,
                    ],
                }),

                // A♭ – B♭ – C – D♭ – E♭ – F – G
                Note::A_FLAT => Ok(Scale {
                    tonic,
                    notes: vec![
                        Note::A_FLAT,
                        Note::B_FLAT,
                        Note::C,
                        Note::D_FLAT,
                        Note::E_FLAT,
                        Note::F,
                        Note::G,
                    ],
                }),

                // B – C♯ – D♯ – E – F♯ – G♯ – A♯
                Note::B => Ok(Scale {
                    tonic,
                    notes: vec![
                        Note::B,
                        Note::C_SHARP,
                        Note::D_SHARP,
                        Note::E,
                        Note::F_SHARP,
                        Note::G_SHARP,
                        Note::A_SHARP,
                    ],
                }),

                // B♭ – C – D – E♭ – F – G – A
                Note::B_FLAT => Ok(Scale {
                    tonic,
                    notes: vec![
                        Note::B_FLAT,
                        Note::C,
                        Note::D,
                        Note::E_FLAT,
                        Note::F,
                        Note::G,
                        Note::A,
                    ],
                }),

                // C - D - E - F - G - A - B
                Note::C => Ok(Scale {
                    tonic,
                    notes: vec![
                        Note::C,
                        Note::D,
                        Note::E,
                        Note::F,
                        Note::G,
                        Note::A,
                        Note::B,
                    ],
                }),

                // C♭ – D♭ – E♭ – F♭ – G♭ – A♭ – B♭
                Note::C_FLAT => Ok(Scale {
                    tonic,
                    notes: vec![
                        Note::C_FLAT,
                        Note::D_FLAT,
                        Note::E_FLAT,
                        Note::F_FLAT,
                        Note::G_FLAT,
                        Note::A_FLAT,
                        Note::B_FLAT,
                    ],
                }),

                // C♯ – D♯ – E♯ – F♯ – G♯ – A♯ – B♯
                Note::C_SHARP => Ok(Scale {
                    tonic,
                    notes: vec![
                        Note::C_SHARP,
                        Note::D_SHARP,
                        Note::E_SHARP,
                        Note::F_SHARP,
                        Note::G_SHARP,
                        Note::A_SHARP,
                        Note::B_SHARP,
                    ],
                }),

                // D – E – F♯ – G – A – B – C♯
                Note::D => Ok(Scale {
                    tonic,
                    notes: vec![
                        Note::D,
                        Note::E,
                        Note::F_SHARP,
                        Note::G,
                        Note::A,
                        Note::B,
                        Note::C_SHARP,
                    ],
                }),

                // D♭ – E♭ – F – G♭ – A♭ – B♭ – C
                Note::D_FLAT => Ok(Scale {
                    tonic,
                    notes: vec![
                        Note::D_FLAT,
                        Note::E_FLAT,
                        Note::F,
                        Note::G_FLAT,
                        Note::A_FLAT,
                        Note::B_FLAT,
                        Note::C,
                    ],
                }),

                // E – F♯ – G♯ – A – B – C♯ – D♯
                Note::E => Ok(Scale {
                    tonic,
                    notes: vec![
                        Note::E,
                        Note::F_SHARP,
                        Note::G_SHARP,
                        Note::A,
                        Note::B,
                        Note::C_SHARP,
                        Note::D_SHARP,
                    ],
                }),

                // E♭ – F – G – A♭ – B♭ – C – D
                Note::E_FLAT => Ok(Scale {
                    tonic,
                    notes: vec![
                        Note::E_FLAT,
                        Note::F,
                        Note::G,
                        Note::A_FLAT,
                        Note::B_FLAT,
                        Note::C,
                        Note::D,
                    ],
                }),

                // F – G – A – B♭ – C – D – E
                Note::F => Ok(Scale {
                    tonic,
                    notes: vec![
                        Note::F,
                        Note::G,
                        Note::A,
                        Note::B_FLAT,
                        Note::C,
                        Note::D,
                        Note::E,
                    ],
                }),

                // F♯ – G♯ – A♯ – B – C♯ – D♯ – E♯
                Note::F_SHARP => Ok(Scale {
                    tonic,
                    notes: vec![
                        Note::F_SHARP,
                        Note::G_SHARP,
                        Note::A_SHARP,
                        Note::B,
                        Note::C_SHARP,
                        Note::D_SHARP,
                        Note::E_SHARP,
                    ],
                }),

                // G – A – B – C – D – E – F♯
                Note::G => Ok(Scale {
                    tonic,
                    notes: vec![
                        Note::G,
                        Note::A,
                        Note::B,
                        Note::C,
                        Note::D,
                        Note::E,
                        Note::F_SHARP,
                    ],
                }),

                // G♭ – A♭ – B♭ – C♭ – D♭ – E♭ – F
                Note::G_FLAT => Ok(Scale {
                    tonic,
                    notes: vec![
                        Note::G_FLAT,
                        Note::A_FLAT,
                        Note::B_FLAT,
                        Note::C_FLAT,
                        Note::D_FLAT,
                        Note::E_FLAT,
                        Note::F,
                    ],
                }),
                _ => Err(anyhow!(
                    "major scale not found for note {}",
                    tonic.to_string()
                )),
            },

            ScaleType::Minor => {
                let relative_major = ScaleType::Major
                    .notes(tonic.relative_major()?)
                    .map_err(|_| anyhow!("minor scale not found for note {}", tonic.to_string()))?;

                Ok(Scale {
                    tonic: relative_major.tonic,
                    notes: vec![
                        relative_major.notes[5],
                        relative_major.notes[6],
                        relative_major.notes[0],
                        relative_major.notes[1],
                        relative_major.notes[2],
                        relative_major.notes[3],
                        relative_major.notes[4],
                    ],
                })
            }

            ScaleType::MajorPentatonic => {
                let major = ScaleType::Major.notes(tonic).map_err(|_| {
                    anyhow!(
                        "major pentatonic scale not found for note {}",
                        tonic.to_string()
                    )
                })?;
                Ok(Scale {
                    tonic,
                    notes: vec![
                        major.notes[0],
                        major.notes[1],
                        major.notes[2],
                        major.notes[4],
                        major.notes[5],
                    ],
                })
            }

            ScaleType::MinorPentatonic => {
                let minor = ScaleType::Minor.notes(tonic).map_err(|_| {
                    anyhow!(
                        "minor pentatonic scale not found for note {}",
                        tonic.to_string()
                    )
                })?;
                Ok(Scale {
                    tonic,
                    notes: vec![
                        minor.notes[0],
                        minor.notes[2],
                        minor.notes[3],
                        minor.notes[4],
                        minor.notes[6],
                    ],
                })
            }
        }
    }

    /// Returns the intervals in the scale.
    pub fn intervals(&self) -> Result<Vec<Interval>> {
        match &self {
            ScaleType::Major => Ok(vec![
                Interval::Unison,
                Interval::MajorSecond,
                Interval::MajorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MajorSixth,
                Interval::MajorSeventh,
            ]),
            ScaleType::Minor => Ok(vec![
                Interval::Unison,
                Interval::MajorSecond,
                Interval::MinorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MinorSixth,
                Interval::MinorSeventh,
            ]),
            ScaleType::MajorPentatonic => Ok(vec![
                Interval::Unison,
                Interval::MajorSecond,
                Interval::MajorThird,
                Interval::PerfectFifth,
                Interval::MajorSixth,
            ]),
            ScaleType::MinorPentatonic => Ok(vec![
                Interval::Unison,
                Interval::MinorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MinorSeventh,
            ]),
        }
    }
}
