//! Contains the logic to generate special types of courses on the fly.
//!
//! This module adds support for declaring special types of courses whose manifests are
//! auto-generated on the fly when Trane first opens the library in which they belong. Doing so
//! allows users to declare complex courses with minimal configuration and ensures the generated
//! manifests always match the current version of Trane.

pub mod improvisation;
pub mod knowledge_base;
pub mod music_piece;
pub mod transcription;
