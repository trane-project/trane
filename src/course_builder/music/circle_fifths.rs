//! Contains utilities to generate courses based on the circles of fifths.
//!
//! Suppose that you have a course that contains guitar riffs that you would like to learn in
//! all keys. The utilities in this module allow you to define a course in which the first lesson
//! teaches you the riffs in the key of C (or A minor if the riffs were in a minor key). From this
//! lesson, there are two dependent lessons one for the key of G and another for the key of F,
//! because these keys contain only one sharp or flat respectively. The process repeats until the
//! circle is traversed in both clockwise and counter-clockwise directions.

use anyhow::Result;

use crate::{
    course_builder::{music::notes::*, AssetBuilder, CourseBuilder, LessonBuilder},
    data::{CourseManifest, LessonManifestBuilder},
};

impl Note {
    /// Returns the note obtained by moving clockwise through the circle of fifths.
    pub fn clockwise(&self) -> Option<Note> {
        match *self {
            Note::C => Some(Note::G),
            Note::G => Some(Note::D),
            Note::D => Some(Note::A),
            Note::A => Some(Note::E),
            Note::E => Some(Note::B),
            Note::B => Some(Note::F_SHARP),
            Note::F_SHARP => Some(Note::C_SHARP),
            Note::C_SHARP => None,

            Note::F => Some(Note::C),
            Note::B_FLAT => Some(Note::F),
            Note::E_FLAT => Some(Note::B_FLAT),
            Note::A_FLAT => Some(Note::E_FLAT),
            Note::D_FLAT => Some(Note::A_FLAT),
            Note::G_FLAT => Some(Note::D_FLAT),
            Note::C_FLAT => Some(Note::G_FLAT),
            _ => None,
        }
    }

    /// Returns the note obtained by moving counter-clockwise through the circle of fifths.
    pub fn counter_clockwise(&self) -> Option<Note> {
        match *self {
            Note::C => Some(Note::F),
            Note::F => Some(Note::B_FLAT),
            Note::B_FLAT => Some(Note::E_FLAT),
            Note::E_FLAT => Some(Note::A_FLAT),
            Note::A_FLAT => Some(Note::D_FLAT),
            Note::D_FLAT => Some(Note::G_FLAT),
            Note::G_FLAT => Some(Note::C_FLAT),
            Note::C_FLAT => None,

            Note::G => Some(Note::C),
            Note::D => Some(Note::G),
            Note::A => Some(Note::D),
            Note::E => Some(Note::A),
            Note::B => Some(Note::E),
            Note::F_SHARP => Some(Note::B),
            Note::C_SHARP => Some(Note::F_SHARP),
            _ => None,
        }
    }
}

/// Generates a course builder that contains a lesson per key and which follows the circle of
/// fifths, starting with the key of C (which has not flats nor sharps), and continuing in both
/// directions, allowing each lesson to depend on the lesson that comes before in the circle of
/// fifths. This is useful to create courses that teach the same exercises for each key.
pub struct CircleFifthsCourse {
    /// Base name of the directory on which to store this lesson.
    pub directory_name: String,

    /// The manifest for the course.
    pub course_manifest: CourseManifest,

    /// The asset builders for the course.
    pub course_asset_builders: Vec<AssetBuilder>,

    /// An optional closure that returns a different note from the one found by traversing the
    /// circle of fifths. This is useful, for example, to generate a course based on the minor scale
    /// in the correct order by the number of flats or sharps in the scale (i.e., the lesson based
    /// on A minor appears first because it's the relative minor of C major).
    pub note_alias: Option<fn(Note) -> Result<Note>>,

    /// The template used to generate the lesson manifests.
    pub lesson_manifest_template: LessonManifestBuilder,

    /// A closure which generates the builder for each lesson.
    pub lesson_builder_generator: Box<dyn Fn(Note, Option<Note>) -> Result<LessonBuilder>>,

    /// An optional closure which generates extra lessons which do not follow the circle of fifths
    /// pattern.
    pub extra_lessons_generator: Option<Box<dyn Fn() -> Result<Vec<LessonBuilder>>>>,
}

impl CircleFifthsCourse {
    /// Generates the lesson builder for the lesson based on the given note. An optional note is
    /// also provided for the purpose of creating the dependencies of the lesson.
    fn generate_lesson_builder(
        &self,
        note: Note,
        previous_note: Option<Note>,
    ) -> Result<LessonBuilder> {
        let note_alias = match &self.note_alias {
            None => note,
            Some(closure) => closure(note)?,
        };

        let previous_note_alias = match &self.note_alias {
            None => previous_note,
            Some(closure) => match previous_note {
                None => None,
                Some(previous_note) => Some(closure(previous_note)?),
            },
        };

        (self.lesson_builder_generator)(note_alias, previous_note_alias)
    }

    /// Traverses the circle of fifths in counter-clockwise motion and returns a list of the lesson
    /// builders generated along the way.
    fn generate_counter_clockwise(&self, note: Option<Note>) -> Result<Vec<LessonBuilder>> {
        if note.is_none() {
            return Ok(vec![]);
        }

        let note = note.unwrap();
        let mut lessons = vec![self.generate_lesson_builder(note, note.clockwise())?];
        lessons.extend(
            self.generate_counter_clockwise(note.counter_clockwise())?
                .into_iter(),
        );
        Ok(lessons)
    }

    /// Traverses the circle of fifths in clockwise motion and returns a list of the lesson builders
    /// generated along the way.
    fn generate_clockwise(&self, note: Option<Note>) -> Result<Vec<LessonBuilder>> {
        if note.is_none() {
            return Ok(vec![]);
        }

        let note = note.unwrap();
        let mut lessons = vec![self.generate_lesson_builder(note, note.counter_clockwise())?];
        lessons.extend(self.generate_clockwise(note.clockwise())?.into_iter());
        Ok(lessons)
    }

    /// Generates all the lesson builders for the course by starting at the note C and traversing
    /// the circle of fifths in both directions.
    fn generate_lesson_builders(&self) -> Result<Vec<LessonBuilder>> {
        let mut lessons = vec![self.generate_lesson_builder(Note::C, None)?];
        lessons.extend(
            self.generate_counter_clockwise(Note::C.counter_clockwise())?
                .into_iter(),
        );
        lessons.extend(self.generate_clockwise(Note::C.clockwise())?.into_iter());
        if let Some(generator) = &self.extra_lessons_generator {
            lessons.extend(generator()?.into_iter());
        }
        Ok(lessons)
    }

    /// Generates a course builder which contains lessons based on each key.
    pub fn generate_course_builder(&self) -> Result<CourseBuilder> {
        Ok(CourseBuilder {
            directory_name: self.directory_name.clone(),
            course_manifest: self.course_manifest.clone(),
            asset_builders: self.course_asset_builders.clone(),
            lesson_builders: self.generate_lesson_builders()?,
            lesson_manifest_template: self.lesson_manifest_template.clone(),
        })
    }
}
