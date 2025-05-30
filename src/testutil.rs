//! Contains utilities common to Trane tests.
//!
//! This module contains utilities to make it easier to generate test libraries, either handwritten
//! or randomly generated, as well as a way to simulate a student scoring Trane questions. The
//! simulation is used by the end-to-end tests to verify that Trane works correctly in different
//! scenarios.

use std::{
    collections::BTreeMap,
    fmt::{Display, Formatter},
    fs::{self, File},
    io::Write,
    path::Path,
};

use anyhow::{Result, bail, ensure};
use chrono::Utc;
use rand::Rng;
use rayon::prelude::*;

use ustr::{Ustr, UstrMap};

use crate::{
    TRANE_CONFIG_DIR_PATH, Trane, USER_PREFERENCES_PATH,
    blacklist::Blacklist,
    course_builder::{AssetBuilder, CourseBuilder, ExerciseBuilder, LessonBuilder},
    data::{
        BasicAsset, CourseManifest, ExerciseAsset, ExerciseManifestBuilder, ExerciseType,
        LessonManifestBuilder, MasteryScore, UserPreferences, filter::ExerciseFilter,
    },
    practice_stats::PracticeStats,
    scheduler::ExerciseScheduler,
};

/// Represents the ID of a test unit. First element is the course ID, followed by optional lesson
/// and exercise IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestId(pub usize, pub Option<usize>, pub Option<usize>);

impl TestId {
    /// Returns whether the exercise ID is part of the given lesson.
    #[allow(dead_code)]
    #[must_use]
    pub fn exercise_in_lesson(&self, lesson: &TestId) -> bool {
        self.0 == lesson.0 && self.1 == lesson.1 && self.2.is_some()
    }

    /// Returns whether the exercise ID is part of the given course.
    #[allow(dead_code)]
    #[must_use]
    pub fn exercise_in_course(&self, course: &TestId) -> bool {
        self.0 == course.0 && self.1.is_some() && self.2.is_some()
    }

    /// Coverts the test ID to a `Ustr` value.
    #[must_use]
    pub fn to_ustr(&self) -> Ustr {
        Ustr::from(&self.to_string())
    }

    /// Returns whether the test ID belongs to a course.
    #[must_use]
    pub fn is_course(&self) -> bool {
        self.1.is_none() && self.2.is_none()
    }

    /// Returns whether the test ID belongs to a lesson.
    #[must_use]
    pub fn is_lesson(&self) -> bool {
        self.1.is_some() && self.2.is_none()
    }

    /// Returns whether the test ID belongs to an exercise.
    #[must_use]
    pub fn is_exercise(&self) -> bool {
        self.1.is_some() && self.2.is_some()
    }
}

impl Display for TestId {
    /// Converts the test ID to a valid string representation.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)?;
        if let Some(lesson_id) = &self.1 {
            write!(f, "::{lesson_id}")?;
        }
        if let Some(exercise_id) = &self.2 {
            write!(f, "::{exercise_id}")?;
        }
        Ok(())
    }
}

impl From<&Ustr> for TestId {
    /// Converts a string representation of a test ID to a test ID
    fn from(s: &Ustr) -> Self {
        let mut parts = s.split("::");
        let course_id = parts.next().unwrap().parse::<usize>().unwrap();
        let lesson_id = parts.next().map(|s| s.parse::<usize>().unwrap());
        let exercise_id = parts.next().map(|s| s.parse::<usize>().unwrap());
        TestId(course_id, lesson_id, exercise_id)
    }
}

/// A test lesson, containing some number of dummy exercises.
pub struct TestLesson {
    /// ID of the lesson.
    pub id: TestId,

    /// Dependencies of the lesson.
    pub dependencies: Vec<TestId>,

    /// The courses or lessons superseded by this lesson.
    pub superseded: Vec<TestId>,

    /// Metadata of the lesson.
    pub metadata: BTreeMap<String, Vec<String>>,

    /// Number of exercises in the lesson.
    pub num_exercises: usize,
}

impl TestLesson {
    /// Returns the lesson builder needed to generate the files for the lesson.
    fn lesson_builder(&self) -> Result<LessonBuilder> {
        // Validate the lesson ID.
        ensure!(self.id.is_lesson(), "Invalid lesson ID");

        // Generate the correct number of exercise builders.
        let exercise_builders = (0..self.num_exercises)
            .map(|i| {
                let id_clone = self.id.clone();
                ExerciseBuilder {
                    directory_name: format!("exercise_{i}"),
                    manifest_closure: Box::new(move |m| {
                        let exercise_id = TestId(id_clone.0, id_clone.1, Some(i)).to_ustr();
                        #[allow(clippy::redundant_clone)]
                        m.clone()
                            .id(exercise_id)
                            .name(format!("Exercise {exercise_id}"))
                            .description(Some(format!("Description for exercise {exercise_id}")))
                            .clone()
                    }),
                    asset_builders: vec![
                        AssetBuilder {
                            file_name: "question.md".to_string(),
                            contents: "question".to_string(),
                        },
                        AssetBuilder {
                            file_name: "answer.md".to_string(),
                            contents: "answer".to_string(),
                        },
                    ],
                }
            })
            .collect::<Vec<_>>();

        // Generate the lesson builder.
        let metadata_clone = self.metadata.clone();
        let id_clone = self.id.clone();
        let dependencies_clone = self.dependencies.clone();
        let superseded_clone = self.superseded.clone();
        Ok(LessonBuilder {
            directory_name: format!("lesson_{}", self.id.1.unwrap()),
            manifest_closure: Box::new(move |m| {
                let lesson_id = id_clone.to_ustr();
                #[allow(clippy::redundant_clone)]
                m.clone()
                    .id(lesson_id)
                    .name(format!("Lesson {lesson_id}"))
                    .description(Some(format!("Description for lesson {lesson_id}")))
                    .dependencies(dependencies_clone.iter().map(TestId::to_ustr).collect())
                    .superseded(superseded_clone.iter().map(TestId::to_ustr).collect())
                    .metadata(Some(metadata_clone.clone()))
                    .clone()
            }),
            exercise_manifest_template: ExerciseManifestBuilder::default()
                .course_id(TestId(self.id.0, None, None).to_ustr())
                .lesson_id(self.id.to_ustr())
                .exercise_type(ExerciseType::Procedural)
                .exercise_asset(ExerciseAsset::FlashcardAsset {
                    front_path: "question.md".to_string(),
                    back_path: Some("answer.md".to_string()),
                })
                .clone(),
            exercise_builders,
            asset_builders: vec![
                AssetBuilder {
                    file_name: "instructions.md".to_string(),
                    contents: "instructions".to_string(),
                },
                AssetBuilder {
                    file_name: "material.md".to_string(),
                    contents: "material".to_string(),
                },
            ],
        })
    }
}

/// A test course containing a number of dummy test lessons.
pub struct TestCourse {
    /// The ID of the course.
    pub id: TestId,

    /// The dependencies of the course.
    pub dependencies: Vec<TestId>,

    /// The courses or lessons this course supersedes.
    pub superseded: Vec<TestId>,

    /// The metadata of the course.
    pub metadata: BTreeMap<String, Vec<String>>,

    /// The lessons in the course.
    pub lessons: Vec<TestLesson>,
}

impl TestCourse {
    /// Returns the course builder needed to generate the files for the course.
    pub fn course_builder(&self) -> Result<CourseBuilder> {
        // Validate the course ID.
        ensure!(self.id.is_course(), "Invalid course ID");

        // Validate the lesson IDs.
        for lesson in &self.lessons {
            if lesson.id.0 != self.id.0 {
                bail!("Course ID in lesson does not match course ID");
            }
        }

        // Generate the lesson builders.
        let lesson_builders = self
            .lessons
            .iter()
            .map(TestLesson::lesson_builder)
            .collect::<Result<Vec<_>>>()?;

        // Generate the course builder.
        let course_id = self.id.to_ustr();
        Ok(CourseBuilder {
            directory_name: format!("course_{}", self.id.0),
            course_manifest: CourseManifest {
                id: course_id,
                name: format!("Course {course_id}"),
                dependencies: self.dependencies.iter().map(TestId::to_ustr).collect(),
                superseded: self.superseded.iter().map(TestId::to_ustr).collect(),
                description: Some(format!("Description for course {course_id}")),
                authors: None,
                metadata: Some(self.metadata.clone()),
                course_material: Some(BasicAsset::MarkdownAsset {
                    path: "material.md".to_string(),
                }),
                course_instructions: Some(BasicAsset::MarkdownAsset {
                    path: "instructions.md".to_string(),
                }),
                generator_config: None,
            },
            lesson_manifest_template: LessonManifestBuilder::default()
                .course_id(self.id.to_ustr())
                .lesson_instructions(Some(BasicAsset::MarkdownAsset {
                    path: "instructions.md".to_string(),
                }))
                .lesson_material(Some(BasicAsset::MarkdownAsset {
                    path: "material.md".to_string(),
                }))
                .clone(),
            lesson_builders,
            asset_builders: vec![
                AssetBuilder {
                    file_name: "instructions.md".to_string(),
                    contents: "instructions".to_string(),
                },
                AssetBuilder {
                    file_name: "material.md".to_string(),
                    contents: "material".to_string(),
                },
            ],
        })
    }

    /// Returns the IDs of all the exercises in the course.
    fn all_exercises(&self) -> Vec<TestId> {
        // Construct a test ID for each exercise in each lesson.
        let mut exercises = vec![];
        for lesson in &self.lessons {
            for exercise in 0..lesson.num_exercises {
                exercises.push(TestId(
                    self.id.0,
                    Some(lesson.id.1.unwrap()),
                    Some(exercise),
                ));
            }
        }
        exercises
    }
}

/// Returns the test IDs for all the exercises in the given courses.
#[must_use]
pub fn all_test_exercises(courses: &Vec<TestCourse>) -> Vec<TestId> {
    // Collect the exercise test IDs from each course.
    let mut exercises = vec![];
    for course in courses {
        exercises.extend(course.all_exercises());
    }
    exercises
}

/// A struct to create a randomly generated course library for use in stress testing and profiling.
/// All ranges in this struct are inclusive.
pub struct RandomCourseLibrary {
    /// The total number of exercises in the library.
    pub num_courses: usize,

    /// Each course will have a random number of dependencies in this range.
    pub course_dependencies_range: (u32, u32),

    /// Each course will have a random number of lessons in this range.
    pub lessons_per_course_range: (u32, u32),

    /// Each lesson will have a random number of dependencies in this range.
    pub lesson_dependencies_range: (u32, u32),

    /// Each lesson will have a random number of exercises in this range.
    pub exercises_per_lesson_range: (usize, usize),
}

impl RandomCourseLibrary {
    /// Generates random dependencies for the given course. All dependencies are to courses with a
    /// lower course ID to ensure the graph is acyclic.
    fn generate_course_dependencies(&self, course_id: &TestId, rng: &mut impl Rng) -> Vec<TestId> {
        let num_dependencies = rng
            .random_range(self.course_dependencies_range.0..=self.course_dependencies_range.1)
            as usize;
        if num_dependencies == 0 {
            return vec![];
        }

        let mut dependencies = Vec::with_capacity(num_dependencies);
        for _ in 0..num_dependencies.min(course_id.0) {
            let dependency_id = TestId(rng.random_range(0..course_id.0), None, None);
            if dependencies.contains(&dependency_id) {
                continue;
            }
            dependencies.push(dependency_id);
        }
        dependencies
    }

    /// Generates random dependencies for the given course. All dependencies are to other lessons in
    /// the same course with a lower course ID to ensure the graph is acyclic.
    fn generate_lesson_dependencies(&self, lesson_id: &TestId, rng: &mut impl Rng) -> Vec<TestId> {
        let num_dependencies = rng
            .random_range(self.lesson_dependencies_range.0..=self.lesson_dependencies_range.1)
            as usize;
        let mut dependencies = Vec::with_capacity(num_dependencies);
        for _ in 0..num_dependencies.min(lesson_id.1.unwrap_or(0)) {
            let dependency_id = TestId(
                lesson_id.0,
                Some(rng.random_range(0..lesson_id.1.unwrap_or(0))),
                None,
            );
            if dependencies.contains(&dependency_id) {
                continue;
            }
            dependencies.push(dependency_id);
        }
        dependencies
    }

    /// Generates the entire randomized course library.
    #[must_use]
    pub fn generate_library(&self) -> Vec<TestCourse> {
        let mut courses = vec![];
        let mut rng = rand::rng();
        for course_index in 0..self.num_courses {
            let mut lessons = vec![];
            let num_lessons = rng
                .random_range(self.lessons_per_course_range.0..=self.lessons_per_course_range.1)
                as usize;
            for lesson_index in 0..num_lessons {
                let num_exercises = rng.random_range(
                    self.exercises_per_lesson_range.0..=self.exercises_per_lesson_range.1,
                );

                let lesson_id = TestId(course_index, Some(lesson_index), None);
                let lesson = TestLesson {
                    id: lesson_id.clone(),
                    dependencies: self.generate_lesson_dependencies(&lesson_id, &mut rng),
                    superseded: vec![],
                    metadata: BTreeMap::new(),
                    num_exercises,
                };
                lessons.push(lesson);
            }

            let course_id = TestId(course_index, None, None);
            courses.push(TestCourse {
                id: course_id.clone(),
                dependencies: self.generate_course_dependencies(&course_id, &mut rng),
                superseded: vec![],
                metadata: BTreeMap::new(),
                lessons,
            });
        }
        courses
    }
}

// The type of the closure needed to score an exercise given its ID.
type AnswerClosure = Box<dyn Fn(&str) -> Option<MasteryScore>>;

/// Simulates the responses to questions that are presented to the user and analyzes the results.
pub struct TraneSimulation {
    /// Number of exercises that will be presented to the user during the simulation.
    pub num_exercises: usize,

    /// Given an exercise ID, returns the mastery score for the exercise. A return value of None
    /// indicates that the exercise should be skipped.
    pub answer_closure: AnswerClosure,

    /// Stores the entire history of exercises and their answers during the simulation.
    pub answer_history: UstrMap<Vec<MasteryScore>>,
}

impl TraneSimulation {
    /// Constructs a new simulation object.
    #[must_use]
    pub fn new(num_questions: usize, answer_closure: AnswerClosure) -> Self {
        Self {
            num_exercises: num_questions,
            answer_closure,
            answer_history: UstrMap::default(),
        }
    }

    /// Runs the simulation with the given instance of Trane, blacklist, and filter.
    pub fn run_simulation(
        &mut self,
        trane: &mut Trane,
        blacklist: &Vec<TestId>,
        filter: &Option<ExerciseFilter>,
    ) -> Result<()> {
        // Update the blacklist.
        for unit_id in blacklist {
            trane.add_to_blacklist(unit_id.to_ustr())?;
        }

        // Initialize the counter and batch.
        let mut completed_exercises = 0;
        let mut batch = vec![];

        // Loop until the simulation has received the desired number of exercises.
        while completed_exercises < self.num_exercises {
            // Update the count.
            completed_exercises += 1;

            // If the batch is empty, try to get another batch. If this batch is also empty, break
            // early to avoid falling into an nfinite loop.
            if batch.is_empty() {
                batch = trane.get_exercise_batch(filter.clone())?;
                if batch.is_empty() {
                    break;
                }
            }

            // Retrieve an exercise, compute its score, add it to the history, and submit it.
            let exercise_manifest = batch.pop().unwrap();
            let score = (self.answer_closure)(&exercise_manifest.id);
            if let Some(score) = score {
                trane.score_exercise(
                    exercise_manifest.id,
                    score.clone(),
                    Utc::now().timestamp(),
                )?;
                self.answer_history
                    .entry(exercise_manifest.id)
                    .or_default()
                    .push(score);
            }
        }

        Ok(())
    }
}

/// Takes the given course builders and builds them in the given directory. Returns a fully
/// initialized instance of Trane and sets the user preferences if provided.
pub fn init_simulation(
    library_root: &Path,
    course_builders: &[CourseBuilder],
    user_preferences: Option<&UserPreferences>,
) -> Result<Trane> {
    // Build the courses.
    course_builders
        .iter()
        .try_for_each(|course_builder| course_builder.build(library_root))?;

    // Write the user preferences if provided.
    if let Some(user_preferences) = user_preferences {
        let config_dir = library_root.join(TRANE_CONFIG_DIR_PATH);
        fs::create_dir(config_dir.clone())?;
        let prefs_path = config_dir.join(USER_PREFERENCES_PATH);
        let mut file = File::create(prefs_path)?;
        let prefs_json = serde_json::to_string_pretty(user_preferences)? + "\n";
        file.write_all(prefs_json.as_bytes())?;
    }

    // Initialize the Trane library.
    let trane = Trane::new_local(library_root, library_root)?;
    Ok(trane)
}

/// Takes the given test courses and builds them in the given directory. Returns a fully initialized
/// instance of Trane with the courses loaded.
pub fn init_test_simulation(library_root: &Path, courses: &Vec<TestCourse>) -> Result<Trane> {
    // Build the courses.
    courses
        .into_par_iter()
        .map(|course| course.course_builder()?.build(library_root))
        .collect::<Result<()>>()?;

    // Initialize the Trane library.
    let before = Utc::now();
    let trane = Trane::new_local(library_root, library_root)?;
    let after = Utc::now();
    println!(
        "Time to load library: {} ms",
        (after - before).num_milliseconds()
    );
    Ok(trane)
}

/// Asserts that the scores in the simulation match the scores reported by Trane for the given
/// exercise.
pub fn assert_simulation_scores(
    exercise_id: Ustr,
    trane: &Trane,
    simulation_scores: &UstrMap<Vec<MasteryScore>>,
) -> Result<()> {
    // Get the last ten scores in the interest of saving time.
    let trane_scores = trane.get_scores(exercise_id, 10)?;

    // Check that the last ten scores from the simulation history equal the scores retrieved
    // directly from Trane.
    let empty_scores = vec![];
    let simulation_scores = simulation_scores.get(&exercise_id).unwrap_or(&empty_scores);
    let most_recent_scores = simulation_scores.iter().rev().take(trane_scores.len());
    let _: Vec<()> = most_recent_scores
        .zip(trane_scores.iter())
        .map(|(simulation_score, trial)| {
            let float_score = simulation_score.float_score();
            assert!(
                (trial.score - float_score).abs() < f32::EPSILON,
                "Score from Trane ({}) does not match score from simulation ({}) for exercise {}",
                trial.score,
                float_score,
                exercise_id,
            );
        })
        .collect();
    Ok(())
}

#[cfg(test)]
mod test {
    use std::{path::Path, sync::LazyLock};

    use crate::testutil::*;

    static NUM_EXERCISES: usize = 2;

    /// A simple set of courses to test the basic functionality of Trane.
    static TEST_LIBRARY: LazyLock<Vec<TestCourse>> = LazyLock::new(|| {
        vec![
            TestCourse {
                id: TestId(0, None, None),
                dependencies: vec![],
                superseded: vec![],
                metadata: BTreeMap::from([
                    (
                        "course_key_1".to_string(),
                        vec!["course_key_1:value_1".to_string()],
                    ),
                    (
                        "course_key_2".to_string(),
                        vec!["course_key_2:value_1".to_string()],
                    ),
                ]),
                lessons: vec![
                    TestLesson {
                        id: TestId(0, Some(0), None),
                        dependencies: vec![],
                        superseded: vec![],
                        metadata: BTreeMap::from([
                            (
                                "lesson_key_1".to_string(),
                                vec!["lesson_key_1:value_1".to_string()],
                            ),
                            (
                                "lesson_key_2".to_string(),
                                vec!["lesson_key_2:value_1".to_string()],
                            ),
                        ]),
                        num_exercises: NUM_EXERCISES,
                    },
                    TestLesson {
                        id: TestId(0, Some(1), None),
                        dependencies: vec![TestId(0, Some(0), None)],
                        superseded: vec![],
                        metadata: BTreeMap::from([
                            (
                                "lesson_key_1".to_string(),
                                vec!["lesson_key_1:value_2".to_string()],
                            ),
                            (
                                "lesson_key_2".to_string(),
                                vec!["lesson_key_2:value_2".to_string()],
                            ),
                        ]),
                        num_exercises: NUM_EXERCISES,
                    },
                ],
            },
            TestCourse {
                id: TestId(1, None, None),
                dependencies: vec![TestId(0, None, None)],
                superseded: vec![],
                metadata: BTreeMap::from([
                    (
                        "course_key_1".to_string(),
                        vec!["course_key_1:value_1".to_string()],
                    ),
                    (
                        "course_key_2".to_string(),
                        vec!["course_key_2:value_1".to_string()],
                    ),
                ]),
                lessons: vec![
                    TestLesson {
                        id: TestId(1, Some(0), None),
                        dependencies: vec![],
                        superseded: vec![],
                        metadata: BTreeMap::from([
                            (
                                "lesson_key_1".to_string(),
                                vec!["lesson_key_1:value_3".to_string()],
                            ),
                            (
                                "lesson_key_2".to_string(),
                                vec!["lesson_key_2:value_3".to_string()],
                            ),
                        ]),
                        num_exercises: NUM_EXERCISES,
                    },
                    TestLesson {
                        id: TestId(1, Some(1), None),
                        dependencies: vec![TestId(1, Some(0), None)],
                        superseded: vec![],
                        metadata: BTreeMap::from([
                            (
                                "lesson_key_1".to_string(),
                                vec!["lesson_key_1:value_3".to_string()],
                            ),
                            (
                                "lesson_key_2".to_string(),
                                vec!["lesson_key_2:value_3".to_string()],
                            ),
                        ]),
                        num_exercises: NUM_EXERCISES,
                    },
                ],
            },
        ]
    });

    /// Verifies checking that a test exercise is in a test lesson.
    #[test]
    fn exercise_in_lesson() {
        let exercise_id = TestId(0, Some(0), Some(0));
        let lesson_id = TestId(0, Some(0), None);
        let other_lesson_id = TestId(0, Some(1), None);

        assert!(exercise_id.exercise_in_lesson(&lesson_id));
        assert!(!exercise_id.exercise_in_lesson(&other_lesson_id));
    }

    /// Verifies checking that a test exercise is in a test course.
    #[test]
    fn exercise_in_course() {
        let exercise_id = TestId(0, Some(0), Some(0));
        let course_id = TestId(0, None, None);
        let other_course_id = TestId(1, None, None);

        assert!(exercise_id.exercise_in_course(&course_id));
        assert!(!exercise_id.exercise_in_course(&other_course_id));
    }

    /// Verifies checking the type of test ID.
    #[test]
    fn id_type() {
        assert!(TestId(0, None, None).is_course());
        assert!(TestId(0, Some(0), None).is_lesson());
        assert!(TestId(0, Some(0), Some(0)).is_exercise());
    }

    /// Verifies converting the test ID to a string.
    #[test]
    fn conversion_to_string() {
        let exercise_id = TestId(0, Some(0), Some(0));
        let lesson_id = TestId(0, Some(0), None);
        let course_id = TestId(0, None, None);

        assert_eq!(exercise_id.to_string(), "0::0::0");
        assert_eq!(lesson_id.to_string(), "0::0");
        assert_eq!(course_id.to_string(), "0");

        assert_eq!(exercise_id.to_ustr(), "0::0::0");
        assert_eq!(lesson_id.to_ustr(), "0::0");
        assert_eq!(course_id.to_ustr(), "0");
    }

    /// Verifies converting a string to a test ID.
    #[test]
    fn conversion_from_string() {
        let exercise_id = TestId(0, Some(0), Some(0));
        let lesson_id = TestId(0, Some(0), None);
        let course_id = TestId(0, None, None);

        assert_eq!(TestId::from(&Ustr::from("0::0::0")), exercise_id);
        assert_eq!(TestId::from(&Ustr::from("0::0")), lesson_id);
        assert_eq!(TestId::from(&Ustr::from("0")), course_id);
    }

    /// Verify that the given test library was built correctly.
    fn verify_test_library(test_library: &[TestCourse], library_path: &Path) {
        for course in test_library {
            // Verify the course directory exists.
            let course_dir = library_path.join(format!("course_{}", course.id.0));
            assert!(course_dir.is_dir());

            // Verify the course manifest exists.
            let course_manifest = course_dir.join("course_manifest.json");
            assert!(course_manifest.is_file());

            // Verify that the course lessons were built correctly.
            for lesson in &course.lessons {
                // Verify the lesson directory exists.
                let lesson_dir = course_dir.join(format!("lesson_{}", lesson.id.1.unwrap()));
                assert!(lesson_dir.is_dir());

                // Verify the lesson manifest exists.
                let lesson_manifest = lesson_dir.join("lesson_manifest.json");
                assert!(lesson_manifest.is_file());

                // Verify all the exercise directories were built correctly.
                for exercise_index in 0..lesson.num_exercises {
                    // Verify the exercise directory exists.
                    let exercise_dir = lesson_dir.join(format!("exercise_{exercise_index}"));
                    assert!(exercise_dir.is_dir());

                    // Verify the exercise manifest exists.
                    let exercise_manifest = exercise_dir.join("exercise_manifest.json");
                    assert!(exercise_manifest.is_file());

                    // Verify the `question.md` and `answer.md` files exist.
                    let question = exercise_dir.join("question.md");
                    let answer = exercise_dir.join("answer.md");
                    assert!(question.is_file());
                    assert!(answer.is_file());
                }
            }
        }
    }

    /// Verifies building a test library.
    #[test]
    fn build_test_library() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        init_test_simulation(temp_dir.path(), &TEST_LIBRARY)?;
        verify_test_library(&TEST_LIBRARY, temp_dir.path());
        Ok(())
    }

    /// Verifies building a random test library.
    #[test]
    fn build_random_test_library() -> Result<()> {
        // Build a random test library.
        let temp_dir = tempfile::tempdir()?;
        let random_library = RandomCourseLibrary {
            num_courses: 5,
            course_dependencies_range: (0, 5),
            lessons_per_course_range: (0, 5),
            lesson_dependencies_range: (0, 5),
            exercises_per_lesson_range: (0, 5),
        }
        .generate_library();
        init_test_simulation(temp_dir.path(), &random_library)?;
        verify_test_library(&random_library, temp_dir.path());
        Ok(())
    }

    /// Verifies building a test lesson with a bad ID fails.
    #[test]
    fn bad_test_lesson() {
        // ID is a course ID.
        let mut lesson = TestLesson {
            id: TestId(1, None, None),
            dependencies: vec![],
            superseded: vec![],
            metadata: BTreeMap::default(),
            num_exercises: NUM_EXERCISES,
        };
        assert!(lesson.lesson_builder().is_err());

        // ID is an exercise ID.
        lesson.id = TestId(1, Some(1), Some(1));
        assert!(lesson.lesson_builder().is_err());
    }

    /// Verifies building a test course with a bad ID fails.
    #[test]
    fn bad_test_course_id() {
        // ID is a lesson ID.
        let mut course = TestCourse {
            id: TestId(1, Some(1), None),
            dependencies: vec![],
            superseded: vec![],
            metadata: BTreeMap::default(),
            lessons: vec![],
        };
        assert!(course.course_builder().is_err());

        // ID is an exercise ID.
        course.id = TestId(1, Some(1), Some(1));
        assert!(course.course_builder().is_err());
    }

    /// Verifies that building a test course with a lesson that does not belong to the course fails.
    #[test]
    fn bad_lesson_in_course() {
        // Lesson ID does not belong to the same course.
        let mut course = TestCourse {
            id: TestId(1, None, None),
            dependencies: vec![],
            superseded: vec![],
            metadata: BTreeMap::default(),
            lessons: vec![TestLesson {
                id: TestId(2, Some(0), None),
                dependencies: vec![],
                superseded: vec![],
                metadata: BTreeMap::default(),
                num_exercises: NUM_EXERCISES,
            }],
        };
        assert!(course.course_builder().is_err());

        // The ID of the lesson is not a lesson ID.
        course.lessons[0].id = TestId(1, None, None);
        assert!(course.course_builder().is_err());
    }

    /// Verifies running an exercise simulation.
    #[test]
    fn run_exercise_simulation() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let mut trane = init_test_simulation(temp_dir.path(), &TEST_LIBRARY)?;

        // Run the simulation answering all exercises with the maximum score.
        let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
        simulation.run_simulation(&mut trane, &vec![], &None)?;

        // Every exercise ID should be in `simulation.answer_history`.
        let exercise_ids = all_test_exercises(&TEST_LIBRARY);
        for exercise_id in exercise_ids {
            let exercise_ustr = exercise_id.to_ustr();
            assert!(
                simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {exercise_id:?} should have been scheduled",
            );
            assert_simulation_scores(exercise_ustr, &trane, &simulation.answer_history)?;
        }
        Ok(())
    }

    /// Verifies that running a simulation with a bad course build fails.
    #[test]
    fn bad_exercise_simulation() -> Result<()> {
        let bad_courses = vec![TestCourse {
            id: TestId(1, Some(1), None),
            dependencies: vec![TestId(0, None, None)],
            superseded: vec![],
            metadata: BTreeMap::default(),
            lessons: vec![],
        }];
        let temp_dir = tempfile::tempdir()?;
        assert!(init_test_simulation(temp_dir.path(), &bad_courses).is_err());
        Ok(())
    }
}
