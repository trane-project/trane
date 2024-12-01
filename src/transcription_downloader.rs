//! Manages the download of asset files for transcription courses.
//!
//! Transcription courses include references to external assets. Manually downloading them is a
//! cumbersome process, so this module automates the process.

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
};

use anyhow::{bail, Result};
use parking_lot::RwLock;
use sha1::{Digest, Sha1};
use ustr::Ustr;

use crate::{
    course_library::{CourseLibrary, LocalCourseLibrary},
    data::{
        course_generator::transcription::{TranscriptionLink, TranscriptionPreferences},
        ExerciseAsset, ExerciseManifest,
    },
    TranscriptionDownloaderError,
};

/// A trait for getting the transcription link for an exercise.
pub trait TranscriptionLinkStore {
    /// Gets the transcription link for the given exercise.
    fn get_transcription_link(&self, exercise_id: Ustr) -> Option<TranscriptionLink>;
}

impl LocalCourseLibrary {
    fn extract_transcription_link(manifest: &ExerciseManifest) -> Option<TranscriptionLink> {
        match &manifest.exercise_asset {
            ExerciseAsset::TranscriptionAsset { external_link, .. } => external_link.clone(),
            _ => None,
        }
    }
}

impl TranscriptionLinkStore for LocalCourseLibrary {
    // grcov-excl-start: Hard to test without creating a full library. Functions called are
    // tested separately.
    fn get_transcription_link(&self, exercise_id: Ustr) -> Option<TranscriptionLink> {
        let exercise_manifest = self.get_exercise_manifest(exercise_id)?;
        Self::extract_transcription_link(&exercise_manifest)
    }
    // grcov-excl-stop
}

/// Downloads transcription assets to local storage.
pub trait TranscriptionDownloader {
    /// Checks if the given asset has been downloaded.
    fn is_transcription_asset_downloaded(&self, exercise_id: Ustr) -> bool;

    /// Downloads the given asset.
    fn download_transcription_asset(
        &self,
        exercise_id: Ustr,
        force_download: bool,
    ) -> Result<(), TranscriptionDownloaderError>;

    /// Returns the download path for the given asset.
    fn transcription_download_path(&self, exercise_id: Ustr) -> Option<PathBuf>;

    /// Returns the download path alias for the given asset.
    fn transcription_download_path_alias(&self, exercise_id: Ustr) -> Option<PathBuf>;
}

/// An implementation of `TranscriptionDownloader` that downloads assets to the local filesystem.
pub struct LocalTranscriptionDownloader {
    /// Preferences for transcription courses.
    pub preferences: TranscriptionPreferences,

    /// The course library from which to get the transcription links.
    pub link_store: Arc<RwLock<dyn TranscriptionLinkStore + Send + Sync>>,
}

impl LocalTranscriptionDownloader {
    /// Gets the name of the directory where the asset should be downloaded.
    fn download_dir_name(link: &TranscriptionLink) -> String {
        let TranscriptionLink::YouTube(input) = link;
        let mut hasher = Sha1::new();
        hasher.update(input.as_bytes());
        let hash = hasher.finalize();
        hex::encode(hash)
    }

    /// Gets the name of the file to which download the asset.
    fn download_file_name(link: &TranscriptionLink) -> String {
        match link {
            TranscriptionLink::YouTube(_) => "audio.m4a".to_string(),
        }
    }

    /// Generates a download path relative to the root download directory.
    fn rel_download_path(link: &TranscriptionLink) -> PathBuf {
        Path::new(&Self::download_dir_name(link)).join(Self::download_file_name(link))
    }

    /// Gets the full path to the asset file with the download directory prepended.
    fn full_download_path(&self, link: &TranscriptionLink) -> Option<PathBuf> {
        self.preferences
            .download_path
            .as_ref()
            .map(|download_path| Path::new(download_path).join(Self::rel_download_path(link)))
    }

    /// Gets the full path to the asset file with the alias directory prepended.
    fn full_alias_path(&self, link: &TranscriptionLink) -> Option<PathBuf> {
        self.preferences
            .download_path_alias
            .as_ref()
            .map(|path_alias| Path::new(path_alias).join(Self::rel_download_path(link)))
    }

    /// Verifies that a binary is installed. The argument should be something simple, like a version
    /// flag, that will exit quickly.
    fn verify_binary(name: &str, arg: &str) -> Result<()> {
        // grcov-excl-start: Hard to test this function since errors require removing the binary.
        let status = Command::new(name)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .arg(arg)
            .status();
        if status.is_err() {
            bail!("command \"{}\" cannot be found", name);
        }
        if !status.unwrap().success() {
            bail!("command \"{}\" failed", name);
        }
        Ok(())
        // grcov-excl-stop
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
        let link = self.link_store.read().get_transcription_link(exercise_id);
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
                let output = Command::new("yt-dlp")
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped())
                    .arg("--enable-file-urls")
                    .arg("--extract-audio")
                    .arg("--audio-format")
                    .arg("m4a")
                    .arg("--output")
                    .arg(temp_file.to_str().unwrap())
                    .arg(&yt_link)
                    .output()?;
                if !output.status.success() {
                    let err = String::from_utf8_lossy(&output.stderr);
                    bail!(
                        "yt-dlp failed to download audio from URL {}: {}",
                        yt_link,
                        err
                    );
                }
                std::fs::create_dir_all(download_path.parent().unwrap())?;
                std::fs::copy(temp_file, &download_path)?;
            }
        }
        Ok(())
    }
}

impl TranscriptionDownloader for LocalTranscriptionDownloader {
    fn is_transcription_asset_downloaded(&self, exercise_id: Ustr) -> bool {
        if self.preferences.download_path.is_none() {
            return false;
        }
        let link = self.link_store.read().get_transcription_link(exercise_id);
        if link.is_none() {
            return false;
        }
        let link = link.unwrap();
        let download_path = self.full_download_path(&link).unwrap();
        download_path.exists()
    }

    fn download_transcription_asset(
        &self,
        exercise_id: Ustr,
        force_download: bool,
    ) -> Result<(), TranscriptionDownloaderError> {
        self.download_asset_helper(exercise_id, force_download)
            .map_err(|e| TranscriptionDownloaderError::DownloadAsset(exercise_id, e))
    }

    fn transcription_download_path(&self, exercise_id: Ustr) -> Option<PathBuf> {
        let link = self.link_store.read().get_transcription_link(exercise_id)?;
        self.full_download_path(&link)
    }

    fn transcription_download_path_alias(&self, exercise_id: Ustr) -> Option<PathBuf> {
        let link = self.link_store.read().get_transcription_link(exercise_id)?;
        self.full_alias_path(&link)
    }
}

#[cfg(test)]
mod test {
    use std::{path::{self, Path}, sync::Arc};
    use ustr::Ustr;

    use crate::{
        course_library::LocalCourseLibrary,
        data::{
            course_generator::transcription::{TranscriptionLink, TranscriptionPreferences},
            BasicAsset, ExerciseAsset, ExerciseManifest, ExerciseType,
        },
        transcription_downloader::{
            LocalTranscriptionDownloader, TranscriptionDownloader, TranscriptionLinkStore,
        },
    };

    struct MockLinkStore {
        link: Option<TranscriptionLink>,
    }
    impl TranscriptionLinkStore for MockLinkStore {
        fn get_transcription_link(&self, _exercise_id: Ustr) -> Option<TranscriptionLink> {
            self.link.clone()
        }
    }

    // Test link to a real YouTube video: Margaret Glaspy and Julian Lage perform “Best Behavior”.
    const YT_LINK: &str = "https://www.youtube.com/watch?v=p4LgzLjF4xE";

    // A local copy of the file above to avoid using the network in tests. 
    const LOCAL_FILE: &str = "testdata/test_audio.m4a";

    /// Verifies extracting the link from a valid exercise manifest.
    #[test]
    fn test_extract_link() {
        // Transcription asset with no link.
        let mut manifest = ExerciseManifest {
            exercise_asset: ExerciseAsset::TranscriptionAsset {
                content: "content".to_string(),
                external_link: None,
            },
            id: Ustr::from("exercise_id"),
            lesson_id: Ustr::from("lesson_id"),
            course_id: Ustr::from("course_id"),
            name: "Exercise Name".to_string(),
            description: None,
            exercise_type: ExerciseType::Procedural,
        };
        assert!(LocalCourseLibrary::extract_transcription_link(&manifest).is_none());

        // Transcription asset with a link.
        manifest.exercise_asset = ExerciseAsset::TranscriptionAsset {
            content: "content".to_string(),
            external_link: Some(TranscriptionLink::YouTube(YT_LINK.into())),
        };
        assert_eq!(
            TranscriptionLink::YouTube(YT_LINK.into()),
            LocalCourseLibrary::extract_transcription_link(&manifest).unwrap()
        );

        // Other type of asset.
        manifest.exercise_asset = ExerciseAsset::BasicAsset(BasicAsset::InlinedAsset {
            content: "content".to_string(),
        });
        assert!(LocalCourseLibrary::extract_transcription_link(&manifest).is_none());
    }

    /// Verifies that exercises with no links are marked as not downloaded.
    #[test]
    fn test_is_downloaded_no_link() {
        let link_store = MockLinkStore { link: None };
        let downloader = LocalTranscriptionDownloader {
            preferences: Default::default(),
            link_store: Arc::new(parking_lot::RwLock::new(link_store)),
        };
        assert!(!downloader.is_transcription_asset_downloaded(Ustr::from("exercise")));
    }

    /// Verifies that exercises that have not been downloaded are marked as such.
    #[test]
    fn test_is_downloaded_no_download() {
        let link_store = MockLinkStore {
            link: Some(TranscriptionLink::YouTube(YT_LINK.into())),
        };
        let downloader = LocalTranscriptionDownloader {
            preferences: Default::default(),
            link_store: Arc::new(parking_lot::RwLock::new(link_store)),
        };
        assert!(!downloader.is_transcription_asset_downloaded(Ustr::from("exercise")));
    }

    /// Verifies that downloading an asset fails if there's no download path set.
    #[test]
    fn test_download_asset_no_path_set() {
        let link_store = MockLinkStore {
            link: Some(TranscriptionLink::YouTube(YT_LINK.into())),
        };
        let downloader = LocalTranscriptionDownloader {
            preferences: TranscriptionPreferences {
                instruments: vec![],
                download_path: None,
                download_path_alias: None,
            },
            link_store: Arc::new(parking_lot::RwLock::new(link_store)),
        };
        // assert!(!downloader.is_downloaded(Ustr::from("exercise")));
        assert!(downloader
            .download_transcription_asset(Ustr::from("exercise"), false)
            .is_err());
    }

    /// Verifies that downloading an asset fails if the download path does not exist.
    #[test]
    fn test_download_asset_missing_dir() {
        let link_store = MockLinkStore {
            link: Some(TranscriptionLink::YouTube(YT_LINK.into())),
        };
        let downloader = LocalTranscriptionDownloader {
            preferences: TranscriptionPreferences {
                instruments: vec![],
                download_path: Some("/some/missing/dir".to_string()),
                download_path_alias: None,
            },
            link_store: Arc::new(parking_lot::RwLock::new(link_store)),
        };
        assert!(!downloader.is_transcription_asset_downloaded(Ustr::from("exercise")));
        assert!(downloader
            .download_transcription_asset(Ustr::from("exercise"), false)
            .is_err());
    }

    /// Verifies downloading an exercise with no link.
    #[test]
    fn test_download_asset_no_link() {
        let temp_dir = tempfile::tempdir().unwrap();
        let link_store = MockLinkStore { link: None };
        let downloader = LocalTranscriptionDownloader {
            preferences: TranscriptionPreferences {
                instruments: vec![],
                download_path: Some(temp_dir.path().to_str().unwrap().to_string()),
                download_path_alias: None,
            },
            link_store: Arc::new(parking_lot::RwLock::new(link_store)),
        };
        assert!(!downloader.is_transcription_asset_downloaded(Ustr::from("exercise")));
        downloader
            .download_transcription_asset(Ustr::from("exercise"), false)
            .unwrap();
        assert!(!downloader.is_transcription_asset_downloaded(Ustr::from("exercise")));
    }

    /// Verifies downloading a valid asset.
    #[test]
    fn test_download_valid_asset() {
        let temp_dir = tempfile::tempdir().unwrap();
        let local_path = path::absolute(Path::new(LOCAL_FILE)).unwrap();
        let file_link = format!("file://{}", local_path.to_str().unwrap());
        let link_store = MockLinkStore {
            link: Some(TranscriptionLink::YouTube(file_link)),
        };
        let downloader = LocalTranscriptionDownloader {
            preferences: TranscriptionPreferences {
                instruments: vec![],
                download_path: Some(temp_dir.path().to_str().unwrap().to_string()),
                download_path_alias: None,
            },
            link_store: Arc::new(parking_lot::RwLock::new(link_store)),
        };
        assert!(!downloader.is_transcription_asset_downloaded(Ustr::from("exercise")));
        downloader
            .download_transcription_asset(Ustr::from("exercise"), false)
            .unwrap();
        assert!(downloader.is_transcription_asset_downloaded(Ustr::from("exercise")));

        // The asset won't be redownloaded if it already exists.
        downloader
            .download_transcription_asset(Ustr::from("exercise"), false)
            .unwrap();
        assert!(downloader.is_transcription_asset_downloaded(Ustr::from("exercise")));

        // Verify re-downloading the asset as well.
        downloader
            .download_transcription_asset(Ustr::from("exercise"), true)
            .unwrap();
        assert!(downloader.is_transcription_asset_downloaded(Ustr::from("exercise")));
    }

    /// Verifies downloading an invalid asset.
    #[test]
    fn test_download_bad_asset() {
        let temp_dir = tempfile::tempdir().unwrap();
        let link_store = MockLinkStore {
            link: Some(TranscriptionLink::YouTube(
                "https://www.youtube.com/watch?v=badID".into(),
            )),
        };
        let downloader = LocalTranscriptionDownloader {
            preferences: TranscriptionPreferences {
                instruments: vec![],
                download_path: Some(temp_dir.path().to_str().unwrap().to_string()),
                download_path_alias: None,
            },
            link_store: Arc::new(parking_lot::RwLock::new(link_store)),
        };
        assert!(!downloader.is_transcription_asset_downloaded(Ustr::from("exercise")));
        assert!(downloader
            .download_transcription_asset(Ustr::from("exercise"), false)
            .is_err());
        assert!(!downloader.is_transcription_asset_downloaded(Ustr::from("exercise")));
    }

    /// Verifies that the download paths are correctly generated.
    #[test]
    fn test_download_paths() {
        let temp_dir = tempfile::tempdir().unwrap();
        let link_store = MockLinkStore {
            link: Some(TranscriptionLink::YouTube(YT_LINK.into())),
        };
        let downloader = LocalTranscriptionDownloader {
            preferences: TranscriptionPreferences {
                instruments: vec![],
                download_path: Some(temp_dir.path().to_str().unwrap().to_string()),
                download_path_alias: Some("C:/Users/username/Music".to_string()),
            },
            link_store: Arc::new(parking_lot::RwLock::new(link_store)),
        };

        let download_path = downloader
            .transcription_download_path(Ustr::from("exercise"))
            .unwrap();
        assert!(download_path.ends_with("audio.m4a"));
        assert!(download_path.starts_with(temp_dir.path()));
        assert_eq!(
            40,
            download_path
                .parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .len()
        );

        let alias_path = downloader
            .transcription_download_path_alias(Ustr::from("exercise"))
            .unwrap();
        assert!(alias_path.ends_with("audio.m4a"));
        assert!(alias_path.starts_with("C:/Users/username/Music"));
        assert_eq!(
            40,
            alias_path
                .parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .len()
        );
    }
}
