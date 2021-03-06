//! Module containing utilities to open and manipulate collecti&ons of courses and lessons stored
//! under a directory, which are named a course library.
use std::{fs::File, io::BufReader, path::Path, sync::Arc};

use anyhow::{anyhow, ensure, Result};
use parking_lot::RwLock;
use serde::de::DeserializeOwned;
use ustr::{Ustr, UstrMap, UstrSet};
use walkdir::{DirEntry, WalkDir};

use crate::{
    data::{CourseManifest, ExerciseManifest, LessonManifest, NormalizePaths, UnitType},
    graph::{InMemoryUnitGraph, UnitGraph},
};

/// Manages a course library and its corresponding course, lessons, and exercise manifests.
pub trait CourseLibrary {
    /// Returns the manifest for the given course.
    fn get_course_manifest(&self, course_id: &Ustr) -> Option<CourseManifest>;

    /// Returns the manifest for the given lesson.
    fn get_lesson_manifest(&self, lesson_id: &Ustr) -> Option<LessonManifest>;

    /// Returns the manfifest for the given exercise.
    fn get_exercise_manifest(&self, exercise_id: &Ustr) -> Option<ExerciseManifest>;

    /// Returns the IDs of all courses in the library.
    fn get_course_ids(&self) -> Vec<Ustr>;

    /// Returns the IDs of all lessons in the given course.
    fn get_lesson_ids(&self, course_id: &Ustr) -> Result<Vec<Ustr>>;

    /// Returns the IDs of all exercises in the given lesson.
    fn get_exercise_ids(&self, lesson_id: &Ustr) -> Result<Vec<Ustr>>;
}

pub(crate) trait GetUnitGraph {
    /// Returns a reference to the unit graph describing the dependencies among the courses and
    /// lessons in this library.
    fn get_unit_graph(&self) -> Arc<RwLock<InMemoryUnitGraph>>;
}

/// An implementation of CourseLibrary backed by the local filesystem.
pub(crate) struct LocalCourseLibrary {
    /// A dependency graph of the course and lessons in the library.
    unit_graph: Arc<RwLock<InMemoryUnitGraph>>,

    /// A map of course ID to the path of its manifest.
    course_map: UstrMap<CourseManifest>,

    /// A map of lesson ID to the path of its manifest.
    lesson_map: UstrMap<LessonManifest>,

    /// A map of exercise ID to the path its manifest.
    exercise_map: UstrMap<ExerciseManifest>,
}

impl LocalCourseLibrary {
    /// Opens the manifest located at the given path.
    fn open_manifest<T: DeserializeOwned>(path: &str) -> Result<T> {
        let file = File::open(path).map_err(|_| anyhow!("cannot open manifest file {}", path))?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader).map_err(|_| anyhow!("cannot parse manifest file {}", path))
    }

    /// Processes the exercise manifest located at the given DirEntry.
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

    /// Processes the lesson manifest located at the given DirEntry.
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
            .ok_or_else(|| anyhow!("cannot get lesson's parent directory"))?;

        // Start a new search from the lesson's root. Each exercise in the lesson must be contained
        // in a directory that is a direct descendent of the root. Therefore, all the exercise
        // manifests will be at a depth of two from the root.
        for entry in WalkDir::new(lesson_root).min_depth(2).max_depth(2) {
            match entry {
                Err(_) => continue,
                Ok(exercise_dir_entry) => {
                    let path = exercise_dir_entry.path().to_str().ok_or_else(|| {
                        anyhow!("invalid dir entry {}", exercise_dir_entry.path().display())
                    })?;
                    if !path.ends_with("exercise_manifest.json") {
                        continue;
                    }

                    let mut exercise_manifest: ExerciseManifest = Self::open_manifest(path)?;
                    exercise_manifest = exercise_manifest
                        .normalize_paths(exercise_dir_entry.path().parent().unwrap())?;
                    self.process_exercise_manifest(&lesson_manifest, exercise_manifest)?;
                }
            }
        }

        self.unit_graph
            .write()
            .add_lesson(&lesson_manifest.id, &lesson_manifest.course_id)?;

        // Add the dependencies explicitly stated by the manifest.
        self.unit_graph.write().add_dependencies(
            &lesson_manifest.id,
            UnitType::Lesson,
            &lesson_manifest.dependencies,
        )?;

        self.lesson_map.insert(lesson_manifest.id, lesson_manifest);
        Ok(())
    }

    // Mark the first lesons in the course (those which do not depend on other lessons in the same
    // course and would be traversed first) as depending on the entire course. This is done so that
    // the scheduler can add a course's lessons in the correct order.
    fn add_implicit_dependencies(&mut self, course_id: Ustr, lesson_ids: UstrSet) -> Result<()> {
        let first_lessons: Vec<Ustr> = lesson_ids
            .iter()
            .filter_map(|lesson_id| {
                let dependencies = self.unit_graph.read().get_dependencies(lesson_id);
                match dependencies {
                    None => Some(*lesson_id),
                    Some(deps) => {
                        if lesson_ids.is_disjoint(&deps) {
                            Some(*lesson_id)
                        } else {
                            None
                        }
                    }
                }
            })
            .collect::<Vec<Ustr>>();

        for lesson_id in first_lessons {
            self.unit_graph
                .write()
                .add_dependencies(&lesson_id, UnitType::Lesson, &[course_id])?;
        }
        Ok(())
    }

    /// Processes the course manifest located at the given DirEntry.
    fn process_course_manifest(
        &mut self,
        dir_entry: &DirEntry,
        course_manifest: CourseManifest,
    ) -> Result<()> {
        ensure!(!course_manifest.id.is_empty(), "ID in manifest is empty",);

        // Start a new search from the course's root. Each lesson in the course must be contained in
        // a directory that is a direct descendent of the root. Therefore, all the lesson manifests
        // will be at a depth of two from the root.
        let mut lesson_ids = UstrSet::default();
        let course_root = dir_entry.path().parent().unwrap();
        for entry in WalkDir::new(course_root).min_depth(2).max_depth(2) {
            match entry {
                Err(_) => continue,
                Ok(lesson_dir_entry) => {
                    let path = lesson_dir_entry.path().to_str().ok_or_else(|| {
                        anyhow!("invalid dir entry {}", dir_entry.path().display())
                    })?;
                    if !path.ends_with("lesson_manifest.json") {
                        continue;
                    }

                    let mut lesson_manifest: LessonManifest = Self::open_manifest(path)?;
                    lesson_manifest = lesson_manifest
                        .normalize_paths(lesson_dir_entry.path().parent().unwrap())?;
                    let lesson_id = lesson_manifest.id;
                    self.process_lesson_manifest(
                        &lesson_dir_entry,
                        &course_manifest,
                        lesson_manifest,
                    )?;

                    // Gather all of the IDs of the lessons in this course to build the implicit
                    // dependencies between the course and its lessons.
                    lesson_ids.insert(lesson_id);
                }
            }
        }

        self.unit_graph.write().add_dependencies(
            &course_manifest.id,
            UnitType::Course,
            &course_manifest.dependencies,
        )?;

        let course_id = course_manifest.id;
        self.course_map.insert(course_manifest.id, course_manifest);
        self.add_implicit_dependencies(course_id, lesson_ids)?;

        Ok(())
    }

    /// A constructor taking the path to the root of the library.
    pub fn new(library_root: &str) -> Result<Self> {
        let root_path = Path::new(library_root);
        if !root_path.is_dir() {
            return Err(anyhow!("{} must be the path to a directory", library_root));
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
                    let path = dir_entry.path().to_str().ok_or_else(|| {
                        anyhow!("invalid dir entry {}", dir_entry.path().display())
                    })?;
                    if !path.ends_with("course_manifest.json") {
                        continue;
                    }

                    let mut course_manifest: CourseManifest = Self::open_manifest(path)?;
                    course_manifest =
                        course_manifest.normalize_paths(dir_entry.path().parent().unwrap())?;
                    library.process_course_manifest(&dir_entry, course_manifest)?;
                }
            }
        }

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
