//! Runs the mantra miner that recites mantras while Trane is running.
//!
//! As a symbolic way to let users of Trane contribute back to the project, Trane spins up an
//! instance of the mantra-miner library that "recites" Tara Sarasvati's mantras in a background
//! thread while Trane is running. Her mantras were chosen because she is the manifestation of Tara
//! most closely related to wisdom, learning, and music and thus match the stated purpose of Trane.

use indoc::indoc;
use mantra_miner::{Mantra, MantraMiner, Options};

/// Runs the mantra miner that recites mantra while Trane is running.
///
/// The preparation, mantras, and conclusion are taken from Dilgo Khyentse Rinpoche's sadhana, whose
/// full text can be found at
/// `<https://www.lotsawahouse.org/tibetan-masters/dilgo-khyentse/sarasvati-sadhana-nyingtik>`.
pub struct TraneMantraMiner {
    /// An instance of the mantra miner.
    pub mantra_miner: MantraMiner,
}

impl TraneMantraMiner {
    fn options() -> Options {
        Options {
            // The preparation is taken from the "Refuge and Bodhicitta" section of the sadhana.
            //
            // namo, lama chok sum nyurma pal
            // Namo. In the guru, Three Jewels and swift and glorious Lady,
            //
            // güpé kyab chi khanyam dro
            // I take refuge with devotion. To bring all beings, as vast as space in number,
            //
            // lamé changchub chok tob chir
            // To supreme, unsurpassable awakening,
            //
            // pakma yangchen drubpar gyi
            // I shall meditate on noble Sarasvatī.
            preparation: Some(
                indoc! {r#"
                    namo, lama chok sum nyurma pal
                    güpé kyab chi khanyam dro
                    lamé changchub chok tob chir
                    pakma yangchen drubpar gyi
                "#}
                .to_string(),
            ),
            // The instructions of the sadhana state that the preparation should be repeated three
            // times.
            preparation_repeats: Some(3),
            // Two mantras are recited. The first is the principal mantra of Tara Sarasvati as
            // stated in the sadhana. The second one is repeating the seed syllable "hrim" 108
            // times, which the sadhana states one should repeat if they wish to sharpen their
            // intelligence. The principal mantra is "om pemo yogini taré tuttaré turé prajna hrim
            // hrim soha".
            mantras: vec![
                Mantra {
                    syllables: vec![
                        "om".to_string(),
                        "pe".to_string(),
                        "mo".to_string(),
                        "yo".to_string(),
                        "gi".to_string(),
                        "ni".to_string(),
                        "ta".to_string(),
                        "ré".to_string(),
                        "tu".to_string(),
                        "tta".to_string(),
                        "ré".to_string(),
                        "tu".to_string(),
                        "ré".to_string(),
                        "praj".to_string(),
                        "na".to_string(),
                        "hrim".to_string(),
                        "hrim".to_string(),
                        "so".to_string(),
                        "ha".to_string(),
                    ],
                    repeats: None,
                },
                Mantra {
                    syllables: vec!["hrim".to_string()],
                    repeats: Some(108),
                },
            ],
            // The conclusion is taken from the "Dedication and Aspiration" part of the sadhana.
            //
            // gewa di yi nyurdu dak
            // Through the positivity and merit of this, may I swiftly
            //
            // drayang lhamo drub gyur né
            // Attain the realization of the goddess Sarasvatī, and thereby
            //
            // drowa chik kyang malüpa
            // Every single sentient being
            //
            // dé yi sa la göpar shok
            // Reach her state of perfection too.
            conclusion: Some(
                indoc! {r#"
                    gewa di yi nyurdu dak
                    drayang lhamo drub gyur né
                    drowa chik kyang malüpa
                    dé yi sa la göpar shok
                "#}
                .to_string(),
            ),
            // The sadhana does not ask the conclusion to be repeated but do so for the sake of
            // symmetry.
            conclusion_repeats: Some(3),
            repeats: None,
            rate_ns: 100_000,
        }
    }
}

impl Default for TraneMantraMiner {
    fn default() -> Self {
        Self {
            mantra_miner: MantraMiner::new(Self::options()),
        }
    }
}
