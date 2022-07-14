//! Module defining the musical intervals.

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

impl ToString for Interval {
    fn to_string(&self) -> String {
        match self {
            Interval::Unison => "Unison".to_string(),
            Interval::MinorSecond => "Minor Second".to_string(),
            Interval::MajorSecond => "Major Second".to_string(),
            Interval::MinorThird => "Minor Third".to_string(),
            Interval::MajorThird => "Major Third".to_string(),
            Interval::PerfectFourth => "Perfect Fourth".to_string(),
            Interval::Tritone => "Tritone".to_string(),
            Interval::PerfectFifth => "Perfect Fifth".to_string(),
            Interval::MinorSixth => "Minor Sixth".to_string(),
            Interval::MajorSixth => "Major Sixth".to_string(),
            Interval::MinorSeventh => "Minor Seventh".to_string(),
            Interval::MajorSeventh => "Major Seventh".to_string(),
            Interval::Octave => "Octave".to_string(),
        }
    }
}
