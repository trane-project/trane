//! Manages the download of asset files for transcription courses.
//!
//! Transcription courses include references to external assets. Manually downloading them is a
//! cumbersome process, so this module automates the process.

use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

use anyhow::{bail, Result};
use parking_lot::RwLock;
use sha1::{Digest, Sha1};
use ustr::Ustr;

use crate::{
    course_library::CourseLibrary,
    data::{
        course_generator::transcription::{TranscriptionLink, TranscriptionPreferences},
        ExerciseAsset,
    },
    TranscriptionDownloaderError,
};

/// Downloads transcription assets to local storage.
pub trait TranscriptionDownloader {
    /// Checks if the given asset has been downloaded.
    fn is_downloaded(&self, exercise_id: Ustr) -> bool;

    /// Downloads the given asset.
    fn download_asset(
        &self,
        exercise_id: Ustr,
        force_download: bool,
    ) -> Result<(), TranscriptionDownloaderError>;

    /// Returns the download path for the given asset.
    fn download_path(&self, exercise_id: Ustr) -> Option<PathBuf>;

    /// Returns the download path alias for the given asset.
    fn download_path_alias(&self, exercise_id: Ustr) -> Option<PathBuf>;
}

/// An implementation of `TranscriptionDownloader` that downloads assets to the local filesystem.
pub struct LocalTranscriptionDownloader {
    /// Preferences for transcription courses.
    pub preferences: TranscriptionPreferences,

    /// The course library from which to extract transcription courses.
    pub course_library: Arc<RwLock<dyn CourseLibrary>>,
}

impl LocalTranscriptionDownloader {
    /// Gets the transcription link from the given exercise, if it exists.
    fn get_link(&self, exercise_id: Ustr) -> Option<TranscriptionLink> {
        let exercise_manifest = self
            .course_library
            .read()
            .get_exercise_manifest(exercise_id)?;
        match &exercise_manifest.exercise_asset {
            ExerciseAsset::TranscriptionAsset { external_link, .. } => external_link.clone(),
            _ => None,
        }
    }

    /// Gets the name of the directory where the asset should be downloaded.
    fn download_dir(link: &TranscriptionLink) -> String {
        let TranscriptionLink::YouTube(input) = link;
        let mut hasher = Sha1::new();
        hasher.update(input.as_bytes());
        let hash = hasher.finalize();
        String::from_utf8_lossy(&hash).to_string()
    }

    /// Gets the name of the file to which download the asset.
    fn download_file(link: &TranscriptionLink) -> String {
        match link {
            TranscriptionLink::YouTube(_) => "audio.m4a".to_string(),
        }
    }

    /// Generates a download directory from the given link.
    fn download_directory(link: &TranscriptionLink) -> PathBuf {
        Path::new(&Self::download_dir(link)).join(Self::download_file(link))
    }

    /// Gets the full path to the asset file with the download directory prepended.
    fn full_download_path(&self, link: &TranscriptionLink) -> Option<PathBuf> {
        self.preferences
            .download_path
            .as_ref()
            .map(|download_path| Path::new(download_path).join(Self::download_directory(link)))
    }

    /// Gets the full path to the asset file with the alias directory prepended.
    fn full_alias_path(&self, link: &TranscriptionLink) -> Option<PathBuf> {
        self.preferences
            .download_path_alias
            .as_ref()
            .map(|path_alias| Path::new(path_alias).join(Self::download_directory(link)))
    }

    /// Verifies that a binary is installed. The argument should be something simple, like a version
    /// flag, that will exit quickly.
    fn verify_binary(name: &str, arg: &str) -> Result<()> {
        let status = Command::new(name).arg(arg).status();
        if status.is_err() {
            bail!("command \"{}\" cannot be found", name);
        }
        if !status.unwrap().success() {
            bail!("command \"{}\" failed", name);
        }
        Ok(())
    }

    /// Checks that the prerequisites to use the downloader are met.
    fn check_prerequisites(&self) -> Result<()> {
        // Check yt-dlp is installed.
        Self::verify_binary("yt-dlp", "--version")?;

        // Check the download path is valid.
        if self.preferences.download_path.is_none() {
            bail!("transcription download path is not set");
        }
        let download_path = Path::new(self.preferences.download_path.as_ref().unwrap());
        if !download_path.exists() {
            bail!("transcription download path does not exist");
        }
        Ok(())
    }

    /// Helper function to download an asset.
    fn download_asset_helper(&self, exercise_id: Ustr, force_download: bool) -> Result<()> {
        // Check if the asset has already been downloaded.
        self.check_prerequisites()?;
        let link = self.get_link(exercise_id);
        if link.is_none() {
            return Ok(());
        }
        let link = link.unwrap();
        let download_path = self.full_download_path(&link).unwrap();
        if download_path.exists() && !force_download {
            return Ok(());
        }

        // Create a temporary directory, download the asset, and copy it to the final location.
        let temp_dir = tempfile::tempdir()?;
        match link {
            TranscriptionLink::YouTube(yt_link) => {
                let temp_file = temp_dir.path().join("audio.m4a");
                let status = Command::new("yt-dlp")
                    .arg("--extract-audio")
                    .arg("--audio-format")
                    .arg("m4a")
                    .arg("--output")
                    .arg(temp_file.to_str().unwrap())
                    .arg(format!("\"{yt_link}\""))
                    .status()?;
                if !status.success() {
                    bail!("yt-dlp failed to download audio from URL {}", yt_link);
                }
                std::fs::create_dir_all(download_path.parent().unwrap())?;
                std::fs::copy(temp_file, &download_path)?;
            }
        }
        Ok(())
    }
}

impl TranscriptionDownloader for LocalTranscriptionDownloader {
    fn is_downloaded(&self, exercise_id: Ustr) -> bool {
        if self.preferences.download_path.is_none() {
            return false;
        }
        let link = self.get_link(exercise_id);
        if link.is_none() {
            return false;
        }
        let link = link.unwrap();
        let download_path = self.full_download_path(&link).unwrap();
        download_path.exists()
    }

    fn download_asset(
        &self,
        exercise_id: Ustr,
        force_download: bool,
    ) -> Result<(), TranscriptionDownloaderError> {
        self.download_asset_helper(exercise_id, force_download)
            .map_err(|e| TranscriptionDownloaderError::DownloadAsset(exercise_id, e))
    }

    fn download_path(&self, exercise_id: Ustr) -> Option<PathBuf> {
        let link = self.get_link(exercise_id)?;
        self.full_download_path(&link)
    }

    fn download_path_alias(&self, exercise_id: Ustr) -> Option<PathBuf> {
        let link = self.get_link(exercise_id)?;
        self.full_alias_path(&link)
    }
}
