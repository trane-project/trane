// Manages the download of asset files for transcription courses.
//
// Transcription courses include references to the assets used in the course. Manually downloading
// them is a cumbersome process, so this module automates the process.

use crate::{
    data::course_generator::transcription::TranscriptionAsset, TranscriptionDownloaderError,
};

/// Downloads transcription assets to local storage.
pub trait TranscriptionDownloader {
    /// Performs any initialization required to download assets.
    fn initialize(&self) -> Result<(), TranscriptionDownloaderError>;

    /// Downloads the given asset.
    fn download_asset(&self, asset: TranscriptionAsset)
        -> Result<(), TranscriptionDownloaderError>;

    /// Checks if the given asset has been downloaded.
    fn is_downloaded(
        &self,
        asset: &TranscriptionAsset,
    ) -> Result<bool, TranscriptionDownloaderError>;
}
