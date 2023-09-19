//! End-to-end tests for verifying the correctness of Trane with superseded courses and lessons.
//!
//! For a more detailed explanation of the testing methodology, see the explanation in the
//! basic_tests module.

use std::collections::BTreeMap;

use anyhow::{Ok, Result};
use lazy_static::lazy_static;
use tempfile::TempDir;
use trane::{data::MasteryScore, testutil::*};

lazy_static! {
    /// A simple set of courses to verify that superseded courses and lessons are dealt with
    /// correctly.
    static ref LIBRARY: Vec<TestCourse> = vec![
        TestCourse {
            id: TestId(0, None, None),
            dependencies: vec![],
            superseded: vec![],
            metadata: BTreeMap::default(),
            lessons: vec![
                TestLesson {
                    id: TestId(0, Some(0), None),
                    dependencies: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(0, Some(1), None),
                    dependencies: vec![TestId(0, Some(0), None)],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(1, None, None),
            dependencies: vec![TestId(0, None, None)],
            superseded: vec![TestId(0, None, None)],
            metadata: BTreeMap::default(),
            lessons: vec![
                TestLesson {
                    id: TestId(1, Some(0), None),
                    dependencies: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(1, Some(1), None),
                    dependencies: vec![TestId(1, Some(0), None)],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(2, None, None),
            dependencies: vec![],
            superseded: vec![],
            metadata: BTreeMap::default(),
            lessons: vec![
                TestLesson {
                    id: TestId(2, Some(0), None),
                    dependencies: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(2, Some(1), None),
                    dependencies: vec![TestId(2, Some(0), None)],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(2, Some(2), None),
                    dependencies: vec![TestId(2, Some(1), None)],
                    superseded: vec![TestId(2, Some(0), None)],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
            ],
        },
    ];
}

/// Verifies that the superseded courses are dealt with correctly during scheduling.
#[test]
fn scheduler_respects_superseded_courses() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(&temp_dir.path(), &LIBRARY)?;

    // Run the simulation first giving a score of 5 to all exercises.
    let superseded_course_id = TestId(0, None, None);
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // Every exercise should be in `simulation.answer_history`.
    let exercise_ids = all_test_exercises(&LIBRARY);
    for exercise_id in &exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        assert!(
            simulation.answer_history.contains_key(&exercise_ustr),
            "exercise {:?} should have been scheduled",
            exercise_id
        );
        assert_simulation_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
    }

    // Run the simulation again to clear the simulation history.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // None of the exercises in the superseded course should have been scheduled.
    for exercise_id in &exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if exercise_id.exercise_in_course(&superseded_course_id) {
            assert!(
                !simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should not have been scheduled",
                exercise_id
            );
        }
    }

    // Run the simulation again, but this time give a score of 1 to all exercises in the superseding
    // course.
    let mut simulation = TraneSimulation::new(
        500,
        Box::new(|id| {
            if id.starts_with("1::") {
                Some(MasteryScore::One)
            } else {
                Some(MasteryScore::Five)
            }
        }),
    );
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // This time around, all the exercises in the superseded course should have been scheduled.
    for exercise_id in &exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if exercise_id.exercise_in_course(&superseded_course_id) {
            assert!(
                simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_simulation_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
        }
    }
    Ok(())
}

/// Verifies that the superseded lessons are dealt with correctly during scheduling.
#[test]
fn scheduler_respects_superseded_lessons() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(&temp_dir.path(), &LIBRARY)?;

    // Run the simulation first giving a score of 5 to all exercises.
    let superseded_lesson_id = TestId(2, Some(0), None);
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // Every exercise should be in `simulation.answer_history`.
    let exercise_ids = all_test_exercises(&LIBRARY);
    for exercise_id in &exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        assert!(
            simulation.answer_history.contains_key(&exercise_ustr),
            "exercise {:?} should have been scheduled",
            exercise_id
        );
        assert_simulation_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
    }

    // Run the simulation again to clear the simulation history.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // None of the exercises in the superseded lesson should have been scheduled.
    for exercise_id in &exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if exercise_id.exercise_in_lesson(&superseded_lesson_id) {
            assert!(
                !simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should not have been scheduled",
                exercise_id
            );
        }
    }

    // Run the simulation again, but this time give a score of 1 to all exercises in the superseding
    // lesson.
    let mut simulation = TraneSimulation::new(
        500,
        Box::new(|id| {
            if id.starts_with("2::2::") {
                Some(MasteryScore::One)
            } else {
                Some(MasteryScore::Five)
            }
        }),
    );
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // This time around, all the exercises in the superseded lesson should have been scheduled.
    for exercise_id in &exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if exercise_id.exercise_in_course(&superseded_lesson_id) {
            assert!(
                simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_simulation_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
        }
    }
    Ok(())
}
