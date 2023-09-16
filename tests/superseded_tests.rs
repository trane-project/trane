//! End-to-end tests for verifying the correctness of Trane with superseded courses and lessons.
//!
//! The end-to-end tests in this file verify the functionality of Trane with superseded courses and
//! lessons. For a more detailed explanation of the testing methodology, see the explanation in the
//! basic_tests module.

use std::collections::BTreeMap;

use anyhow::{Ok, Result};
use lazy_static::lazy_static;
use tempfile::TempDir;
use trane::{
    data::{
        filter::{ExerciseFilter, UnitFilter},
        MasteryScore,
    },
    testutil::*,
};

lazy_static! {
    /// A simple set of courses to test the basic functionality of Trane.
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

    // Run the simulation first giving a score of 5 to all exercises in the superseding course.
    let superseded_course_id = TestId(0, None, None);
    let superseding_course_id = TestId(1, None, None);
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(
        &mut trane,
        &vec![],
        Some(ExerciseFilter::UnitFilter(UnitFilter::CourseFilter {
            course_ids: vec![superseding_course_id.to_ustr()],
        })),
    )?;

    // Every exercise ID in the superseding course should be in `simulation.answer_history`.
    let exercise_ids = all_test_exercises(&LIBRARY);
    for exercise_id in &exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if exercise_id.exercise_in_course(&superseding_course_id) {
            assert!(
                simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_simulation_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
        } else {
            assert!(
                !simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should not have been scheduled",
                exercise_id
            );
        }
    }

    // Run the simulation again with no course filter.
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
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::One)));
    simulation.run_simulation(
        &mut trane,
        &vec![],
        Some(ExerciseFilter::UnitFilter(UnitFilter::CourseFilter {
            course_ids: vec![superseding_course_id.to_ustr()],
        })),
    )?;

    // Run the simulation again with no course filter and giving a score of 5 to all exercises.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
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

    // Run the simulation first giving a score of 5 to all exercises in the superseding lesson.
    let superseded_lesson_id = TestId(2, Some(0), None);
    let superseding_lesson_id = TestId(2, Some(2), None);
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(
        &mut trane,
        &vec![],
        Some(ExerciseFilter::UnitFilter(UnitFilter::LessonFilter {
            lesson_ids: vec![superseding_lesson_id.to_ustr()],
        })),
    )?;

    // Every exercise ID in the superseding lesson should be in `simulation.answer_history`.
    let exercise_ids = all_test_exercises(&LIBRARY);
    for exercise_id in &exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if exercise_id.exercise_in_lesson(&superseding_lesson_id) {
            assert!(
                simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_simulation_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
        } else {
            assert!(
                !simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should not have been scheduled",
                exercise_id
            );
        }
    }

    // Run the simulation again with no lesson filter.
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
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::One)));
    simulation.run_simulation(
        &mut trane,
        &vec![],
        Some(ExerciseFilter::UnitFilter(UnitFilter::LessonFilter {
            lesson_ids: vec![superseding_lesson_id.to_ustr()],
        })),
    )?;

    // Run the simulation again with no lesson filter and giving a score of 5 to all exercises.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // This time around, all the exercises in the superseded lesson should have been scheduled.
    for exercise_id in &exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if exercise_id.exercise_in_lesson(&superseded_lesson_id) {
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
