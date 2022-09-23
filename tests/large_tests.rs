//! End-to-end tests for verifying the correctness of Trane with large course libraries.
//!
//! These tests verify that Trane works correctly with large course libraries and can be used to
//! measure its performance with the use of the `cargo flamegraph` command. These tests are slower
//! than the unit tests and the basic end-to-end tests, but they still run under 10 seconds when
//! compiled in release mode.

mod common;

use std::collections::BTreeMap;

use anyhow::{Ok, Result};
use rand::Rng;
use tempfile::TempDir;
use trane::data::MasteryScore;

use crate::common::*;

/// A struct to create a randomly generated course library for use in stress testing and profiling.
/// All ranges in this struct are inclusive.
struct RandomCourseLibrary {
    /// The total number of exercises in the library.
    num_courses: u32,

    /// Each course will have a random number of dependencies in this range.
    course_dependencies_range: (u32, u32),

    /// Each course will have a random number of lessons in this range.
    lessons_per_course_range: (u32, u32),

    // Each lesson will have a random number of dependencies in this range.
    lesson_dependencies_range: (u32, u32),

    /// Each lesson will have a random number of exercises in this range.
    exercises_per_lesson_range: (usize, usize),
}

impl RandomCourseLibrary {
    /// Generates random dependencies for the given course. All dependencies are to courses with a
    /// lower course ID to ensure the graph is acyclic.
    fn generate_course_dependencies(&self, course_id: &TestId, rng: &mut impl Rng) -> Vec<TestId> {
        let num_dependencies =
            rng.gen_range(self.course_dependencies_range.0..=self.course_dependencies_range.1);
        let mut dependencies = Vec::with_capacity(num_dependencies as usize);
        for _ in 0..num_dependencies.min(course_id.0) {
            let dependency_id = TestId(rng.gen_range(0..course_id.0), None, None);
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
        let num_dependencies =
            rng.gen_range(self.lesson_dependencies_range.0..=self.lesson_dependencies_range.1);
        let mut dependencies = Vec::with_capacity(num_dependencies as usize);
        for _ in 0..num_dependencies.min(lesson_id.1.unwrap_or(0)) {
            let dependency_id = TestId(
                lesson_id.0,
                Some(rng.gen_range(0..lesson_id.1.unwrap_or(0))),
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
    fn generate_library(&self) -> Vec<TestCourse> {
        let mut courses = vec![];
        let mut rng = rand::thread_rng();
        for course_index in 0..self.num_courses {
            let mut lessons = vec![];
            let num_lessons =
                rng.gen_range(self.lessons_per_course_range.0..=self.lessons_per_course_range.1);
            for lesson_index in 0..num_lessons {
                let num_exercises = rng.gen_range(
                    self.exercises_per_lesson_range.0..=self.exercises_per_lesson_range.1,
                );

                let lesson_id = TestId(course_index, Some(lesson_index), None);
                let lesson = TestLesson {
                    id: lesson_id.clone(),
                    dependencies: self.generate_lesson_dependencies(&lesson_id, &mut rng),
                    metadata: BTreeMap::new(),
                    num_exercises: num_exercises,
                };
                lessons.push(lesson);
            }

            let course_id = TestId(course_index, None, None);
            courses.push(TestCourse {
                id: course_id.clone(),
                dependencies: self.generate_course_dependencies(&course_id, &mut rng),
                metadata: BTreeMap::new(),
                lessons: lessons,
            });
        }
        courses
    }
}

/// A test that verifies that all the exercises are scheduled with no blacklist or filter when the
/// user gives a score of five to every exercise, even in a course library with a lot of exercises.
#[test]
fn all_exercises_scheduled_random() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let random_library = RandomCourseLibrary {
        num_courses: 25,
        course_dependencies_range: (0, 5),
        lessons_per_course_range: (0, 5),
        lesson_dependencies_range: (0, 5),
        exercises_per_lesson_range: (0, 20),
    }
    .generate_library();
    let mut trane = init_trane(&temp_dir.path().to_path_buf(), &random_library)?;

    // Run the simulation.
    let exercise_ids = all_exercises(&random_library);
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 100,
        Box::new(|_| Some(MasteryScore::Five)),
    );
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // Every exercise ID should be in `simulation.answer_history`.
    for exercise_id in exercise_ids {
        assert!(
            simulation
                .answer_history
                .contains_key(&exercise_id.to_ustr()),
            "exercise {:?} should have been scheduled",
            exercise_id
        );
        assert_scores(&exercise_id.to_ustr(), &trane, &simulation.answer_history)?;
    }
    Ok(())
}

/// A test that generates and reads a very large course library. Used mostly to keep track of how
/// long this operation takes.
#[test]
fn generate_and_read_large_library() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let random_library = RandomCourseLibrary {
        num_courses: 100,
        course_dependencies_range: (0, 10),
        lessons_per_course_range: (1, 10),
        lesson_dependencies_range: (0, 10),
        exercises_per_lesson_range: (1, 20),
    }
    .generate_library();
    init_trane(&temp_dir.path().to_path_buf(), &random_library)?;
    Ok(())
}
