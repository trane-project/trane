//! Defines the operations that can be performed on a collection of courses stored by the student.
//!
//! A course library (the term Trane library will be used interchangeably) is a collection of
//! courses that the student wishes to practice together. Courses, lessons, and exercises are
//! defined by their manifest files (see [data](crate::data)).

use anyhow::{anyhow, ensure, Context, Result};
use parking_lot::RwLock;
use serde::de::DeserializeOwned;
use std::{collections::BTreeMap, fs::File, io::BufReader, path::Path, sync::Arc};
use tantivy::{
    collector::TopDocs,
    doc,
    query::QueryParser,
    schema::{Field, Schema, Value, STORED, TEXT},
    Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument,
};
use ustr::{Ustr, UstrMap, UstrSet};
use walkdir::WalkDir;

use crate::{
    data::{
        CourseManifest, ExerciseManifest, GenerateManifests, LessonManifest, NormalizePaths,
        UnitType, UserPreferences,
    },
    error::CourseLibraryError,
    graph::{InMemoryUnitGraph, UnitGraph},
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
    fn get_course_manifest(&self, course_id: Ustr) -> Option<CourseManifest>;

    /// Returns the lesson manifest for the given lesson.
    fn get_lesson_manifest(&self, lesson_id: Ustr) -> Option<LessonManifest>;

    /// Returns the exercise manifest for the given exercise.
    fn get_exercise_manifest(&self, exercise_id: Ustr) -> Option<ExerciseManifest>;

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

    /// Returns the IDs of all the units which match the given query.
    fn search(&self, query: &str) -> Result<Vec<Ustr>, CourseLibraryError>;
}

/// A trait that retrieves the unit graph generated after reading a course library.
pub(crate) trait GetUnitGraph {
    /// Returns a reference to the in-memory unit graph describing the dependencies among the
    /// courses and lessons in this library.
    fn get_unit_graph(&self) -> Arc<RwLock<InMemoryUnitGraph>>;
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
    /// A `UnitGraph` constructed when opening the library.
    unit_graph: Arc<RwLock<InMemoryUnitGraph>>,

    /// A mapping of course ID to its corresponding course manifest.
    course_map: UstrMap<CourseManifest>,

    /// A mapping of lesson ID to its corresponding lesson manifest.
    lesson_map: UstrMap<LessonManifest>,

    /// A mapping of exercise ID to its corresponding exercise manifest.
    exercise_map: UstrMap<ExerciseManifest>,

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
        description: Option<&str>,
        metadata: Option<&BTreeMap<String, Vec<String>>>,
    ) -> Result<()> {
        // Extract the description from the `Option` value to satisfy the borrow checker.
        let empty = String::new();
        let description = description.unwrap_or(&empty);

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

    /// Adds an exercise to the course library given its manifest and the manifest of the lesson to
    /// which it belongs.
    fn process_exercise_manifest(
        &mut self,
        lesson_manifest: &LessonManifest,
        exercise_manifest: ExerciseManifest,
        index_writer: &mut IndexWriter,
    ) -> Result<()> {
        LocalCourseLibrary::verify_exercise_manifest(lesson_manifest, &exercise_manifest)?;

        // Add the exercise manifest to the search index.
        Self::add_to_index_writer(
            index_writer,
            exercise_manifest.id,
            &exercise_manifest.name,
            exercise_manifest.description.as_deref(),
            None,
        )?;

        // Add the exercise to the unit graph and exercise map.
        self.unit_graph
            .write()
            .add_exercise(exercise_manifest.id, exercise_manifest.lesson_id)?;
        self.exercise_map
            .insert(exercise_manifest.id, exercise_manifest);
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

    /// Converts the integer weights to float weights. The largest value is assigned a weight of
    /// 1.0, and the rest are assigned proportional weights. Missing dependencies are assigned the
    /// largest weight. If no weights are provided, each dependency is assigned a weight of 1.0.
    fn convert_weights(dependencies: &[Ustr], weights: Option<&BTreeMap<Ustr, u8>>) -> Vec<f32> {
        if let Some(weights) = weights {
            let mut converted_weights = Vec::new();
            let max_weight = *weights.values().max().unwrap_or(&1);
            for dependency in dependencies {
                if let Some(weight) = weights.get(dependency) {
                    converted_weights.push(f32::from(*weight) / f32::from(max_weight));
                } else {
                    converted_weights.push(1.0);
                }
            }
            converted_weights
        } else {
            vec![1.0; dependencies.len()]
        }
    }

    /// Adds a lesson to the course library given its manifest and the manifest of the course to
    /// which it belongs. It also traverses the given `DirEntry` and adds all the exercises in the
    /// lesson.
    fn process_lesson_manifest(
        &mut self,
        lesson_root: &Path,
        course_manifest: &CourseManifest,
        lesson_manifest: &LessonManifest,
        index_writer: &mut IndexWriter,
        generated_exercises: Option<&Vec<ExerciseManifest>>,
    ) -> Result<()> {
        LocalCourseLibrary::verify_lesson_manifest(course_manifest, lesson_manifest)?;

        // Add the lesson, the dependencies, and the superseded units explicitly listed in the
        // lesson manifest.
        self.unit_graph
            .write()
            .add_lesson(lesson_manifest.id, lesson_manifest.course_id)?;
        self.unit_graph.write().add_dependencies(
            lesson_manifest.id,
            UnitType::Lesson,
            &lesson_manifest.dependencies,
            &Self::convert_weights(
                &lesson_manifest.dependencies,
                lesson_manifest.dependency_weights.as_ref(),
            ),
        )?;
        self.unit_graph
            .write()
            .add_superseded(lesson_manifest.id, &lesson_manifest.superseded);

        // Add the generated exercises to the lesson.
        if let Some(exercises) = generated_exercises {
            for exercise_manifest in exercises {
                let exercise_manifest = &exercise_manifest.normalize_paths(lesson_root)?;
                self.process_exercise_manifest(
                    lesson_manifest,
                    exercise_manifest.clone(),
                    index_writer,
                )?;
            }
        }

        // Start a new search from the parent of the passed `DirEntry`, which corresponds to the
        // lesson's root. Each exercise in the lesson must be contained in a directory that is a
        // direct descendant of its root. Therefore, all the exercise manifests will be found at a
        // depth of two.
        for entry in WalkDir::new(lesson_root)
            .min_depth(2)
            .max_depth(2)
            .into_iter()
            .flatten()
        {
            // Ignore any entries which are not directories.
            if entry.path().is_dir() {
                continue;
            }

            // Ignore any files which are not named `exercise_manifest.json`.
            let file_name = Self::get_file_name(entry.path())?;
            if file_name != EXERCISE_MANIFEST_FILENAME {
                continue;
            }

            // Open the exercise manifest and process it.
            let mut exercise_manifest: ExerciseManifest = Self::open_manifest(entry.path())?;
            exercise_manifest =
                exercise_manifest.normalize_paths(entry.path().parent().unwrap())?;
            self.process_exercise_manifest(lesson_manifest, exercise_manifest, index_writer)?;
        }

        // Add the lesson manifest to the lesson map and the search index.
        self.lesson_map
            .insert(lesson_manifest.id, lesson_manifest.clone());
        Self::add_to_index_writer(
            index_writer,
            lesson_manifest.id,
            &lesson_manifest.name,
            lesson_manifest.description.as_deref(),
            lesson_manifest.metadata.as_ref(),
        )?;
        Ok(())
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
        &mut self,
        course_root: &Path,
        mut course_manifest: CourseManifest,
        index_writer: &mut IndexWriter,
    ) -> Result<()> {
        LocalCourseLibrary::verify_course_manifest(&course_manifest)?;

        // Add the course, the dependencies, and the superseded units explicitly listed in the
        // manifest.
        self.unit_graph.write().add_course(course_manifest.id)?;
        self.unit_graph.write().add_dependencies(
            course_manifest.id,
            UnitType::Course,
            &course_manifest.dependencies,
            &Self::convert_weights(
                &course_manifest.dependencies,
                course_manifest.dependency_weights.as_ref(),
            ),
        )?;
        self.unit_graph
            .write()
            .add_superseded(course_manifest.id, &course_manifest.superseded);

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
                    &lesson_manifest,
                    index_writer,
                    Some(&exercise_manifests),
                )?;
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
            self.process_lesson_manifest(
                entry.path().parent().unwrap(),
                &course_manifest,
                &lesson_manifest,
                index_writer,
                None,
            )?;
        }

        // Add the course manifest to the course map and the search index. This needs to happen at
        // the end in case the course has a generator config and the course manifest was updated.
        self.course_map
            .insert(course_manifest.id, course_manifest.clone());
        Self::add_to_index_writer(
            index_writer,
            course_manifest.id,
            &course_manifest.name,
            course_manifest.description.as_deref(),
            course_manifest.metadata.as_ref(),
        )?;
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
            index: Index::create_in_ram(Self::search_schema()),
            reader: None,
        };

        // Initialize the search index writer with an initial arena size of 150 MB.
        let mut index_writer = library.index.writer(150_000_000)?;

        // Convert the list of paths to ignore into absolute paths.
        let absolute_root = library_root.canonicalize()?;
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

        // Start a search from the library root. Courses can be located at any level within the
        // library root. However, the course manifests, assets, and its lessons and exercises follow
        // a fixed structure.
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

            // Open the course manifest and process it.
            let mut course_manifest: CourseManifest = Self::open_manifest(entry.path())?;
            course_manifest = course_manifest.normalize_paths(entry.path().parent().unwrap())?;
            library.process_course_manifest(
                entry.path().parent().unwrap(),
                course_manifest,
                &mut index_writer,
            )?;
        }

        // Commit the search index writer and initialize the reader in the course library.
        index_writer.commit()?;
        library.reader = Some(
            library
                .index
                .reader_builder()
                .reload_policy(ReloadPolicy::OnCommitWithDelay)
                .try_into()?,
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
            return Ok(Vec::new());
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
                let doc: TantivyDocument = searcher.doc(doc_address)?;
                let id = doc.get_first(id_field).unwrap();
                Ok(id.as_str().unwrap_or("").to_string().into())
            })
            .collect::<Result<Vec<Ustr>>>()
    }
}

impl CourseLibrary for LocalCourseLibrary {
    fn get_course_manifest(&self, course_id: Ustr) -> Option<CourseManifest> {
        self.course_map.get(&course_id).cloned()
    }

    fn get_lesson_manifest(&self, lesson_id: Ustr) -> Option<LessonManifest> {
        self.lesson_map.get(&lesson_id).cloned()
    }

    fn get_exercise_manifest(&self, exercise_id: Ustr) -> Option<ExerciseManifest> {
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
            .into_iter()
            .collect::<Vec<Ustr>>();
        lessons.sort();
        Some(lessons)
    }

    fn get_exercise_ids(&self, lesson_id: Ustr) -> Option<Vec<Ustr>> {
        let mut exercises = self
            .unit_graph
            .read()
            .get_lesson_exercises(lesson_id)?
            .into_iter()
            .collect::<Vec<Ustr>>();
        exercises.sort();
        Some(exercises)
    }

    fn get_all_exercise_ids(&self, unit_id: Option<Ustr>) -> Vec<Ustr> {
        let mut exercises = match unit_id {
            Some(unit_id) => {
                // Return the exercises according to the type of the unit.
                let unit_type = self.unit_graph.read().get_unit_type(unit_id);
                match unit_type {
                    Some(UnitType::Course) => self
                        .unit_graph
                        .read()
                        .get_course_lessons(unit_id)
                        .unwrap_or_default()
                        .into_iter()
                        .flat_map(|lesson_id| {
                            self.unit_graph
                                .read()
                                .get_lesson_exercises(lesson_id)
                                .unwrap_or_default()
                        })
                        .collect::<Vec<Ustr>>(),
                    Some(UnitType::Lesson) => self
                        .unit_graph
                        .read()
                        .get_lesson_exercises(unit_id)
                        .unwrap_or_default()
                        .into_iter()
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

    fn search(&self, query: &str) -> Result<Vec<Ustr>, CourseLibraryError> {
        self.search_helper(query)
            .map_err(|e| CourseLibraryError::Search(query.into(), e))
    }
}

impl GetUnitGraph for LocalCourseLibrary {
    fn get_unit_graph(&self) -> Arc<RwLock<InMemoryUnitGraph>> {
        self.unit_graph.clone()
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use std::collections::BTreeMap;

    /// Verifies that the weights are converted correctly.
    #[test]
    fn convert_weights() {
        let dependencies = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        let weights = Some(BTreeMap::from([
            ("a".into(), 1),
            ("b".into(), 2),
            ("d".into(), 4),
        ]));
        let converted_weights =
            super::LocalCourseLibrary::convert_weights(&dependencies, weights.as_ref());
        assert_eq!(converted_weights, vec![0.25, 0.5, 1.0, 1.0]);
    }

    /// Verifies that the weights are converted correctly when no weights are provided.
    #[test]
    fn convert_weights_empty() {
        let dependencies = vec!["a".into(), "b".into(), "c".into()];
        let converted_weights = super::LocalCourseLibrary::convert_weights(&dependencies, None);
        assert_eq!(converted_weights, vec![1.0, 1.0, 1.0]);
    }
}
