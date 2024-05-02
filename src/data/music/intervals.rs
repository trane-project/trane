//! Defines the musical intervals.

use std::fmt::{Display, Formatter, Result};

/// Defines the different musical intervals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Interval {
    Unison,
    MinorSecond,
    MajorSecond,
    MinorThird,
    MajorThird,
    PerfectFourth,
    Tritone,
    PerfectFifth,
    MinorSixth,
    MajorSixth,
    MinorSeventh,
    MajorSeventh,
    Octave,
}

impl Display for Interval {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Interval::Unison => write!(f, "Unison"),
            Interval::MinorSecond => write!(f, "Minor Second"),
            Interval::MajorSecond => write!(f, "Major Second"),
            Interval::MinorThird => write!(f, "Minor Third"),
            Interval::MajorThird => write!(f, "Major Third"),
            Interval::PerfectFourth => write!(f, "Perfect Fourth"),
            Interval::Tritone => write!(f, "Tritone"),
            Interval::PerfectFifth => write!(f, "Perfect Fifth"),
            Interval::MinorSixth => write!(f, "Minor Sixth"),
            Interval::MajorSixth => write!(f, "Major Sixth"),
            Interval::MinorSeventh => write!(f, "Minor Seventh"),
            Interval::MajorSeventh => write!(f, "Major Seventh"),
            Interval::Octave => write!(f, "Octave"),
        }
    }
}
