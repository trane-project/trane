//! FFI types for the `data::course_generator` module.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use typeshare::typeshare;
use ustr::Ustr;

use crate::data::course_generator;
use crate::data::course_generator::knowledge_base;
use crate::data::course_generator::music_piece;
use crate::data::course_generator::transcription;

// grcov-excl-start: The FFI types are not tested since the implementations of the `From` trait
// should be sufficient to ensure that the types are equivalent at compile time.

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Instrument {
    pub name: String,
    pub id: String,
}

impl From<Instrument> for course_generator::Instrument {
    fn from(instrument: Instrument) -> Self {
        Self {
            name: instrument.name,
            id: instrument.id,
        }
    }
}

impl From<course_generator::Instrument> for Instrument {
    fn from(instrument: course_generator::Instrument) -> Self {
        Self {
            name: instrument.name,
            id: instrument.id,
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct KnowledgeBaseConfig {}

impl From<KnowledgeBaseConfig> for knowledge_base::KnowledgeBaseConfig {
    fn from(_: KnowledgeBaseConfig) -> Self {
        Self {}
    }
}

impl From<knowledge_base::KnowledgeBaseConfig> for KnowledgeBaseConfig {
    fn from(_: knowledge_base::KnowledgeBaseConfig) -> Self {
        Self {}
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "content")]
pub enum MusicAsset {
    SoundSlice(String),
    LocalFile(String),
}

impl From<MusicAsset> for music_piece::MusicAsset {
    fn from(asset: MusicAsset) -> Self {
        match asset {
            MusicAsset::SoundSlice(id) => Self::SoundSlice(id),
            MusicAsset::LocalFile(path) => Self::LocalFile(path),
        }
    }
}

impl From<music_piece::MusicAsset> for MusicAsset {
    fn from(asset: music_piece::MusicAsset) -> Self {
        match asset {
            music_piece::MusicAsset::SoundSlice(id) => Self::SoundSlice(id),
            music_piece::MusicAsset::LocalFile(path) => Self::LocalFile(path),
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MusicPassage {
    pub start: String,
    pub end: String,
    #[typeshare(serialized_as = "HashMap<u32, MusicPassage>")]
    pub sub_passages: HashMap<usize, MusicPassage>,
}

impl From<MusicPassage> for music_piece::MusicPassage {
    fn from(passage: MusicPassage) -> Self {
        Self {
            start: passage.start,
            end: passage.end,
            sub_passages: passage
                .sub_passages
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
        }
    }
}

impl From<music_piece::MusicPassage> for MusicPassage {
    fn from(passage: music_piece::MusicPassage) -> Self {
        Self {
            start: passage.start,
            end: passage.end,
            sub_passages: passage
                .sub_passages
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MusicPieceConfig {
    pub music_asset: MusicAsset,
    pub passages: MusicPassage,
}

impl From<MusicPieceConfig> for music_piece::MusicPieceConfig {
    fn from(config: MusicPieceConfig) -> Self {
        Self {
            music_asset: config.music_asset.into(),
            passages: config.passages.into(),
        }
    }
}

impl From<music_piece::MusicPieceConfig> for MusicPieceConfig {
    fn from(config: music_piece::MusicPieceConfig) -> Self {
        Self {
            music_asset: config.music_asset.into(),
            passages: config.passages.into(),
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "content")]
pub enum TranscriptionLink {
    YouTube(String),
}

impl From<TranscriptionLink> for transcription::TranscriptionLink {
    fn from(link: TranscriptionLink) -> Self {
        match link {
            TranscriptionLink::YouTube(id) => Self::YouTube(id),
        }
    }
}

impl From<transcription::TranscriptionLink> for TranscriptionLink {
    fn from(link: transcription::TranscriptionLink) -> Self {
        match link {
            transcription::TranscriptionLink::YouTube(id) => Self::YouTube(id),
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "content")]
pub enum TranscriptionAsset {
    Track {
        short_id: String,
        track_name: String,
        #[serde(default)]
        artist_name: Option<String>,
        #[serde(default)]
        album_name: Option<String>,
        #[serde(default)]
        duration: Option<String>,
        #[serde(default)]
        external_link: Option<TranscriptionLink>,
    },
}

impl From<TranscriptionAsset> for transcription::TranscriptionAsset {
    fn from(asset: TranscriptionAsset) -> Self {
        match asset {
            TranscriptionAsset::Track {
                short_id,
                track_name,
                artist_name,
                album_name,
                duration,
                external_link,
            } => Self::Track {
                short_id,
                track_name,
                artist_name,
                album_name,
                duration,
                external_link: external_link.map(Into::into),
            },
        }
    }
}

impl From<transcription::TranscriptionAsset> for TranscriptionAsset {
    fn from(asset: transcription::TranscriptionAsset) -> Self {
        match asset {
            transcription::TranscriptionAsset::Track {
                short_id,
                track_name,
                artist_name,
                album_name,
                duration,
                external_link,
            } => Self::Track {
                short_id,
                track_name,
                artist_name,
                album_name,
                duration,
                external_link: external_link.map(Into::into),
            },
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TranscriptionPassages {
    pub asset: TranscriptionAsset,
    #[typeshare(serialized_as = "HashMap<u32, Vec<String>>")]
    pub intervals: HashMap<usize, (String, String)>,
}

impl From<TranscriptionPassages> for transcription::TranscriptionPassages {
    fn from(passages: TranscriptionPassages) -> Self {
        Self {
            asset: passages.asset.into(),
            intervals: passages.intervals,
        }
    }
}

impl From<transcription::TranscriptionPassages> for TranscriptionPassages {
    fn from(passages: transcription::TranscriptionPassages) -> Self {
        Self {
            asset: passages.asset.into(),
            intervals: passages.intervals,
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct TranscriptionPreferences {
    #[serde(default)]
    pub instruments: Vec<Instrument>,
}

impl From<TranscriptionPreferences> for transcription::TranscriptionPreferences {
    fn from(preferences: TranscriptionPreferences) -> Self {
        Self {
            instruments: preferences
                .instruments
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<transcription::TranscriptionPreferences> for TranscriptionPreferences {
    fn from(preferences: transcription::TranscriptionPreferences) -> Self {
        Self {
            instruments: preferences
                .instruments
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TranscriptionConfig {
    #[serde(default)]
    #[typeshare(serialized_as = "Vec<String>")]
    pub transcription_dependencies: Vec<Ustr>,
    #[serde(default)]
    pub passage_directory: String,
    #[serde(default)]
    pub inlined_passages: Vec<TranscriptionPassages>,
    #[serde(default)]
    pub skip_singing_lessons: bool,
    #[serde(default)]
    pub skip_advanced_lessons: bool,
}

impl From<TranscriptionConfig> for transcription::TranscriptionConfig {
    fn from(config: TranscriptionConfig) -> Self {
        Self {
            transcription_dependencies: config.transcription_dependencies,
            passage_directory: config.passage_directory,
            inlined_passages: config
                .inlined_passages
                .into_iter()
                .map(Into::into)
                .collect(),
            skip_singing_lessons: config.skip_singing_lessons,
            skip_advanced_lessons: config.skip_advanced_lessons,
        }
    }
}

impl From<transcription::TranscriptionConfig> for TranscriptionConfig {
    fn from(config: transcription::TranscriptionConfig) -> Self {
        Self {
            transcription_dependencies: config.transcription_dependencies,
            passage_directory: config.passage_directory,
            inlined_passages: config
                .inlined_passages
                .into_iter()
                .map(Into::into)
                .collect(),
            skip_singing_lessons: config.skip_singing_lessons,
            skip_advanced_lessons: config.skip_advanced_lessons,
        }
    }
}
