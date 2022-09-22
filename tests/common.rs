//! Contains utilities common to all Trane end-to-end tests.
//!
//! The `scheduler` module in Trane is difficult to test with unit tests that check the expected
//! return value of `get_exercise_batch` against the actual output for a couple of reasons:
//! - When the dependents of a unit are added to the stack during the depth-first search phase, they
//!   are shuffled beforehand to avoid traversing the graph in the same order every time.
//! - In the second phase, candidate exercises are grouped in buckets based on their score. From
//!   each of these buckets, a subset is selected randomly based on weights assigned to each
//!   candidate.
//!
//! Instead of testing the output of a single call to `get_exercise_batch`, Trane's end-to-end tests
//! verify less strict assertions about the results of a simulated study session in which a
//! simulated student scores simulated exercises. For example, given a course library, an empty
//! blacklist, and a student that always scores the highest score in all exercises, all the
//! exercises in the course should be scheduled after going through a sufficiently large number of
//! exercise batches. If this assertion fails, it points to a bug in the scheduler that is not
//! letting the student make progress past some points in the graph.
//!
//! In practice, this testing strategy has been used to catch many bugs in Trane. Most of these bugs
//! were in the scheduler, but some were in other parts of the codebase as this simulation opens a
//! real course library that is written to disk beforehand. While this testing strategy cannot catch
//! every bug, it has proved to offer sufficient coverage for the part of Trane's codebase that is
//! difficult to verify using simple unit tests.

use std::{collections::BTreeMap, path::PathBuf};

use anyhow::{anyhow, Result};
use chrono::Utc;
use rayon::prelude::*;
use trane::{
    blacklist::Blacklist,
    course_builder::{AssetBuilder, CourseBuilder, ExerciseBuilder, LessonBuilder},
    data::{
        filter::UnitFilter, BasicAsset, CourseManifest, ExerciseAsset, ExerciseManifest,
        ExerciseManifestBuilder, ExerciseType, LessonManifestBuilder, MasteryScore,
    },
    practice_stats::PracticeStats,
    scheduler::ExerciseScheduler,
    Trane,
};
use ustr::{Ustr, UstrMap};

/// Represents the ID of a test unit. First element is the course ID, followed by optional lesson
/// and exercise IDs.
#[derive(Debug, Clone, PartialEq)]
pub struct TestId(pub u32, pub Option<u32>, pub Option<u32>);

impl TestId {
    /// Returns whether the exercise ID is part of the given lesson.
    #[allow(dead_code)]
    pub fn exercise_in_lesson(&self, lesson: &TestId) -> bool {
        self.0 == lesson.0 && self.1 == lesson.1 && self.2.is_some()
    }

    /// Returns whether the exercise ID is part of the given course.
    #[allow(dead_code)]
    pub fn exercise_in_course(&self, course: &TestId) -> bool {
        self.0 == course.0 && self.1.is_some() && self.2.is_some()
    }

    pub fn to_ustr(&self) -> Ustr {
        Ustr::from(&self.to_string())
    }
}

impl ToString for TestId {
    /// Converts the test ID to a valid string representation.
    fn to_string(&self) -> String {
        let mut s = self.0.to_string();
        if let Some(lesson_id) = &self.1 {
            s.push_str("::");
            s.push_str(&lesson_id.to_string());
        }
        if let Some(exercise_id) = &self.2 {
            s.push_str("::");
            s.push_str(&exercise_id.to_string());
        }
        s
    }
}

impl From<&Ustr> for TestId {
    /// Converts a string representation of a test ID to a `TestId`.
    fn from(s: &Ustr) -> Self {
        let mut parts = s.split("::");
        let course_id = parts.next().unwrap().parse::<u32>().unwrap();
        let lesson_id = parts.next().map(|s| s.parse::<u32>().unwrap());
        let exercise_id = parts.next().map(|s| s.parse::<u32>().unwrap());
        TestId(course_id, lesson_id, exercise_id)
    }
}

/// A test lesson, containing some number of dummy exercises.
pub struct TestLesson {
    /// ID of the lesson.
    pub id: TestId,

    /// Dependencies of the lesson.
    pub dependencies: Vec<TestId>,

    /// Metadata of the lesson.
    pub metadata: BTreeMap<String, Vec<String>>,

    /// Number of exercises in the lesson.
    pub num_exercises: usize,
}

impl TestLesson {
    /// Returns the lesson builder needed to generate the files for the lesson.
    fn lesson_builder(&self) -> Result<LessonBuilder> {
        if self.id.1.is_none() {
            return Err(anyhow!("Lesson ID is missing"));
        }
        if self.id.2.is_some() {
            return Err(anyhow!("Exercise ID is present"));
        }

        let exercise_builders = (0..self.num_exercises)
            .map(|i| {
                let id_clone = self.id.clone();
                ExerciseBuilder {
                    directory_name: format! {"exercise_{}", i.to_string()},
                    manifest_closure: Box::new(move |m| {
                        let exercise_id = TestId(id_clone.0, id_clone.1, Some(i as u32)).to_ustr();
                        m.clone()
                            .id(exercise_id)
                            .name(format! {"Exercise {}", exercise_id})
                            .description(Some(format! {"Description for exercise {}", exercise_id}))
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

        let metadata_clone = self.metadata.clone();
        let id_clone = self.id.clone();
        let dependencies_clone = self.dependencies.clone();
        Ok(LessonBuilder {
            directory_name: format!("lesson_{}", self.id.1.unwrap()),
            manifest_closure: Box::new(move |m| {
                let lesson_id = id_clone.to_ustr();
                m.clone()
                    .id(lesson_id)
                    .name(format! {"Lesson {}", lesson_id})
                    .description(Some(format! {"Description for lesson {}", lesson_id}))
                    .dependencies(dependencies_clone.iter().map(|id| id.to_ustr()).collect())
                    .metadata(Some(metadata_clone.clone()))
                    .clone()
            }),
            exercise_manifest_template: ExerciseManifestBuilder::default()
                .course_id(TestId(self.id.0, None, None).to_ustr())
                .lesson_id(self.id.to_ustr())
                .exercise_type(ExerciseType::Procedural)
                .exercise_asset(ExerciseAsset::FlashcardAsset {
                    front_path: "question.md".to_string(),
                    back_path: "answer.md".to_string(),
                })
                .clone(),
            exercise_builders: exercise_builders,
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

    /// The metadata of the course.
    pub metadata: BTreeMap<String, Vec<String>>,

    /// The lessons in the course.
    pub lessons: Vec<TestLesson>,
}

impl TestCourse {
    /// Returns the course builder needed to generate the files for the course.
    pub fn course_builder(&self) -> Result<CourseBuilder> {
        if self.id.1.is_some() {
            return Err(anyhow!("Lesson ID is present"));
        }
        if self.id.2.is_some() {
            return Err(anyhow!("Exercise ID is present"));
        }
        for lesson in &self.lessons {
            if lesson.id.0 != self.id.0 {
                return Err(anyhow!("Course ID in lesson does not match course ID"));
            }
        }

        let lesson_builders = self
            .lessons
            .iter()
            .map(|lesson| lesson.lesson_builder())
            .collect::<Result<Vec<_>>>()?;
        let course_id = self.id.to_ustr();
        Ok(CourseBuilder {
            directory_name: format!("course_{}", self.id.0),
            course_manifest: CourseManifest {
                id: course_id,
                name: format!("Course {}", course_id),
                dependencies: self.dependencies.iter().map(|id| id.to_ustr()).collect(),
                description: Some(format!("Description for course {}", course_id)),
                authors: None,
                metadata: Some(self.metadata.clone()),
                course_material: Some(BasicAsset::MarkdownAsset {
                    path: "material.md".to_string(),
                }),
                course_instructions: Some(BasicAsset::MarkdownAsset {
                    path: "instructions.md".to_string(),
                }),
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
        let mut exercises = vec![];
        for lesson in &self.lessons {
            for exercise in 0..lesson.num_exercises {
                exercises.push(TestId(
                    self.id.0,
                    Some(lesson.id.1.unwrap()),
                    Some(exercise as u32),
                ));
            }
        }
        exercises
    }
}

/// Returns the test IDs for all the exercises in the given courses.
pub fn all_exercises(courses: &Vec<TestCourse>) -> Vec<TestId> {
    let mut exercises = vec![];
    for course in courses {
        exercises.extend(course.all_exercises());
    }
    exercises
}

/// Simulates the responses to questions that are presented to the user and analyzes the results.
pub struct TraneSimulation {
    /// Number of exercises that will be presented to the user during the simulation.
    pub num_exercises: usize,

    /// Given an exercise ID, returns the mastery score for the exercise. A return value of None
    /// indicates that the exercise should be skipped.
    pub answer_closure: Box<dyn Fn(&str) -> Option<MasteryScore>>,

    /// Stores the entire history of exercises and their answers during the simulation.
    pub answer_history: UstrMap<Vec<MasteryScore>>,
}

impl TraneSimulation {
    /// Constructs a new simulation object.
    pub fn new(
        num_questions: usize,
        answer_closure: Box<dyn Fn(&str) -> Option<MasteryScore>>,
    ) -> Self {
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
        filter: Option<&UnitFilter>,
    ) -> Result<()> {
        for unit_id in blacklist {
            trane.add_to_blacklist(&unit_id.to_ustr())?;
        }

        let mut completed_exercises = 0;
        let mut batch: Vec<(Ustr, ExerciseManifest)> = vec![];
        while completed_exercises < self.num_exercises {
            completed_exercises += 1;
            if batch.is_empty() {
                batch = trane.get_exercise_batch(filter)?;
            }
            if batch.is_empty() {
                break;
            }

            let (exercise_id, _) = batch.pop().unwrap();
            let score = (self.answer_closure)(&exercise_id);
            if score.is_some() {
                trane.score_exercise(
                    &exercise_id,
                    score.clone().unwrap(),
                    Utc::now().timestamp(),
                )?;
                self.answer_history
                    .entry(exercise_id)
                    .or_insert(vec![])
                    .push(score.unwrap());
            }
        }

        Ok(())
    }
}

/// Takes the given courses and builds them in the given directory. Returns a fully initialized
/// instance of Trane with the courses loaded.
pub fn init_trane(library_directory: &PathBuf, courses: &Vec<TestCourse>) -> Result<Trane> {
    courses
        .into_par_iter()
        .map(|course| course.course_builder()?.build(library_directory))
        .collect::<Result<()>>()?;
    let trane = Trane::new(library_directory.as_path())?;
    Ok(trane)
}

/// Asserts that the scores in the simulation match the scores reported by Trane for the given
/// exercise.
pub fn assert_scores(
    exercise_id: &Ustr,
    trane: &Trane,
    simulation_scores: &UstrMap<Vec<MasteryScore>>,
) -> Result<()> {
    // Get the last ten scores in the interest of saving time.
    let trane_scores = trane.get_scores(exercise_id, 10)?;

    // Check that the last ten simulation scores equal `trane_scores`.
    let simulation_scores = simulation_scores.get(exercise_id).ok_or(anyhow!(
        "No simulation scores for exercise with ID {:?}",
        exercise_id
    ))?;
    let most_recent_scores = simulation_scores.iter().rev().take(trane_scores.len());
    let _: Vec<()> =
        most_recent_scores
            .zip(trane_scores.iter())
            .map(|(simulation_score, trial)| {
                assert_eq!(
                trial.score,
                simulation_score.float_score(),
                "Score from Trane ({}) does not match score from simulation ({}) for exercise {}",
                trial.score, simulation_score.float_score(), exercise_id.to_string()
            );
            })
            .collect();

    Ok(())
}
