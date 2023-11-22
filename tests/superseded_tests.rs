//! End-to-end tests for verifying the correctness of Trane with superseded courses and lessons.
//!
//! For a more detailed explanation of the testing methodology, see the explanation in the
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
        TestCourse {
            id: TestId(3, None, None),
            dependencies: vec![],
            superseded: vec![],
            metadata: BTreeMap::default(),
            lessons: vec![
                TestLesson {
                    id: TestId(3, Some(0), None),
                    dependencies: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(3, Some(1), None),
                    dependencies: vec![TestId(3, Some(0), None)],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(4, None, None),
            dependencies: vec![TestId(3, None, None)],
            superseded: vec![TestId(3, None, None)],
            metadata: BTreeMap::default(),
            lessons: vec![
                TestLesson {
                    id: TestId(4, Some(0), None),
                    dependencies: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(4, Some(1), None),
                    dependencies: vec![TestId(4, Some(0), None)],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(5, None, None),
            dependencies: vec![TestId(4, None, None)],
            superseded: vec![TestId(4, None, None)],
            metadata: BTreeMap::default(),
            lessons: vec![
                TestLesson {
                    id: TestId(5, Some(0), None),
                    dependencies: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(5, Some(1), None),
                    dependencies: vec![TestId(5, Some(0), None)],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(6, None, None),
            dependencies: vec![],
            superseded: vec![],
            metadata: BTreeMap::default(),
            lessons: vec![
                TestLesson {
                    id: TestId(6, Some(0), None),
                    dependencies: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(6, Some(1), None),
                    dependencies: vec![TestId(6, Some(0), None)],
                    superseded: vec![TestId(6, Some(0), None)],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(6, Some(2), None),
                    dependencies: vec![TestId(6, Some(1), None)],
                    superseded: vec![TestId(6, Some(1), None)],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(7, None, None),
            dependencies: vec![TestId(6, None, None)],
            superseded: vec![],
            metadata: BTreeMap::default(),
            lessons: vec![
                TestLesson {
                    id: TestId(7, Some(0), None),
                    dependencies: vec![],
                    superseded: vec![],
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
    let mut simulation = TraneSimulation::new(2000, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(&mut trane, &vec![], &None)?;

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
    let mut simulation = TraneSimulation::new(2000, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(&mut trane, &vec![], &None)?;

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
        2000,
        Box::new(|id| {
            if id.starts_with("1::") {
                Some(MasteryScore::One)
            } else {
                Some(MasteryScore::Five)
            }
        }),
    );
    simulation.run_simulation(&mut trane, &vec![], &None)?;

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
    let mut simulation = TraneSimulation::new(2000, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(&mut trane, &vec![], &None)?;

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
    let mut simulation = TraneSimulation::new(2000, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(&mut trane, &vec![], &None)?;

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
        2000,
        Box::new(|id| {
            if id.starts_with("2::2::") {
                Some(MasteryScore::One)
            } else {
                Some(MasteryScore::Five)
            }
        }),
    );
    simulation.run_simulation(&mut trane, &vec![], &None)?;

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

/// Verifies that the superseded courses are dealt with correctly during scheduling when the
/// superseding course is superseded by another unit.
#[test]
fn scheduler_respects_superseded_course_chain() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(&temp_dir.path(), &LIBRARY)?;

    // Run the simulation first giving a score of 5 to all exercises.
    let superseded_course_ids = [TestId(3, None, None), TestId(4, None, None)];
    let mut simulation = TraneSimulation::new(2000, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(&mut trane, &vec![], &None)?;

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
    let mut simulation = TraneSimulation::new(2000, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(&mut trane, &vec![], &None)?;

    // None of the exercises in the superseded courses should have been scheduled.
    for exercise_id in &exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if superseded_course_ids
            .iter()
            .any(|id| exercise_id.exercise_in_course(id))
        {
            assert!(
                !simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should not have been scheduled",
                exercise_id
            );
        }
    }

    // Run the simulation again, but this time give a score of 1 to all exercises in the superseding
    // course at the end of the chain.
    let mut simulation = TraneSimulation::new(
        2000,
        Box::new(|id| {
            if id.starts_with("5::") {
                Some(MasteryScore::One)
            } else {
                Some(MasteryScore::Five)
            }
        }),
    );
    simulation.run_simulation(&mut trane, &vec![], &None)?;

    // This time around, all the exercises in the second superseded course should have been
    // scheduled. The exercises in the first superseded course should not have been scheduled.
    for exercise_id in &exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if exercise_id.exercise_in_course(&superseded_course_ids[1]) {
            assert!(
                simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_simulation_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
        } else if exercise_id.exercise_in_course(&superseded_course_ids[0]) {
            assert!(
                !simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should not have been scheduled",
                exercise_id
            );
        }
    }

    // Run the simulation again, but this time give a score of 1 to all exercises in both
    // superseding courses.
    let mut simulation = TraneSimulation::new(
        2000,
        Box::new(|id| {
            if id.starts_with("4::") || id.starts_with("5::") {
                Some(MasteryScore::One)
            } else {
                Some(MasteryScore::Five)
            }
        }),
    );

    // This time around all exercises in the first superseded course should have been scheduled.
    simulation.run_simulation(&mut trane, &vec![], &None)?;
    for exercise_id in &exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if exercise_id.exercise_in_course(&superseded_course_ids[0]) {
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

/// Verifies that the superseded lessons are dealt with correctly during scheduling when the
/// superseding lesson is superseded by another unit.
#[test]
fn scheduler_respects_superseded_lesson_chain() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(&temp_dir.path(), &LIBRARY)?;

    // Run the simulation first giving a score of 5 to all exercises.
    let superseded_lesson_ids = [TestId(6, Some(0), None), TestId(6, Some(1), None)];
    let mut simulation = TraneSimulation::new(2000, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(&mut trane, &vec![], &None)?;

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
    let mut simulation = TraneSimulation::new(2000, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(&mut trane, &vec![], &None)?;

    // None of the exercises in the superseded lessons should have been scheduled.
    for exercise_id in &exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if superseded_lesson_ids
            .iter()
            .any(|id| exercise_id.exercise_in_lesson(id))
        {
            assert!(
                !simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should not have been scheduled",
                exercise_id
            );
        }
    }

    // Run the simulation again, but this time give a score of 1 to all exercises in the superseding
    // lesson at the end of the chain.
    let mut simulation = TraneSimulation::new(
        2000,
        Box::new(|id| {
            if id.starts_with("6::2::") {
                Some(MasteryScore::One)
            } else {
                Some(MasteryScore::Five)
            }
        }),
    );
    simulation.run_simulation(&mut trane, &vec![], &None)?;

    // This time around, all the exercises in the second superseded lesson should have been
    // scheduled. The exercises in the first superseded lesson should not have been scheduled.
    for exercise_id in &exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if exercise_id.exercise_in_lesson(&superseded_lesson_ids[1]) {
            assert!(
                simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_simulation_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
        } else if exercise_id.exercise_in_lesson(&superseded_lesson_ids[0]) {
            assert!(
                !simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should not have been scheduled",
                exercise_id
            );
        }
    }

    // Run the simulation again, but this time give a score of 1 to all exercises in both
    // superseding lessons.
    let mut simulation = TraneSimulation::new(
        2000,
        Box::new(|id| {
            if id.starts_with("6::1") || id.starts_with("6::2") {
                Some(MasteryScore::One)
            } else {
                Some(MasteryScore::Five)
            }
        }),
    );

    // This time around all exercises in the first superseded lesson should have been scheduled.
    simulation.run_simulation(&mut trane, &vec![], &None)?;
    for exercise_id in &exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if exercise_id.exercise_in_lesson(&superseded_lesson_ids[0]) {
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

/// Verifies that the superseded courses are dealt with correctly during scheduling.
#[test]
fn scheduler_ignores_superseded_exercises() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(&temp_dir.path(), &LIBRARY)?;

    // Run the simulation, filtering only the  exercises in the superseded lessons and giving them a
    // score of 1. This will have the effect of bringing down the average score of all athe
    // exercises in the course.
    let superseded_lesson_ids = [TestId(6, Some(0), None), TestId(6, Some(1), None)];
    let mut simulation = TraneSimulation::new(2000, Box::new(|_| Some(MasteryScore::One)));
    simulation.run_simulation(
        &mut trane,
        &vec![],
        &Some(ExerciseFilter::UnitFilter(UnitFilter::LessonFilter {
            lesson_ids: superseded_lesson_ids
                .iter()
                .map(|id| id.to_ustr())
                .collect(),
        })),
    )?;

    // Verify that all the exercises in the superseded lessons were scheduled.
    let exercise_ids = all_test_exercises(&LIBRARY);
    for exercise_id in &exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if superseded_lesson_ids
            .iter()
            .any(|id| exercise_id.exercise_in_lesson(id))
        {
            assert!(
                simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_simulation_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
        }
    }

    // Run the simulation, filtering only the  exercises in the superseding lesson and giving them a
    // score of four. This will have the effect of bringing up the average score of the course and
    // ensuring the previous lessons are superseded.
    let superseding_lesson_ids = vec![TestId(6, Some(2), None)];
    let mut simulation = TraneSimulation::new(2000, Box::new(|_| Some(MasteryScore::Four)));
    simulation.run_simulation(
        &mut trane,
        &vec![],
        &Some(ExerciseFilter::UnitFilter(UnitFilter::LessonFilter {
            lesson_ids: superseding_lesson_ids
                .iter()
                .map(|id| id.to_ustr())
                .collect(),
        })),
    )?;

    // Verify that all the exercises in the superseding lessons were scheduled.
    let exercise_ids = all_test_exercises(&LIBRARY);
    for exercise_id in &exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if superseding_lesson_ids
            .iter()
            .any(|id| exercise_id.exercise_in_lesson(id))
        {
            assert!(
                simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_simulation_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
        }
    }

    // Run the simulation again, giving a score of 4 to all exercises. If the superseded exercises
    // were ignored correctly, the dependent course should have been scheduled. If they were not
    // ignored correctly, the previous steps ensured the average score of the course is too low to
    // consider the course as mastered.
    let mut simulation = TraneSimulation::new(2000, Box::new(|_| Some(MasteryScore::Four)));
    let dependant_course = TestId(7, None, None);
    simulation.run_simulation(&mut trane, &vec![], &None)?;
    for exercise_id in &exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if exercise_id.exercise_in_course(&dependant_course) {
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
