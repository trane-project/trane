//! Runs the mantra miner that recites mantras while Trane is running.
//!
//! As a symbolic way to let users of Trane contribute back to the project, Trane spins up an
//! instance of the mantra-miner library that "recites" relevant mantras in a background thread.
//! There is no explicit tradition or textual precedent for electronic mantra recitation, but
//! carving mantras in stone and spinning them in prayer wheels is a common practice.
//!
//! The mantras chosen are:
//!
//! - The Song of the Vajra, a central song from the Dzogchen tradition that embodies the supreme
//!   realization that the true nature of all beings has been naturally perfect from the very
//!   beginning.
//! - The mantra of the bodhisattva Manjushri, embodiment of wisdom.
//! - The mantra of Tara Sarasvati, the female Buddha of knowledge, wisdom, and eloquence. She often
//!   appears with Manjushri.
//! - Mantras used to dedicate the merit of one's practice.

use indoc::indoc;
use mantra_miner::{Mantra, MantraMiner, Options};

/// Runs the mantra miner that recites mantra while Trane is running.
pub struct TraneMantraMiner {
    /// An instance of the mantra miner.
    pub mantra_miner: MantraMiner,
}

impl TraneMantraMiner {
    fn options() -> Options {
        Options {
            // The Song of the Vajra.
            preparation: Some(
                indoc! {r"
                    ཨ
                    
                    E MA KI RI KĪ RĪ
                    MA SṬA VA LI VĀ LĪ
                    SAMITA SU RU SŪ RŪ KUTALI MA SU MĀ SŪ
                    E KARA SULI BHAṬAYE CI KIRA BHULI BHAṬHAYE
                    SAMUNTA CARYA SUGHAYE BHETA SANA BHYA KU LAYE
                    SAKARI DHU KA NA MATARI VAI TA NA
                    PARALI HI SA NA MAKHARTA KHE LA NAM
                    SAMBHA RA THA ME KHA CA NTA PA SŪRYA BHA TA RAI PA SHA NA PA
                    RANA BI DHI SA GHU RA LA PA MAS MIN SA GHU LĪ TA YA PA
                    GHU RA GHŪ RĀ SA GHA KHAR ṆA LAM
                    NA RA NĀ RĀ ITHA PA ṬA LAM SIR ṆA SĪR ṆĀ BHE SARAS PA LAM
                    BHUN DHA BHŪN DHĀ CI SHA SA KE LAM
                    SA SĀ RI RĪ LI LĪ I Ī MI MĪ
                    RA RA RĀ
                "}
                .to_string(),
            ),
            preparation_repeats: None,
            // The mantras of Manjushri and Tara Sarasvati, each recited 108 times.
            mantras: vec![
                Mantra {
                    syllables: vec![
                        "om".to_string(),
                        "a".to_string(),
                        "ra".to_string(),
                        "pa".to_string(),
                        "tsa".to_string(),
                        "na".to_string(),
                        "dhi".to_string(),
                    ],
                    repeats: Some(108),
                },
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
                    repeats: Some(108),
                },
            ],
            // Mantras of dedication.
            conclusion: Some(
                indoc! {r"
                    OM DHARE DHARE BHANDHARE SVĀHĀ
                    JAYA JAYA SIDDHI SIDDHI PHALA PHALA
                    HĂ A HA SHA SA MA
                    MAMAKOLIṄ SAMANTA
                "}
                .to_string(),
            ),
            conclusion_repeats: None,
            repeats: None,
            rate_ns: 108_000,
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
