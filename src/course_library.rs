//! Defines the operations that can be performed on a collection of courses stored by the student.
//!
//! A course library (the term Trane library will be used interchangeably) is a collection of
//! courses that the student wishes to practice together. Courses, lessons, and exercises are
//! defined by their manifest files (see data.rs).

use std::{fs::File, io::BufReader, path::Path, sync::Arc};

use anyhow::{anyhow, ensure, Result};
use parking_lot::RwLock;
use serde::de::DeserializeOwned;
use ustr::{Ustr, UstrMap};
use walkdir::{DirEntry, WalkDir};

use crate::{
    data::{CourseManifest, ExerciseManifest, LessonManifest, NormalizePaths, UnitType},
    graph::{InMemoryUnitGraph, UnitGraph},
};

/// The file name for all course manifests.
const COURSE_MANIFEST_FILENAME: &str = "course_manifest.json";

/// The file name for all lesson manifests.
const LESSON_MANIFEST_FILENAME: &str = "lesson_manifest.json";

/// The file name for all exercise manifests.
const EXERCISE_MANIFEST_FILENAME: &str = "exercise_manifest.json";

/// A trait that manages a course library, its corresponding manifest files, and provides basic
/// operations to retrieve the courses, lessons in a course, and exercises in a lesson.
pub trait CourseLibrary {
    /// Returns the course manifest for the given course.
    fn get_course_manifest(&self, course_id: &Ustr) -> Option<CourseManifest>;

    /// Returns the lesson manifest for the given lesson.
    fn get_lesson_manifest(&self, lesson_id: &Ustr) -> Option<LessonManifest>;

    /// Returns the exercise manifest for the given exercise.
    fn get_exercise_manifest(&self, exercise_id: &Ustr) -> Option<ExerciseManifest>;

    /// Returns the IDs of all courses in the library sorted alphabetically.
    fn get_course_ids(&self) -> Vec<Ustr>;

    /// Returns the IDs of all lessons in the given course sorted alphabetically.
    fn get_lesson_ids(&self, course_id: &Ustr) -> Result<Vec<Ustr>>;

    /// Returns the IDs of all exercises in the given lesson sorted alphabetically.
    fn get_exercise_ids(&self, lesson_id: &Ustr) -> Result<Vec<Ustr>>;
}

/// A trait that retrieves the unit graph generated after reading a course library. The visibility
/// is set to `pub(crate)` because `InMemoryUnitGraph` has the same visibility and returning a
/// concrete type avoids the need for indirection.
pub(crate) trait GetUnitGraph {
    /// Returns a reference to the unit graph describing the dependencies among the courses and
    /// lessons in this library.
    fn get_unit_graph(&self) -> Arc<RwLock<InMemoryUnitGraph>>;
}

/// An implementation of `CourseLibrary` backed by the local file system. The courses in this
/// library are those directories located anywhere under the given root directory that match the
/// following structure:
///
/// ```text
/// course-manifest.json
/// <LESSON_DIR_1>/
///     lesson-manifest.json
///    <EXERCISE_DIR_1>/
///       exercise-manifest.json
///   <EXERCISE_DIR_2>/
///      exercise-manifest.json
///    ...
/// <LESSON_DIR_2>/
///    lesson-manifest.json
///   <EXERCISE_DIR_1>/
///     exercise-manifest.json
///   ...
/// ```
///
/// The directory can also contain asset files referenced by the manifests. For example, a basic
/// flashcard with a front and back can be stored using two markdown files.
pub(crate) struct LocalCourseLibrary {
    /// A `UnitGraph` constructed when opening the library.
    unit_graph: Arc<RwLock<InMemoryUnitGraph>>,

    /// A mapping of course ID to its corresponding course manifest.
    course_map: UstrMap<CourseManifest>,

    /// A mapping of lesson ID to its corresponding lesson manifest.
    lesson_map: UstrMap<LessonManifest>,

    /// A mapping of exercise ID to its corresponding exercise manifest.
    exercise_map: UstrMap<ExerciseManifest>,
}

impl LocalCourseLibrary {
    /// Opens the course, lesson, or exercise manifest located at the given path.
    fn open_manifest<T: DeserializeOwned>(path: &str) -> Result<T> {
        let file = File::open(path).map_err(|_| anyhow!("cannot open manifest file {}", path))?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader).map_err(|_| anyhow!("cannot parse manifest file {}", path))
    }

    /// Returns the full string representation of the given path.
    fn get_full_path(path: &Path) -> Result<String> {
        Ok(path
            .to_str()
            .ok_or_else(|| anyhow!("invalid dir entry {}", path.display()))?
            .to_string())
    }

    /// Returns the file name of the given path.
    fn get_file_name(path: &Path) -> Result<String> {
        Ok(path
            .file_name()
            .ok_or_else(|| anyhow!("cannot get file name from DirEntry"))? // grcov-excl-line
            .to_str()
            .ok_or_else(|| anyhow!("invalid dir entry {}", path.display()))?
            .to_string())
    }

    /// Adds an exercise to the course library given its manifest and the manifest of the lesson to
    /// which it belongs.
    fn process_exercise_manifest(
        &mut self,
        lesson_manifest: &LessonManifest,
        exercise_manifest: ExerciseManifest,
    ) -> Result<()> {
        ensure!(!exercise_manifest.id.is_empty(), "ID in manifest is empty",);
        ensure!(
            exercise_manifest.lesson_id == lesson_manifest.id,
            "lesson_id in manifest for exercise {} does not match the manifest for lesson {}",
            exercise_manifest.id,
            lesson_manifest.id,
        );
        ensure!(
            exercise_manifest.course_id == lesson_manifest.course_id,
            "course_id in manifest for exercise {} does not match the manifest for course {}",
            exercise_manifest.id,
            lesson_manifest.course_id,
        );

        self.unit_graph
            .write()
            .add_exercise(&exercise_manifest.id, &exercise_manifest.lesson_id)?;
        self.exercise_map
            .insert(exercise_manifest.id, exercise_manifest);
        Ok(())
    }

    /// Adds a lesson to the course library given its manifest and the manifest of the course to
    /// which it belongs. It also traverses the given `DirEntry` and adds all the exercises in the
    /// lesson.
    fn process_lesson_manifest(
        &mut self,
        dir_entry: &DirEntry,
        course_manifest: &CourseManifest,
        lesson_manifest: LessonManifest,
    ) -> Result<()> {
        ensure!(!lesson_manifest.id.is_empty(), "ID in manifest is empty",);
        ensure!(
            lesson_manifest.course_id == course_manifest.id,
            "course_id in manifest for lesson {} does not match the manifest for course {}",
            lesson_manifest.id,
            course_manifest.id,
        );

        let lesson_root = dir_entry
            .path()
            .parent()
            .ok_or_else(|| anyhow!("cannot get lesson's parent directory"))?; // grcov-excl-line

        // Add the lesson and the dependencies explicitly listed in the lesson manifest.
        self.unit_graph
            .write()
            .add_lesson(&lesson_manifest.id, &lesson_manifest.course_id)?;
        self.unit_graph.write().add_dependencies(
            &lesson_manifest.id,
            UnitType::Lesson,
            &lesson_manifest.dependencies,
        )?;
        self.lesson_map
            .insert(lesson_manifest.id, lesson_manifest.clone());

        // Start a new search from the passed `DirEntry`, which corresponds to the lesson's root.
        // Each exercise in the lesson must be contained in a directory that is a direct descendant
        // of its root. Therefore, all the exercise manifests will be found at a depth of two.
        for entry in WalkDir::new(lesson_root).min_depth(2).max_depth(2) {
            match entry {
                Err(_) => continue,
                Ok(exercise_dir_entry) => {
                    if exercise_dir_entry.path().is_dir() {
                        continue;
                    }

                    let file_name = Self::get_file_name(exercise_dir_entry.path())?;
                    if file_name != EXERCISE_MANIFEST_FILENAME {
                        continue;
                    }

                    let path = Self::get_full_path(exercise_dir_entry.path())?;
                    let mut exercise_manifest: ExerciseManifest = Self::open_manifest(&path)?;
                    exercise_manifest = exercise_manifest
                        .normalize_paths(exercise_dir_entry.path().parent().unwrap())?;
                    self.process_exercise_manifest(&lesson_manifest, exercise_manifest)?;
                }
            }
        }
        Ok(())
    }

    /// Adds a course to the course library given its manifest. It also traverses the given
    /// `DirEntry` and adds all the lessons in the course.
    fn process_course_manifest(
        &mut self,
        dir_entry: &DirEntry,
        course_manifest: CourseManifest,
    ) -> Result<()> {
        ensure!(!course_manifest.id.is_empty(), "ID in manifest is empty",);

        // Add the course and the dependencies explicitly listed in the manifest.
        self.unit_graph.write().add_course(&course_manifest.id)?;
        self.unit_graph.write().add_dependencies(
            &course_manifest.id,
            UnitType::Course,
            &course_manifest.dependencies,
        )?;
        self.course_map
            .insert(course_manifest.id, course_manifest.clone());

        // Start a new search from the passed `DirEntry`, which corresponds to the course's root.
        // Each lesson in the course must be contained in a directory that is a direct descendant of
        // its root. Therefore, all the lesson manifests will be found at a depth of two.
        let course_root = dir_entry.path().parent().unwrap();
        for entry in WalkDir::new(course_root).min_depth(2).max_depth(2) {
            match entry {
                Err(_) => continue,
                Ok(lesson_dir_entry) => {
                    if lesson_dir_entry.path().is_dir() {
                        continue;
                    }

                    let file_name = Self::get_file_name(lesson_dir_entry.path())?;
                    if file_name != LESSON_MANIFEST_FILENAME {
                        continue;
                    }

                    let path = Self::get_full_path(lesson_dir_entry.path())?;
                    let mut lesson_manifest: LessonManifest = Self::open_manifest(&path)?;
                    lesson_manifest = lesson_manifest
                        .normalize_paths(lesson_dir_entry.path().parent().unwrap())?;
                    self.process_lesson_manifest(
                        &lesson_dir_entry,
                        &course_manifest,
                        lesson_manifest,
                    )?; // grcov-excl-line
                }
            }
        }
        Ok(())
    }

    /// A constructor taking the path to the root of the library.
    pub fn new(library_root: &Path) -> Result<Self> {
        if !library_root.is_dir() {
            return Err(anyhow!(
                "{:#?} must be the path to a directory",
                library_root
            ));
        }
        let mut library = LocalCourseLibrary {
            course_map: UstrMap::default(),
            lesson_map: UstrMap::default(),
            exercise_map: UstrMap::default(),
            unit_graph: Arc::new(RwLock::new(InMemoryUnitGraph::default())),
        };

        // Start a search from the library root. Courses can be located at any level within the
        // library root. However, the lessons and exercises inside each course follow a fixed
        // structure.
        for entry in WalkDir::new(library_root).min_depth(2) {
            match entry {
                Err(_) => continue,
                Ok(dir_entry) => {
                    if dir_entry.path().is_dir() {
                        continue;
                    }

                    let file_name = Self::get_file_name(dir_entry.path())?;
                    if file_name != COURSE_MANIFEST_FILENAME {
                        continue;
                    }

                    let path = Self::get_full_path(dir_entry.path())?;
                    let mut course_manifest: CourseManifest = Self::open_manifest(&path)?;
                    course_manifest =
                        course_manifest.normalize_paths(dir_entry.path().parent().unwrap())?;
                    library.process_course_manifest(&dir_entry, course_manifest)?;
                }
            }
        }

        // Lessons implicitly depend on the course to which they belong. Calling
        // `update_starting_lessons` computes the lessons in a course not dependent on any other
        // lesson in the course. This allows the scheduler to traverse the lessons in the correct
        // order.
        library.unit_graph.write().update_starting_lessons();
        // Perform a check to detect cyclic dependencies, which could cause infinite loops during
        // traversal.
        library.unit_graph.read().check_cycles()?;
        Ok(library)
    }
}

impl CourseLibrary for LocalCourseLibrary {
    fn get_course_manifest(&self, course_id: &Ustr) -> Option<CourseManifest> {
        self.course_map.get(course_id).cloned()
    }

    fn get_lesson_manifest(&self, lesson_id: &Ustr) -> Option<LessonManifest> {
        self.lesson_map.get(lesson_id).cloned()
    }

    fn get_exercise_manifest(&self, exercise_id: &Ustr) -> Option<ExerciseManifest> {
        self.exercise_map.get(exercise_id).cloned()
    }

    fn get_course_ids(&self) -> Vec<Ustr> {
        let mut courses = self.course_map.keys().cloned().collect::<Vec<Ustr>>();
        courses.sort();
        courses
    }

    fn get_lesson_ids(&self, course_id: &Ustr) -> Result<Vec<Ustr>> {
        let mut lessons = self
            .unit_graph
            .read()
            .get_course_lessons(course_id)
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<Ustr>>();
        lessons.sort();
        Ok(lessons)
    }

    fn get_exercise_ids(&self, lesson_id: &Ustr) -> Result<Vec<Ustr>> {
        let mut exercises = self
            .unit_graph
            .read()
            .get_lesson_exercises(lesson_id)
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<Ustr>>();
        exercises.sort();
        Ok(exercises)
    }
}

impl GetUnitGraph for LocalCourseLibrary {
    fn get_unit_graph(&self) -> Arc<RwLock<InMemoryUnitGraph>> {
        self.unit_graph.clone()
    }
}

#[cfg(test)]
mod test {
    use std::path::Path;

    use crate::course_library::LocalCourseLibrary;

    #[test]
    fn path_is_not_dir() {
        let path = Path::new("foo");
        let result = LocalCourseLibrary::new(path);
        assert!(result.is_err());
    }
}