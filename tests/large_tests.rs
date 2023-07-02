//! End-to-end tests for verifying the correctness of Trane with large course libraries.
//!
//! These tests verify that Trane works correctly with large course libraries and can be used to
//! measure its performance with the use of the `cargo flamegraph` command. These tests are slower
//! than the unit tests and the basic end-to-end tests, but they still run under 10 seconds when
//! compiled in release mode.

use anyhow::{Ok, Result};
use tempfile::TempDir;
use trane::{data::MasteryScore, testutil::*};

/// Verifies that all the exercises are scheduled with no blacklist or filter when the user gives a
/// score of five to every exercise, even in a course library with a lot of exercises.
#[test]
fn all_exercises_scheduled_random() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let random_library = RandomCourseLibrary {
        num_courses: 50,
        course_dependencies_range: (0, 5),
        lessons_per_course_range: (0, 5),
        lesson_dependencies_range: (0, 5),
        exercises_per_lesson_range: (0, 20),
    }
    .generate_library();
    let mut trane = init_test_simulation(&temp_dir.path().to_path_buf(), &random_library)?;

    // Run the simulation.
    let exercise_ids = all_test_exercises(&random_library);
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 50,
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
        assert_simulation_scores(&exercise_id.to_ustr(), &trane, &simulation.answer_history)?;
    }
    Ok(())
}

/// Generates and reads a very large course library. Used mostly to keep track of how long this
/// operation takes.
// #[test]
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
    init_test_simulation(&temp_dir.path().to_path_buf(), &random_library)?;
    Ok(())
}
