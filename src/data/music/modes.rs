use serde::{Deserialize, Serialize};

/// One of the seven musical modes.
/// Major and Minor correspond to the Ionian and Aeolian modes, respectively.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Mode {
    Ionian,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Aeolian,
    Locrian,
}
