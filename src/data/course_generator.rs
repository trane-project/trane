//! Contains the logic to generate special types of courses on the fly.
//!
//! This module adds support for declaring special types of courses whose manifests are
//! auto-generated on the fly when Trane first opens the library in which they belong. Doing so
//! allows users to declare complex courses with minimal configuration and ensures the generated
//! manifests always match the current version of Trane.

use serde::{Deserialize, Serialize};
use typeshare::typeshare;

pub mod improvisation;
pub mod knowledge_base;
pub mod music_piece;
pub mod transcription;

//@<instrument
/// Describes an instrument that can be used to practice in a generated course.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[typeshare]
pub struct Instrument {
    /// The name of the instrument. For example, "Tenor Saxophone".
    pub name: String,

    /// An ID for this instrument used to generate lesson IDs. For example, "tenor_saxophone".
    pub id: String,
}
//>@instrument

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies cloning an instrument. Done so that the auto-generated trait implementation is
    /// included in the code coverage reports.
    #[test]
    fn instrument_clone() {
        let instrument = Instrument {
            name: "Piano".to_string(),
            id: "piano".to_string(),
        };
        let instrument_clone = instrument.clone();
        assert_eq!(instrument.name, instrument_clone.name);
        assert_eq!(instrument.id, instrument_clone.id);
    }
}
