//! FFI types for the data::filter module.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use typeshare::typeshare;
use ustr::Ustr;

use crate::data::filter;

// grcov-excl-start: The FFI types are not tested since the implementations of the `From` trait
// should be sufficient to ensure that the types are equivalent at compile time.

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum FilterOp {
    All,
    Any,
}

impl From<FilterOp> for filter::FilterOp {
    fn from(op: FilterOp) -> Self {
        match op {
            FilterOp::All => Self::All,
            FilterOp::Any => Self::Any,
        }
    }
}

impl From<filter::FilterOp> for FilterOp {
    fn from(op: filter::FilterOp) -> Self {
        match op {
            filter::FilterOp::All => Self::All,
            filter::FilterOp::Any => Self::Any,
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum FilterType {
    Include,
    Exclude,
}

impl From<FilterType> for filter::FilterType {
    fn from(ty: FilterType) -> Self {
        match ty {
            FilterType::Include => Self::Include,
            FilterType::Exclude => Self::Exclude,
        }
    }
}

impl From<filter::FilterType> for FilterType {
    fn from(ty: filter::FilterType) -> Self {
        match ty {
            filter::FilterType::Include => Self::Include,
            filter::FilterType::Exclude => Self::Exclude,
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "type", content = "content")]
pub enum KeyValueFilter {
    CourseFilter {
        key: String,
        value: String,
        filter_type: FilterType,
    },
    LessonFilter {
        key: String,
        value: String,
        filter_type: FilterType,
    },
    CombinedFilter {
        op: FilterOp,
        filters: Vec<KeyValueFilter>,
    },
}

impl From<KeyValueFilter> for filter::KeyValueFilter {
    fn from(filter: KeyValueFilter) -> Self {
        match filter {
            KeyValueFilter::CourseFilter {
                key,
                value,
                filter_type,
            } => Self::CourseFilter {
                key,
                value,
                filter_type: filter_type.into(),
            },
            KeyValueFilter::LessonFilter {
                key,
                value,
                filter_type,
            } => Self::LessonFilter {
                key,
                value,
                filter_type: filter_type.into(),
            },
            KeyValueFilter::CombinedFilter { op, filters } => Self::CombinedFilter {
                op: op.into(),
                filters: filters.into_iter().map(Into::into).collect(),
            },
        }
    }
}

impl From<filter::KeyValueFilter> for KeyValueFilter {
    fn from(filter: filter::KeyValueFilter) -> Self {
        match filter {
            filter::KeyValueFilter::CourseFilter {
                key,
                value,
                filter_type,
            } => Self::CourseFilter {
                key,
                value,
                filter_type: filter_type.into(),
            },
            filter::KeyValueFilter::LessonFilter {
                key,
                value,
                filter_type,
            } => Self::LessonFilter {
                key,
                value,
                filter_type: filter_type.into(),
            },
            filter::KeyValueFilter::CombinedFilter { op, filters } => Self::CombinedFilter {
                op: op.into(),
                filters: filters.into_iter().map(Into::into).collect(),
            },
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "type", content = "content")]
pub enum UnitFilter {
    CourseFilter {
        #[typeshare(serialized_as = "Vec<String>")]
        course_ids: Vec<Ustr>,
    },
    LessonFilter {
        #[typeshare(serialized_as = "Vec<String>")]
        lesson_ids: Vec<Ustr>,
    },
    MetadataFilter {
        filter: KeyValueFilter,
    },
    ReviewListFilter,
    Dependents {
        #[typeshare(serialized_as = "Vec<String>")]
        unit_ids: Vec<Ustr>,
    },
    Dependencies {
        #[typeshare(serialized_as = "Vec<String>")]
        unit_ids: Vec<Ustr>,
        #[typeshare(serialized_as = "u32")]
        depth: usize,
    },
}

impl From<UnitFilter> for filter::UnitFilter {
    fn from(filter: UnitFilter) -> Self {
        match filter {
            UnitFilter::CourseFilter { course_ids } => Self::CourseFilter {
                course_ids: course_ids.into_iter().map(Into::into).collect(),
            },
            UnitFilter::LessonFilter { lesson_ids } => Self::LessonFilter {
                lesson_ids: lesson_ids.into_iter().map(Into::into).collect(),
            },
            UnitFilter::MetadataFilter { filter } => Self::MetadataFilter {
                filter: filter.into(),
            },
            UnitFilter::ReviewListFilter => Self::ReviewListFilter,
            UnitFilter::Dependents { unit_ids } => Self::Dependents {
                unit_ids: unit_ids.into_iter().map(Into::into).collect(),
            },
            UnitFilter::Dependencies { unit_ids, depth } => Self::Dependencies {
                unit_ids: unit_ids.into_iter().map(Into::into).collect(),
                depth,
            },
        }
    }
}

impl From<filter::UnitFilter> for UnitFilter {
    fn from(filter: filter::UnitFilter) -> Self {
        match filter {
            filter::UnitFilter::CourseFilter { course_ids } => Self::CourseFilter {
                course_ids: course_ids.into_iter().map(Into::into).collect(),
            },
            filter::UnitFilter::LessonFilter { lesson_ids } => Self::LessonFilter {
                lesson_ids: lesson_ids.into_iter().map(Into::into).collect(),
            },
            filter::UnitFilter::MetadataFilter { filter } => Self::MetadataFilter {
                filter: filter.into(),
            },
            filter::UnitFilter::ReviewListFilter => Self::ReviewListFilter,
            filter::UnitFilter::Dependents { unit_ids } => Self::Dependents {
                unit_ids: unit_ids.into_iter().map(Into::into).collect(),
            },
            filter::UnitFilter::Dependencies { unit_ids, depth } => Self::Dependencies {
                unit_ids: unit_ids.into_iter().map(Into::into).collect(),
                depth,
            },
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SavedFilter {
    pub id: String,
    pub description: String,
    pub filter: UnitFilter,
}

impl From<SavedFilter> for filter::SavedFilter {
    fn from(filter: SavedFilter) -> Self {
        Self {
            id: filter.id,
            description: filter.description,
            filter: filter.filter.into(),
        }
    }
}

impl From<filter::SavedFilter> for SavedFilter {
    fn from(filter: filter::SavedFilter) -> Self {
        Self {
            id: filter.id,
            description: filter.description,
            filter: filter.filter.into(),
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "type", content = "content")]
pub enum SessionPart {
    UnitFilter { filter: UnitFilter, duration: u32 },
    SavedFilter { filter_id: String, duration: u32 },
    NoFilter { duration: u32 },
}

impl From<SessionPart> for filter::SessionPart {
    fn from(part: SessionPart) -> Self {
        match part {
            SessionPart::UnitFilter { filter, duration } => Self::UnitFilter {
                filter: filter.into(),
                duration,
            },
            SessionPart::SavedFilter {
                filter_id,
                duration,
            } => Self::SavedFilter {
                filter_id,
                duration,
            },
            SessionPart::NoFilter { duration } => Self::NoFilter { duration },
        }
    }
}

impl From<filter::SessionPart> for SessionPart {
    fn from(part: filter::SessionPart) -> Self {
        match part {
            filter::SessionPart::UnitFilter { filter, duration } => Self::UnitFilter {
                filter: filter.into(),
                duration,
            },
            filter::SessionPart::SavedFilter {
                filter_id,
                duration,
            } => Self::SavedFilter {
                filter_id,
                duration,
            },
            filter::SessionPart::NoFilter { duration } => Self::NoFilter { duration },
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StudySession {
    pub id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub parts: Vec<SessionPart>,
}

impl From<StudySession> for filter::StudySession {
    fn from(session: StudySession) -> Self {
        Self {
            id: session.id,
            description: session.description,
            parts: session.parts.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<filter::StudySession> for StudySession {
    fn from(session: filter::StudySession) -> Self {
        Self {
            id: session.id,
            description: session.description,
            parts: session.parts.into_iter().map(Into::into).collect(),
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StudySessionData {
    pub start_time: String,
    pub definition: StudySession,
}

impl From<StudySessionData> for filter::StudySessionData {
    fn from(session: StudySessionData) -> Self {
        Self {
            start_time: DateTime::parse_from_rfc3339(&session.start_time)
                .unwrap_or_else(|_| Utc::now().fixed_offset())
                .with_timezone(&Utc),
            definition: session.definition.into(),
        }
    }
}

impl From<filter::StudySessionData> for StudySessionData {
    fn from(session: filter::StudySessionData) -> Self {
        Self {
            start_time: session.start_time.to_rfc3339(),
            definition: session.definition.into(),
        }
    }
}

#[typeshare]
#[allow(missing_docs)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "type", content = "content")]
pub enum ExerciseFilter {
    UnitFilter(UnitFilter),
    StudySession(StudySessionData),
}

impl From<ExerciseFilter> for filter::ExerciseFilter {
    fn from(filter: ExerciseFilter) -> Self {
        match filter {
            ExerciseFilter::UnitFilter(filter) => Self::UnitFilter(filter.into()),
            ExerciseFilter::StudySession(session) => Self::StudySession(session.into()),
        }
    }
}

impl From<filter::ExerciseFilter> for ExerciseFilter {
    fn from(filter: filter::ExerciseFilter) -> Self {
        match filter {
            filter::ExerciseFilter::UnitFilter(filter) => Self::UnitFilter(filter.into()),
            filter::ExerciseFilter::StudySession(session) => Self::StudySession(session.into()),
        }
    }
}
