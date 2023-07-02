//! Defines the operations that can be performed on a collection of courses stored by the student.
//!
//! A course library (the term Trane library will be used interchangeably) is a collection of
//! courses that the student wishes to practice together. Courses, lessons, and exercises are
//! defined by their manifest files (see [data](crate::data)).

use anyhow::{anyhow, bail, ensure, Context, Result};
use parking_lot::RwLock;
use serde::de::DeserializeOwned;
use std::{
    collections::BTreeMap,
    fs::{create_dir, File},
    io::{BufReader, Write},
    path::Path,
    sync::Arc,
};
use tantivy::{
    collector::TopDocs,
    doc,
    query::QueryParser,
    schema::{Field, Schema, STORED, TEXT},
    Index, IndexReader, IndexWriter, ReloadPolicy,
};
use ustr::{Ustr, UstrMap};
use walkdir::WalkDir;

use crate::{
    data::{
        CourseManifest, ExerciseManifest, GenerateManifests, LessonManifest, NormalizePaths,
        UnitType, UserPreferences,
    },
    error::CourseLibraryError,
    graph::{InMemoryUnitGraph, UnitGraph},
    FILTERS_DIR, STUDY_SESSIONS_DIR, TRANE_CONFIG_DIR_PATH, USER_PREFERENCES_PATH,
};

/// The file name for all course manifests.
pub const COURSE_MANIFEST_FILENAME: &str = "course_manifest.json";

/// The file name for all lesson manifests.
pub const LESSON_MANIFEST_FILENAME: &str = "lesson_manifest.json";

/// The file name for all exercise manifests.
pub const EXERCISE_MANIFEST_FILENAME: &str = "exercise_manifest.json";

/// The name of the field for the unit ID in the search schema.
const ID_SCHEMA_FIELD: &str = "id";

/// The name of the field for the unit name in the search schema.
const NAME_SCHEMA_FIELD: &str = "name";

/// The name of the field for the unit description in the search schema.
const DESCRIPTION_SCHEMA_FIELD: &str = "description";

/// The name of the field for the unit metadata in the search schema.
const METADATA_SCHEMA_FIELD: &str = "metadata";

/// A trait that manages a course library, its corresponding manifest files, and provides basic
/// operations to retrieve the courses, lessons in a course, and exercises in a lesson.
pub trait CourseLibrary {
    /// Returns the course manifest for the given course.
    fn get_course_manifest(&self, course_id: &Ustr) -> Option<Arc<RwLock<CourseManifest>>>;

    /// Returns the lesson manifest for the given lesson.
    fn get_lesson_manifest(&self, lesson_id: &Ustr) -> Option<Arc<RwLock<LessonManifest>>>;

    /// Returns the exercise manifest for the given exercise.
    fn get_exercise_manifest(&self, exercise_id: &Ustr) -> Option<Arc<RwLock<ExerciseManifest>>>;

    /// Returns the IDs of all courses in the library sorted alphabetically.
    fn get_course_ids(&self) -> Vec<Ustr>;

    /// Returns the IDs of all lessons in the given course sorted alphabetically.
    fn get_lesson_ids(&self, course_id: &Ustr) -> Option<Vec<Ustr>>;

    /// Returns the IDs of all exercises in the given lesson sorted alphabetically.
    fn get_exercise_ids(&self, lesson_id: &Ustr) -> Option<Vec<Ustr>>;

    /// Returns the IDs of all exercises in the given course sorted alphabetically.
    fn get_all_exercise_ids(&self) -> Vec<Ustr>;

    /// Returns the IDs of all the units which match the given query.
    fn search(&self, query: &str) -> Result<Vec<Ustr>, CourseLibraryError>;

    /// Returns the user preferences found in the library. The default preferences should be
    /// returned if the user preferences file is not found.
    fn get_user_preferences(&self) -> UserPreferences;
}

/// A trait that retrieves the unit graph generated after reading a course library. The visibility
/// is set to `pub(crate)` because `InMemoryUnitGraph` has the same visibility and returning a
/// concrete type avoids the need for indirection.
pub(crate) trait GetUnitGraph {
    /// Returns a reference to the unit graph describing the dependencies among the courses and
    /// lessons in this library.
    fn get_unit_graph(&self) -> Arc<RwLock<InMemoryUnitGraph>>;
}

/// An implementation of [CourseLibrary] backed by the local file system. The courses in this
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
    course_map: UstrMap<Arc<RwLock<CourseManifest>>>,

    /// A mapping of lesson ID to its corresponding lesson manifest.
    lesson_map: UstrMap<Arc<RwLock<LessonManifest>>>,

    /// A mapping of exercise ID to its corresponding exercise manifest.
    exercise_map: UstrMap<Arc<RwLock<ExerciseManifest>>>,

    /// The user preferences.
    user_preferences: UserPreferences,

    /// A tantivy index used for searching the course library.
    index: Index,

    /// A reader to access the search index.
    reader: Option<IndexReader>,
}

impl LocalCourseLibrary {
    /// Returns the tantivy schema used for searching the course library.
    fn search_schema() -> Schema {
        let mut schema = Schema::builder();
        schema.add_text_field(ID_SCHEMA_FIELD, TEXT | STORED);
        schema.add_text_field(NAME_SCHEMA_FIELD, TEXT | STORED);
        schema.add_text_field(DESCRIPTION_SCHEMA_FIELD, TEXT | STORED);
        schema.add_text_field(METADATA_SCHEMA_FIELD, TEXT | STORED);
        schema.build()
    }

    /// Returns the field in the search schema with the given name.
    fn schema_field(field_name: &str) -> Result<Field> {
        let schema = Self::search_schema();
        let field = schema.get_field(field_name)?;
        Ok(field)
    }

    /// Adds the unit with the given field values to the search index.
    fn add_to_index_writer(
        index_writer: &mut IndexWriter,
        id: Ustr,
        name: &str,
        description: &Option<String>,
        metadata: &Option<BTreeMap<String, Vec<String>>>,
    ) -> Result<()> {
        // Extract the description from the `Option` value to satisfy the borrow checker.
        let empty = String::new();
        let description = description.as_ref().unwrap_or(&empty);

        // Declare the base document with the ID, name, and description fields.
        let mut doc = doc!(
            Self::schema_field(ID_SCHEMA_FIELD)? => id.to_string(),
            Self::schema_field(NAME_SCHEMA_FIELD)? => name.to_string(),
            Self::schema_field(DESCRIPTION_SCHEMA_FIELD)? => description.to_string(),
        );

        // Add the metadata. Encode each key-value pair as a string in the format "key:value". Then
        // add the document to the index.
        let metadata_field = Self::schema_field(METADATA_SCHEMA_FIELD)?;
        if let Some(metadata) = metadata {
            for (key, values) in metadata {
                for value in values {
                    doc.add_text(metadata_field, format!("{key}:{value}"));
                }
            }
        }
        index_writer.add_document(doc)?;
        Ok(())
    }

    /// Opens the course, lesson, or exercise manifest located at the given path.
    fn open_manifest<T: DeserializeOwned>(path: &Path) -> Result<T> {
        let file = File::open(path)
            .with_context(|| anyhow!("cannot open manifest file {}", path.display()))?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader)
            .with_context(|| anyhow!("cannot parse manifest file {}", path.display()))
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
        index_writer: &mut IndexWriter,
    ) -> Result<()> {
        // Verify that the IDs mentioned in the manifests are valid and agree with each other.
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

        // Add the exercise manifest to the search index.
        Self::add_to_index_writer(
            index_writer,
            exercise_manifest.id,
            &exercise_manifest.name,
            &exercise_manifest.description,
            &None,
        )?; // grcov-excl-line

        // Add the exercise to the unit graph and exercise map.
        self.unit_graph
            .write()
            .add_exercise(&exercise_manifest.id, &exercise_manifest.lesson_id)?;
        self.exercise_map.insert(
            exercise_manifest.id,
            Arc::new(RwLock::new(exercise_manifest)),
        );
        Ok(())
    }

    /// Adds a lesson to the course library given its manifest and the manifest of the course to
    /// which it belongs. It also traverses the given `DirEntry` and adds all the exercises in the
    /// lesson.
    fn process_lesson_manifest(
        &mut self,
        lesson_root: &Path,
        course_manifest: &CourseManifest,
        lesson_manifest: LessonManifest,
        index_writer: &mut IndexWriter,
        generated_exercises: Option<&Vec<ExerciseManifest>>,
    ) -> Result<()> {
        // Verify that the IDs mentioned in the manifests are valid and agree with each other.
        ensure!(!lesson_manifest.id.is_empty(), "ID in manifest is empty",);
        ensure!(
            lesson_manifest.course_id == course_manifest.id,
            "course_id in manifest for lesson {} does not match the manifest for course {}",
            lesson_manifest.id,
            course_manifest.id,
        );

        // Add the lesson and the dependencies explicitly listed in the lesson manifest.
        self.unit_graph
            .write()
            .add_lesson(&lesson_manifest.id, &lesson_manifest.course_id)?;
        self.unit_graph.write().add_dependencies(
            &lesson_manifest.id,
            UnitType::Lesson,
            &lesson_manifest.dependencies,
        )?;

        // Add the generated exercises to the lesson.
        if let Some(exercises) = generated_exercises {
            for exercise_manifest in exercises {
                let exercise_manifest = &exercise_manifest.normalize_paths(lesson_root)?;
                self.process_exercise_manifest(
                    &lesson_manifest,
                    exercise_manifest.clone(),
                    index_writer,
                )?; // grcov-excl-line
            }
        }

        // Start a new search from the parent of the passed `DirEntry`, which corresponds to the
        // lesson's root. Each exercise in the lesson must be contained in a directory that is a
        // direct descendant of its root. Therefore, all the exercise manifests will be found at a
        // depth of two.
        for entry in WalkDir::new(lesson_root).min_depth(2).max_depth(2) {
            match entry {
                Err(_) => continue,
                Ok(exercise_dir_entry) => {
                    // Ignore any entries which are not directories.
                    if exercise_dir_entry.path().is_dir() {
                        continue;
                    }

                    // Ignore any files which are not named `exercise_manifest.json`.
                    let file_name = Self::get_file_name(exercise_dir_entry.path())?;
                    if file_name != EXERCISE_MANIFEST_FILENAME {
                        continue;
                    }

                    // Open the exercise manifest and process it.
                    let mut exercise_manifest: ExerciseManifest =
                        Self::open_manifest(exercise_dir_entry.path())?;
                    exercise_manifest = exercise_manifest
                        .normalize_paths(exercise_dir_entry.path().parent().unwrap())?;
                    self.process_exercise_manifest(
                        &lesson_manifest,
                        exercise_manifest,
                        index_writer,
                    )?; // grcov-excl-line
                }
            }
        }

        // Add the lesson manifest to the lesson map and the search index.
        self.lesson_map.insert(
            lesson_manifest.id,
            Arc::new(RwLock::new(lesson_manifest.clone())),
        );
        Self::add_to_index_writer(
            index_writer,
            lesson_manifest.id,
            &lesson_manifest.name,
            &lesson_manifest.description,
            &lesson_manifest.metadata,
        )?; // grcov-excl-line
        Ok(())
    }

    /// Adds a course to the course library given its manifest. It also traverses the given
    /// `DirEntry` and adds all the lessons in the course.
    fn process_course_manifest(
        &mut self,
        course_root: &Path,
        mut course_manifest: CourseManifest,
        index_writer: &mut IndexWriter,
    ) -> Result<()> {
        ensure!(!course_manifest.id.is_empty(), "ID in manifest is empty",);

        // Add the course and the dependencies explicitly listed in the manifest.
        self.unit_graph.write().add_course(&course_manifest.id)?;
        self.unit_graph.write().add_dependencies(
            &course_manifest.id,
            UnitType::Course,
            &course_manifest.dependencies,
        )?;

        // If the course has a generator config, generate the lessons and exercises and add them to
        // the library.
        if let Some(generator_config) = &course_manifest.generator_config {
            let generated_course = generator_config.generate_manifests(
                course_root,
                &course_manifest,
                &self.user_preferences,
            )?;
            for (lesson_manifest, exercise_manifests) in generated_course.lessons {
                // All the generated lessons will use the root of the course as their root.
                self.process_lesson_manifest(
                    course_root,
                    &course_manifest,
                    lesson_manifest,
                    index_writer,
                    Some(&exercise_manifests),
                )?; // grcov-excl-line
            }

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
        for entry in WalkDir::new(course_root).min_depth(2).max_depth(2) {
            match entry {
                Err(_) => continue,
                Ok(lesson_dir_entry) => {
                    // Ignore any entries which are not directories.
                    if lesson_dir_entry.path().is_dir() {
                        continue;
                    }

                    // Ignore any files which are not named `lesson_manifest.json`.
                    let file_name = Self::get_file_name(lesson_dir_entry.path())?;
                    if file_name != LESSON_MANIFEST_FILENAME {
                        continue;
                    }

                    // Open the lesson manifest and process it.
                    let mut lesson_manifest: LessonManifest =
                        Self::open_manifest(lesson_dir_entry.path())?;
                    lesson_manifest = lesson_manifest
                        .normalize_paths(lesson_dir_entry.path().parent().unwrap())?;
                    self.process_lesson_manifest(
                        lesson_dir_entry.path().parent().unwrap(),
                        &course_manifest,
                        lesson_manifest,
                        index_writer,
                        None,
                    )?; // grcov-excl-line
                }
            }
        }

        // Add the course manifest to the course map and the search index. This needs to happen at
        // the end in case the course has a generator config and the course manifest was updated.
        self.course_map.insert(
            course_manifest.id,
            Arc::new(RwLock::new(course_manifest.clone())),
        );
        Self::add_to_index_writer(
            index_writer,
            course_manifest.id,
            &course_manifest.name,
            &course_manifest.description,
            &course_manifest.metadata,
        )?; // grcov-excl-line
        Ok(())
    }

    /// Initializes the config directory at path `.trane` inside the library root.
    fn init_config_directory(library_root: &Path) -> Result<()> {
        // Verify that the library root is a directory.
        ensure!(
            library_root.is_dir(),
            "library root {} is not a directory",
            library_root.display(),
        );

        // Create the config folder inside the library root if it does not exist already.
        let trane_path = library_root.join(TRANE_CONFIG_DIR_PATH);
        if !trane_path.exists() {
            create_dir(trane_path.clone()).with_context(|| {
                format!(
                    "failed to create config directory at {}",
                    trane_path.display()
                )
            })?;
        } else if !trane_path.is_dir() {
            bail!("config path .trane inside library must be a directory");
        }

        // Create the `filters` directory if it does not exist already.
        let filters_path = trane_path.join(FILTERS_DIR);
        if !filters_path.is_dir() {
            create_dir(filters_path.clone()).with_context(|| {
                format!(
                    "failed to create filters directory at {}",
                    filters_path.display()
                )
            })?;
        }

        // Create the `study_sessions` directory if it does not exist already.
        let sessions_path = trane_path.join(STUDY_SESSIONS_DIR);
        if !sessions_path.is_dir() {
            create_dir(sessions_path.clone()).with_context(|| {
                format!(
                    "failed to create filters directory at {}",
                    sessions_path.display()
                )
            })?;
        }

        // Create the user preferences file if it does not exist already.
        let user_prefs_path = trane_path.join(USER_PREFERENCES_PATH);
        if !user_prefs_path.exists() {
            // Create the file.
            let mut file = File::create(user_prefs_path.clone()).with_context(|| {
                format!(
                    "failed to create user preferences file at {}",
                    user_prefs_path.display()
                )
            })?;

            // Write the default user preferences to the file.
            let default_prefs = UserPreferences::default();
            let prefs_json = serde_json::to_string_pretty(&default_prefs)? + "\n";
            file.write_all(prefs_json.as_bytes()).with_context(|| {
                // grcov-excl-start: File should be writable.
                format!(
                    "failed to write to user preferences file at {}",
                    user_prefs_path.display()
                )
                // grcov-excl-stop
            })?; // grcov-excl-line
        } else if !user_prefs_path.is_file() {
            // The user preferences file exists but is not a regular file.
            bail!(
                "user preferences file must be a regular file at {}",
                user_prefs_path.display()
            );
        }

        Ok(())
    }

    /// Returns the user preferences stored in the local course library.
    fn open_preferences(library_root: &Path) -> Result<UserPreferences> {
        // The user preferences should exist when this function is called.
        let path = library_root
            .join(TRANE_CONFIG_DIR_PATH)
            .join(USER_PREFERENCES_PATH);
        let file = File::open(path.clone())
            .with_context(|| anyhow!("cannot open user preferences file {}", path.display()))?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader)
            .with_context(|| anyhow!("cannot parse user preferences file {}", path.display()))
    }

    /// A constructor taking the path to the root of the library.
    pub fn new(library_root: &Path) -> Result<Self> {
        // Initialize the local course library.
        Self::init_config_directory(library_root)?;
        let user_preferences = Self::open_preferences(library_root)?;
        let mut library = LocalCourseLibrary {
            course_map: UstrMap::default(),
            lesson_map: UstrMap::default(),
            exercise_map: UstrMap::default(),
            user_preferences,
            unit_graph: Arc::new(RwLock::new(InMemoryUnitGraph::default())),
            index: Index::create_in_ram(Self::search_schema()),
            reader: None,
        };

        // Initialize the search index writer with an initial arena size of 50 MB.
        let mut index_writer = library.index.writer(50_000_000)?;

        // Convert the list of paths to ignore into absolute paths.
        let absolute_root = library_root.canonicalize()?;
        let ignored_paths = library
            .user_preferences
            .ignored_paths
            .iter()
            .map(|path| {
                let mut absolute_path = absolute_root.to_path_buf();
                absolute_path.push(path);
                absolute_path
            })
            .collect::<Vec<_>>();

        // Start a search from the library root. Courses can be located at any level within the
        // library root. However, the course manifests, assets, and its lessons and exercises follow
        // a fixed structure.
        for entry in WalkDir::new(library_root).min_depth(2) {
            match entry {
                Err(_) => continue,
                Ok(dir_entry) => {
                    // Ignore any entries which are not directories.
                    if dir_entry.path().is_dir() {
                        continue;
                    }

                    // Ignore any files which are not named `course_manifest.json`.
                    let file_name = Self::get_file_name(dir_entry.path())?;
                    if file_name != COURSE_MANIFEST_FILENAME {
                        continue;
                    }

                    // Ignore any directory that matches the list of paths to ignore.
                    if ignored_paths
                        .iter()
                        .any(|ignored_path| dir_entry.path().starts_with(ignored_path))
                    {
                        continue;
                    }

                    // Open the course manifest and process it.
                    let mut course_manifest: CourseManifest =
                        Self::open_manifest(dir_entry.path())?;
                    course_manifest =
                        course_manifest.normalize_paths(dir_entry.path().parent().unwrap())?;
                    library.process_course_manifest(
                        dir_entry.path().parent().unwrap(),
                        course_manifest,
                        &mut index_writer,
                    )?; // grcov-excl-line
                }
            }
        }

        // Commit the search index writer and initialize the reader in the course library.
        index_writer.commit()?;
        library.reader = Some(
            library
                .index
                .reader_builder()
                .reload_policy(ReloadPolicy::OnCommit)
                .try_into()?, // grcov-excl-line
        );

        // Compute the lessons in a course not dependent on any other lesson in the course. This
        // allows the scheduler to traverse the lessons in a course in the correct order.
        library.unit_graph.write().update_starting_lessons();

        // Perform a check to detect cyclic dependencies to prevent infinite loops during traversal.
        library.unit_graph.read().check_cycles()?;
        Ok(library)
    }

    /// Helper function to search the course library.
    fn search_helper(&self, query: &str) -> Result<Vec<Ustr>> {
        // Retrieve a searcher from the reader and parse the query.
        if self.reader.is_none() {
            // This should never happen since the reader is initialized in the constructor.
            return Ok(Vec::new()); // grcov-excl-line
        }
        let searcher = self.reader.as_ref().unwrap().searcher();
        let id_field = Self::schema_field(ID_SCHEMA_FIELD)?;
        let query_parser = QueryParser::for_index(
            &self.index,
            vec![
                id_field,
                Self::schema_field(NAME_SCHEMA_FIELD)?,
                Self::schema_field(DESCRIPTION_SCHEMA_FIELD)?,
                Self::schema_field(METADATA_SCHEMA_FIELD)?,
            ],
        );
        let query = query_parser.parse_query(query)?;

        // Execute the query and return the results as a list of unit IDs.
        let top_docs = searcher.search(&query, &TopDocs::with_limit(50))?;
        top_docs
            .into_iter()
            .map(|(_, doc_address)| {
                let doc = searcher.doc(doc_address)?;
                let id = doc.get_first(id_field).unwrap();
                Ok(id.as_text().unwrap_or("").to_string().into())
            })
            .collect::<Result<Vec<Ustr>>>()
    }
}

impl CourseLibrary for LocalCourseLibrary {
    fn get_course_manifest(&self, course_id: &Ustr) -> Option<Arc<RwLock<CourseManifest>>> {
        self.course_map.get(course_id).cloned()
    }

    fn get_lesson_manifest(&self, lesson_id: &Ustr) -> Option<Arc<RwLock<LessonManifest>>> {
        self.lesson_map.get(lesson_id).cloned()
    }

    fn get_exercise_manifest(&self, exercise_id: &Ustr) -> Option<Arc<RwLock<ExerciseManifest>>> {
        self.exercise_map.get(exercise_id).cloned()
    }

    fn get_course_ids(&self) -> Vec<Ustr> {
        let mut courses = self.course_map.keys().cloned().collect::<Vec<Ustr>>();
        courses.sort();
        courses
    }

    fn get_lesson_ids(&self, course_id: &Ustr) -> Option<Vec<Ustr>> {
        let mut lessons = self
            .unit_graph
            .read()
            .get_course_lessons(course_id)?
            .into_iter()
            .collect::<Vec<Ustr>>();
        lessons.sort();
        Some(lessons)
    }

    fn get_exercise_ids(&self, lesson_id: &Ustr) -> Option<Vec<Ustr>> {
        let mut exercises = self
            .unit_graph
            .read()
            .get_lesson_exercises(lesson_id)?
            .into_iter()
            .collect::<Vec<Ustr>>();
        exercises.sort();
        Some(exercises)
    }

    fn get_all_exercise_ids(&self) -> Vec<Ustr> {
        let mut exercises = self.exercise_map.keys().cloned().collect::<Vec<Ustr>>();
        exercises.sort();
        exercises
    }

    fn search(&self, query: &str) -> Result<Vec<Ustr>, CourseLibraryError> {
        self.search_helper(query)
            .map_err(|e| CourseLibraryError::Search(query.into(), e))
    }

    fn get_user_preferences(&self) -> UserPreferences {
        self.user_preferences.clone()
    }
}

impl GetUnitGraph for LocalCourseLibrary {
    fn get_unit_graph(&self) -> Arc<RwLock<InMemoryUnitGraph>> {
        self.unit_graph.clone()
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use std::{fs::create_dir, os::unix::prelude::PermissionsExt};

    use crate::{
        course_library::LocalCourseLibrary, FILTERS_DIR, STUDY_SESSIONS_DIR, TRANE_CONFIG_DIR_PATH,
        USER_PREFERENCES_PATH,
    };

    /// Verifies opening a course library with a path that is not a directory fails.
    #[test]
    fn path_is_not_dir() -> Result<()> {
        let file = tempfile::NamedTempFile::new()?;
        let result = LocalCourseLibrary::new(file.path());
        assert!(result.is_err());
        Ok(())
    }

    /// Verifies that opening a library where the `.trane` directory exists but is a file fails.
    #[test]
    fn user_preferences_file_is_a_dir() -> Result<()> {
        // Create directory `./trane/user_preferences.json` which is not a file.
        let temp_dir = tempfile::tempdir()?;
        std::fs::create_dir_all(
            temp_dir
                .path()
                .join(TRANE_CONFIG_DIR_PATH)
                .join(USER_PREFERENCES_PATH),
        )?;
        assert!(LocalCourseLibrary::new(temp_dir.path()).is_err());
        Ok(())
    }

    /// Verifies that opening a library fails if the `.trane/filters` directory cannot be created.
    #[test]
    fn cannot_create_filters_directory() -> Result<()> {
        // Create config directory.
        let temp_dir = tempfile::tempdir()?;
        let config_dir = temp_dir.path().join(TRANE_CONFIG_DIR_PATH);
        create_dir(config_dir.clone())?;

        // Set permissions of `.trane` directory to read-only.
        std::fs::set_permissions(temp_dir.path(), std::fs::Permissions::from_mode(0o444))?;

        assert!(LocalCourseLibrary::new(temp_dir.path()).is_err());
        Ok(())
    }

    /// Verifies that opening a library fails if the `.trane/study_sessions` directory cannot be
    /// created.
    #[test]
    fn cannot_create_study_sessions() -> Result<()> {
        // Create config and filters directories.
        let temp_dir = tempfile::tempdir()?;
        let config_dir = temp_dir.path().join(TRANE_CONFIG_DIR_PATH);
        create_dir(config_dir.clone())?;
        let filters_dir = config_dir.join(FILTERS_DIR);
        create_dir(filters_dir)?;

        // Set permissions of `.trane` directory to read-only. This should prevent Trane from
        // creating the user preferences file.
        std::fs::set_permissions(config_dir, std::fs::Permissions::from_mode(0o500))?;
        assert!(LocalCourseLibrary::new(temp_dir.path()).is_err());
        Ok(())
    }

    /// Verifies that opening a library fails if the `.trane/user_preferences.json` file cannot be
    /// created.
    #[test]
    fn cannot_create_user_preferences() -> Result<()> {
        // Create config, filters, and study sessions directories.
        let temp_dir = tempfile::tempdir()?;
        let config_dir = temp_dir.path().join(TRANE_CONFIG_DIR_PATH);
        create_dir(config_dir.clone())?;
        let filters_dir = config_dir.join(FILTERS_DIR);
        create_dir(filters_dir)?;
        let sessions_dir = config_dir.join(STUDY_SESSIONS_DIR);
        create_dir(sessions_dir)?;

        // Set permissions of `.trane` directory to read-only. This should prevent Trane from
        // creating the user preferences file.
        std::fs::set_permissions(config_dir, std::fs::Permissions::from_mode(0o500))?;
        assert!(LocalCourseLibrary::new(temp_dir.path()).is_err());
        Ok(())
    }
}
