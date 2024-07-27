use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::data::{CourseManifest, GenerateManifests, GeneratedCourse, UserPreferences};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct LiteracyConfig {
    #[serde(default)]
    inline_examples: Vec<String>,

    #[serde(default)]
    inline_exceptions: Vec<String>,
}

impl GenerateManifests for LiteracyConfig {
    fn generate_manifests(
        &self,
        course_root: &Path,
        course_manifest: &CourseManifest,
        preferences: &UserPreferences,
    ) -> Result<GeneratedCourse> {
        unimplemented!()
    }
}
