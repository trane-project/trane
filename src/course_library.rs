//! Defines the operations that can be performed on a collection of courses stored by the student.
//!
//! A course library (the term Trane library will be used interchangeably) is a collection of
//! courses that the student wishes to practice together. Courses, lessons, and exercises are
//! defined by their manifest files (see [data](crate::data)).

use anyhow::{Context, Result, anyhow, ensure};
use bincode::{Decode, Encode};
use parking_lot::RwLock;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::de::DeserializeOwned;
use std::{
    fs::File,
    io::BufReader,
    path::{self, Path, PathBuf},
    sync::Arc,
};
use ustr::{Ustr, UstrMap, UstrSet};
use walkdir::WalkDir;

use crate::{
    data::{
        CourseManifest, ExerciseManifest, GenerateManifests, LessonManifest, NormalizePaths,
        UnitType, UserPreferences,
    },
    graph::{InMemoryUnitGraph, UnitGraph},
};

/// The file name for all course manifests.
pub const COURSE_MANIFEST_FILENAME: &str = "course_manifest.json";

/// The file name for all lesson manifests.
pub const LESSON_MANIFEST_FILENAME: &str = "lesson_manifest.json";

/// The file name for all exercise manifests.
pub const EXERCISE_MANIFEST_FILENAME: &str = "exercise_manifest.json";

/// A trait that manages a course library, its corresponding manifest files, and provides basic
/// operations to retrieve the courses, lessons in a course, and exercises in a lesson.
pub trait CourseLibrary {
    /// Returns the course manifest for the given course.
    fn get_course_manifest(&self, course_id: Ustr) -> Option<Arc<CourseManifest>>;

    /// Returns the lesson manifest for the given lesson.
    fn get_lesson_manifest(&self, lesson_id: Ustr) -> Option<Arc<LessonManifest>>;

    /// Returns the exercise manifest for the given exercise.
    fn get_exercise_manifest(&self, exercise_id: Ustr) -> Option<Arc<ExerciseManifest>>;

    /// Returns the IDs of all courses in the library sorted alphabetically.
    fn get_course_ids(&self) -> Vec<Ustr>;

    /// Returns the IDs of all lessons in the given course sorted alphabetically.
    fn get_lesson_ids(&self, course_id: Ustr) -> Option<Vec<Ustr>>;

    /// Returns the IDs of all exercises in the given lesson sorted alphabetically.
    fn get_exercise_ids(&self, lesson_id: Ustr) -> Option<Vec<Ustr>>;

    /// Returns the IDs of all exercises in the given course sorted alphabetically.
    fn get_all_exercise_ids(&self, unit_id: Option<Ustr>) -> Vec<Ustr>;

    /// Returns the set of units whose ID starts with the given prefix and are of the given type.
    /// If `unit_type` is `None`, then all unit types are considered.
    fn get_matching_prefix(&self, prefix: &str, unit_type: Option<UnitType>) -> UstrSet;
}

/// A trait that retrieves the unit graph generated after reading a course library.
pub(crate) trait GetUnitGraph {
    /// Returns a reference to the in-memory unit graph describing the dependencies among the
    /// courses and lessons in this library.
    fn get_unit_graph(&self) -> Arc<RwLock<InMemoryUnitGraph>>;
}

/// A version of a course library that can be serialized and deserialized. Useful to embed course
/// libraries in binaries. It uses bincode for fast serialization and deserialization.
#[derive(Clone, Debug, Decode, Encode, PartialEq)]
pub struct SerializedCourseLibrary {
    /// The graph of units and dependencies.
    #[bincode(with_serde)]
    unit_graph: InMemoryUnitGraph,

    #[bincode(with_serde)]
    /// A mapping of course ID to its corresponding course manifest.
    course_map: UstrMap<CourseManifest>,

    #[bincode(with_serde)]
    /// A mapping of lesson ID to its corresponding lesson manifest.
    lesson_map: UstrMap<LessonManifest>,

    #[bincode(with_serde)]
    /// A mapping of exercise ID to its corresponding exercise manifest.
    exercise_map: UstrMap<ExerciseManifest>,
}

impl From<&LocalCourseLibrary> for SerializedCourseLibrary {
    /// Converts a `LocalCourseLibrary` into a `SerializedCourseLibrary`.
    fn from(library: &LocalCourseLibrary) -> Self {
        SerializedCourseLibrary {
            unit_graph: (*library.unit_graph.read()).clone(),
            course_map: library
                .course_map
                .iter()
                .map(|(k, v)| (*k, (**v).clone()))
                .collect(),
            lesson_map: library
                .lesson_map
                .iter()
                .map(|(k, v)| (*k, (**v).clone()))
                .collect(),
            exercise_map: library
                .exercise_map
                .iter()
                .map(|(k, v)| (*k, (**v).clone()))
                .collect(),
        }
    }
}

/// A request to open a single course in the library. This is used to allow parallel processing when
/// opening a library.
struct OpenCourseRequest {
    /// The path to the course root directory.
    course_root: PathBuf,

    /// The course manifest.
    course_manifest: CourseManifest,
}

/// The result of opening a single course in the library. Used to allow parallel processing when
/// opening a library.
struct OpenCourseResult {
    /// The course manifest.
    manifest: CourseManifest,

    /// The lessons in the course, consisting of the lesson manifest and a list of exercise
    /// manifests.
    lessons: Vec<(LessonManifest, Vec<ExerciseManifest>)>,
}

/// An implementation of [`CourseLibrary`] backed by the local file system. The courses in this
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
pub struct LocalCourseLibrary {
    /// The graph of units and dependencies.
    pub unit_graph: Arc<RwLock<InMemoryUnitGraph>>,

    /// A mapping of course ID to its corresponding course manifest.
    pub course_map: UstrMap<Arc<CourseManifest>>,

    /// A mapping of lesson ID to its corresponding lesson manifest.
    pub lesson_map: UstrMap<Arc<LessonManifest>>,

    /// A mapping of exercise ID to its corresponding exercise manifest.
    pub exercise_map: UstrMap<Arc<ExerciseManifest>>,

    /// The user preferences.
    pub user_preferences: UserPreferences,
}

impl LocalCourseLibrary {
    /// Opens the course, lesson, or exercise manifest located at the given path.
    fn open_manifest<T: DeserializeOwned>(path: &Path) -> Result<T> {
        let display = path.display();
        let file = File::open(path).context(format!("cannot open manifest file {display}"))?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader).context(format!("cannot parse manifest file {display}"))
    }

    /// Returns the file name of the given path.
    fn get_file_name(path: &Path) -> Result<String> {
        Ok(path
            .file_name()
            .ok_or(anyhow!("cannot get file name from DirEntry"))?
            .to_str()
            .ok_or(anyhow!("invalid dir entry {}", path.display()))?
            .to_string())
    }

    // Verifies that the IDs mentioned in the exercise manifest and its lesson manifest are valid
    // and agree with each other.
    #[cfg_attr(coverage, coverage(off))]
    fn verify_exercise_manifest(
        lesson_manifest: &LessonManifest,
        exercise_manifest: &ExerciseManifest,
    ) -> Result<()> {
        ensure!(!exercise_manifest.id.is_empty(), "ID in manifest is empty");
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
        Ok(())
    }

    /// Verifes that the IDs mentioned in the lesson manifest and its course manifestsare valid and
    /// agree with each other.
    #[cfg_attr(coverage, coverage(off))]
    fn verify_lesson_manifest(
        course_manifest: &CourseManifest,
        lesson_manifest: &LessonManifest,
    ) -> Result<()> {
        // Verify that the IDs mentioned in the manifests are valid and agree with each other.
        ensure!(!lesson_manifest.id.is_empty(), "ID in manifest is empty",);
        ensure!(
            lesson_manifest.course_id == course_manifest.id,
            "course_id in manifest for lesson {} does not match the manifest for course {}",
            lesson_manifest.id,
            course_manifest.id,
        );
        Ok(())
    }

    /// Adds a lesson to the course library given its manifest and the manifest of the course to
    /// which it belongs. It also traverses the given `DirEntry` and adds all the exercises in the
    /// lesson.
    fn process_lesson_manifest(
        lesson_root: &Path,
        course_manifest: &CourseManifest,
        lesson_manifest: LessonManifest,
    ) -> Result<(LessonManifest, Vec<ExerciseManifest>)> {
        // Verify the manifest and create a vector for the exercises.
        LocalCourseLibrary::verify_lesson_manifest(course_manifest, &lesson_manifest)?;
        let mut exercises = Vec::new();

        // Start a new search from the parent of the passed `DirEntry`, which corresponds to the
        // lesson's root. Each exercise in the lesson must be contained in a directory that is a
        // direct descendant of the lesson's root. Therefore, all the exercise manifests will be
        // found at a depth of two.
        for entry in WalkDir::new(lesson_root)
            .min_depth(2)
            .max_depth(2)
            .into_iter()
            .flatten()
        {
            // Ignore any entries that are not files named `exercise_manifest.json`.
            if entry.path().is_dir() {
                continue; // grcov-excl-line
            }
            let file_name = Self::get_file_name(entry.path())?;
            if file_name != EXERCISE_MANIFEST_FILENAME {
                continue;
            }

            // Open the exercise manifest and process it.
            let mut exercise_manifest: ExerciseManifest = Self::open_manifest(entry.path())?;
            exercise_manifest =
                exercise_manifest.normalize_paths(entry.path().parent().unwrap())?;
            LocalCourseLibrary::verify_exercise_manifest(&lesson_manifest, &exercise_manifest)?;
            exercises.push(exercise_manifest);
        }

        Ok((lesson_manifest, exercises))
    }

    /// Verifies that the IDs mentioned in the course manifest are valid.
    #[cfg_attr(coverage, coverage(off))]
    fn verify_course_manifest(course_manifest: &CourseManifest) -> Result<()> {
        ensure!(!course_manifest.id.is_empty(), "ID in manifest is empty",);
        Ok(())
    }

    /// Adds a course to the course library given its manifest. It also traverses the given
    /// `DirEntry` and adds all the lessons in the course.
    fn process_course_manifest(
        &self,
        course_root: &Path,
        mut course_manifest: CourseManifest,
    ) -> Result<OpenCourseResult> {
        // Verify the manifest and create a vector for the lessons.
        LocalCourseLibrary::verify_course_manifest(&course_manifest)?;
        let mut lessons = Vec::new();

        // Generate the course if the manifest has a generator config.
        if let Some(generator_config) = &course_manifest.generator_config {
            let generated_course = generator_config.generate_manifests(
                course_root,
                &course_manifest,
                &self.user_preferences,
            )?;
            lessons.extend(generated_course.lessons);

            // Update the course manifest's metadata, material, and instructions if needed.
            if generated_course.updated_metadata.is_some() {
                course_manifest.metadata = generated_course.updated_metadata;
            }
            if generated_course.updated_instructions.is_some() {
                course_manifest.course_instructions = generated_course.updated_instructions;
            }
        }

        // Start a new search from the parent of the passed `DirEntry`, which corresponds to the
        // course's root. Each lesson in the course must be contained in a directory that is a
        // direct descendant of its root. Therefore, all the lesson manifests will be found at a
        // depth of two.
        for entry in WalkDir::new(course_root)
            .min_depth(2)
            .max_depth(2)
            .into_iter()
            .flatten()
        {
            // Ignore any entries which are not directories.
            if entry.path().is_dir() {
                continue;
            }

            // Ignore any files which are not named `lesson_manifest.json`.
            let file_name = Self::get_file_name(entry.path())?;
            if file_name != LESSON_MANIFEST_FILENAME {
                continue;
            }

            // Open the lesson manifest and process it.
            let mut lesson_manifest: LessonManifest = Self::open_manifest(entry.path())?;
            lesson_manifest = lesson_manifest.normalize_paths(entry.path().parent().unwrap())?;
            lessons.push(Self::process_lesson_manifest(
                entry.path().parent().unwrap(),
                &course_manifest,
                lesson_manifest,
            )?);
        }

        Ok(OpenCourseResult {
            manifest: course_manifest,
            lessons,
        })
    }

    /// Inserts the results of opening the courses into the course library.
    fn process_results(&mut self, courses: Vec<OpenCourseResult>) -> Result<()> {
        // Keep track of whether the encompassing and dependency graphs are effectively the same.
        // This is done to save memory when this is true.
        let mut encompassing_equals_dependency = true;

        // Process the courses and the units inside them.
        let mut graph = self.unit_graph.write();
        for course in courses {
            // Add the course and update all the graphs.
            graph.add_course(course.manifest.id)?;
            graph.add_dependencies(
                course.manifest.id,
                UnitType::Course,
                &course.manifest.dependencies,
            )?;
            graph.add_encompassed(
                course.manifest.id,
                &course.manifest.dependencies,
                &course.manifest.encompassed,
            )?;
            graph.add_superseded(course.manifest.id, &course.manifest.superseded);

            // Check if the encompassing and dependency graphs are the same and add the manifest
            // to the course map.
            if !course.manifest.encompassed.is_empty() {
                encompassing_equals_dependency = false;
            }
            self.course_map
                .insert(course.manifest.id, Arc::new(course.manifest));

            // Process the lessons.
            for (lesson_manifest, exercises) in course.lessons {
                // Add the lesson and update all the graphs.
                graph.add_lesson(lesson_manifest.id, lesson_manifest.course_id)?;
                graph.add_dependencies(
                    lesson_manifest.id,
                    UnitType::Lesson,
                    &lesson_manifest.dependencies,
                )?;
                graph.add_encompassed(
                    lesson_manifest.id,
                    &lesson_manifest.dependencies,
                    &lesson_manifest.encompassed,
                )?;
                graph.add_superseded(lesson_manifest.id, &lesson_manifest.superseded);

                // Check if the encompassing and dependency graphs are the same and add the manifest
                // to the lesson map.
                if !lesson_manifest.encompassed.is_empty() {
                    encompassing_equals_dependency = false;
                }
                self.lesson_map
                    .insert(lesson_manifest.id, Arc::new(lesson_manifest));

                // Process the exercises.
                for exercise_manifest in exercises {
                    // Add the exercise to the unit graph and exercise map.
                    graph.add_exercise(exercise_manifest.id, exercise_manifest.lesson_id)?;
                    self.exercise_map
                        .insert(exercise_manifest.id, Arc::new(exercise_manifest));
                }
            }
        }

        // Compute the lessons in a course not dependent on any other lesson in the course. This
        // allows the scheduler to traverse the lessons in a course in the correct order.
        graph.update_starting_lessons();

        // Perform a check to detect cyclic dependencies to prevent infinite loops during traversal.
        graph.check_cycles()?;

        // Delete the encompassing graph if possible to save memory.
        if encompassing_equals_dependency {
            graph.set_encompasing_equals_dependency();
        }
        Ok(())
    }

    /// A constructor taking the path to the root of the library.
    pub fn new(library_root: &Path, user_preferences: UserPreferences) -> Result<Self> {
        let mut library = LocalCourseLibrary {
            course_map: UstrMap::default(),
            lesson_map: UstrMap::default(),
            exercise_map: UstrMap::default(),
            user_preferences,
            unit_graph: Arc::new(RwLock::new(InMemoryUnitGraph::default())),
        };

        // Convert the list of paths to ignore into absolute paths.
        let absolute_root = path::absolute(library_root)?;
        let ignored_paths = library
            .user_preferences
            .ignored_paths
            .iter()
            .map(|path| {
                let mut absolute_path = absolute_root.clone();
                absolute_path.push(path);
                absolute_path
            })
            .collect::<Vec<_>>();

        // Start a search for courses from the library root. Courses can be located at any level
        // within the library root. However, the course manifests, assets, and its lessons and
        // exercises follow a fixed structure.
        let mut courses = Vec::new();
        for entry in WalkDir::new(library_root)
            .min_depth(2)
            .into_iter()
            .flatten()
        {
            // Ignore any entries which are not directories.
            if entry.path().is_dir() {
                continue;
            }

            // Ignore any files which are not named `course_manifest.json`.
            let file_name = Self::get_file_name(entry.path())?;
            if file_name != COURSE_MANIFEST_FILENAME {
                continue;
            }

            // Ignore any directory that matches the list of paths to ignore.
            if ignored_paths
                .iter()
                .any(|ignored_path| entry.path().starts_with(ignored_path))
            {
                continue;
            }

            // Open the course manifest and create a request to open the course.
            let mut course_manifest: CourseManifest = Self::open_manifest(entry.path())?;
            let parent = entry.path().parent().unwrap();
            course_manifest = course_manifest.normalize_paths(parent)?;
            courses.push(OpenCourseRequest {
                course_root: parent.to_path_buf(),
                course_manifest,
            });
        }

        // Process the courses in parallel and add them to the library.
        let course_results = courses
            .into_par_iter()
            .map(|course| {
                library.process_course_manifest(&course.course_root, course.course_manifest)
            })
            .collect::<Result<Vec<_>>>()?;
        library.process_results(course_results)?;
        Ok(library)
    }

    /// A constructor taking a serialized library.
    pub fn new_from_serialized(
        serialized_library: SerializedCourseLibrary,
        user_preferences: UserPreferences,
    ) -> Result<Self> {
        Ok(LocalCourseLibrary {
            course_map: serialized_library
                .course_map
                .into_iter()
                .map(|(k, v)| (k, Arc::new(v)))
                .collect(),
            lesson_map: serialized_library
                .lesson_map
                .into_iter()
                .map(|(k, v)| (k, Arc::new(v)))
                .collect(),
            exercise_map: serialized_library
                .exercise_map
                .into_iter()
                .map(|(k, v)| (k, Arc::new(v)))
                .collect(),
            user_preferences,
            unit_graph: Arc::new(RwLock::new(serialized_library.unit_graph)),
        })
    }
}

impl CourseLibrary for LocalCourseLibrary {
    fn get_course_manifest(&self, course_id: Ustr) -> Option<Arc<CourseManifest>> {
        self.course_map.get(&course_id).cloned()
    }

    fn get_lesson_manifest(&self, lesson_id: Ustr) -> Option<Arc<LessonManifest>> {
        self.lesson_map.get(&lesson_id).cloned()
    }

    fn get_exercise_manifest(&self, exercise_id: Ustr) -> Option<Arc<ExerciseManifest>> {
        self.exercise_map.get(&exercise_id).cloned()
    }

    fn get_course_ids(&self) -> Vec<Ustr> {
        let mut courses = self.course_map.keys().copied().collect::<Vec<Ustr>>();
        courses.sort();
        courses
    }

    fn get_lesson_ids(&self, course_id: Ustr) -> Option<Vec<Ustr>> {
        let mut lessons = self
            .unit_graph
            .read()
            .get_course_lessons(course_id)?
            .iter()
            .copied()
            .collect::<Vec<Ustr>>();
        lessons.sort();
        Some(lessons)
    }

    fn get_exercise_ids(&self, lesson_id: Ustr) -> Option<Vec<Ustr>> {
        let mut exercises = self
            .unit_graph
            .read()
            .get_lesson_exercises(lesson_id)?
            .iter()
            .copied()
            .collect::<Vec<Ustr>>();
        exercises.sort();
        Some(exercises)
    }

    fn get_all_exercise_ids(&self, unit_id: Option<Ustr>) -> Vec<Ustr> {
        let unit_graph = self.unit_graph.read();
        let mut exercises = match unit_id {
            Some(unit_id) => {
                // Return the exercises according to the type of the unit.
                let unit_type = unit_graph.get_unit_type(unit_id);
                match unit_type {
                    Some(UnitType::Course) => unit_graph
                        .get_course_lessons(unit_id)
                        .unwrap_or_default()
                        .iter()
                        .copied()
                        .flat_map(|lesson_id| {
                            unit_graph
                                .get_lesson_exercises(lesson_id)
                                .unwrap_or_default()
                                .iter()
                                .copied()
                                .collect::<Vec<Ustr>>()
                        })
                        .collect::<Vec<Ustr>>(),
                    Some(UnitType::Lesson) => unit_graph
                        .get_lesson_exercises(unit_id)
                        .unwrap_or_default()
                        .iter()
                        .copied()
                        .collect::<Vec<Ustr>>(),
                    Some(UnitType::Exercise) => vec![unit_id],
                    None => vec![],
                }
            }
            // If none, return all the exercises in the library.
            None => self.exercise_map.keys().copied().collect::<Vec<Ustr>>(),
        };

        // Sort the exercises before returning them.
        exercises.sort();
        exercises
    }

    fn get_matching_prefix(&self, prefix: &str, unit_type: Option<UnitType>) -> UstrSet {
        match unit_type {
            Some(UnitType::Course) => self
                .course_map
                .iter()
                .filter_map(|(id, _)| {
                    if id.starts_with(prefix) {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect(),
            Some(UnitType::Lesson) => self
                .lesson_map
                .iter()
                .filter_map(|(id, _)| {
                    if id.starts_with(prefix) {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect(),
            Some(UnitType::Exercise) => self
                .exercise_map
                .iter()
                .filter_map(|(id, _)| {
                    if id.starts_with(prefix) {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect(),
            None => self
                .course_map
                .keys()
                .chain(self.lesson_map.keys())
                .chain(self.exercise_map.keys())
                .filter(|id| id.starts_with(prefix))
                .copied()
                .collect(),
        }
    }
}

impl GetUnitGraph for LocalCourseLibrary {
    fn get_unit_graph(&self) -> Arc<RwLock<InMemoryUnitGraph>> {
        self.unit_graph.clone()
    }
}
