//! Contains the logic to generate special types of courses on the fly.
//!
//! This module adds support for declaring special types of courses whose material is auto-generated
//! on the fly when Trane first opens a library. Doing so allows users to declare complex courses
//! with a minimal configuration and ensures the generated manifests always match the current
//! version of Trane.

pub mod trane_improvisation;
pub mod trane_music_piece;
