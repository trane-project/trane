//! Manages the download of asset files for transcription courses.
//!
//! Transcription courses include references to external assets. Manually downloading them is a
//! cumbersome process, so this module automates the process.

use crate::{
    data::course_generator::transcription::TranscriptionAsset, TranscriptionDownloaderError,
};

/// Downloads transcription assets to local storage.
pub trait TranscriptionDownloader {
    /// Downloads the given asset.
    fn download_asset(&self, asset: TranscriptionAsset)
        -> Result<(), TranscriptionDownloaderError>;

    /// Checks if the given asset has been downloaded.
    fn is_downloaded(
        &self,
        asset: &TranscriptionAsset,
    ) -> Result<bool, TranscriptionDownloaderError>;

    /// Downloads all assets for all the transcription courses in the current Trane library.
    fn download_all_assets(&self) -> Result<(), TranscriptionDownloaderError>;
}

/// An implementation of `TranscriptionDownloader` that downloads assets to a directory inside the
/// `.trane` directory.
pub struct LocalTranscriptionDownloader {}

impl LocalTranscriptionDownloader {
    /// Creates a new `LocalTranscriptionDownloader`.
    pub fn new() -> Self {
        Self{}
    }
}
