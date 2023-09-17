//! End-to-end tests for verifying the correctness of Trane with blacklisted courses, lessons, and
//! exercises.
//!
//! For a more detailed explanation of the testing methodology, see the explanation in the
//! basic_tests module.

use std::collections::BTreeMap;

use anyhow::{Ok, Result};
use lazy_static::lazy_static;
use tempfile::TempDir;
use trane::{blacklist::Blacklist, data::MasteryScore, testutil::*};

lazy_static! {
    /// A simple set of courses to verify that blacklisting works correctly.
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
            superseded: vec![],
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
            dependencies: vec![TestId(0, None, None)],
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
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(3, None, None),
            dependencies: vec![TestId(1, None, None)],
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
    ];
}

/// Verifies that all the exercises are scheduled except for those belonging to the courses in the
/// blacklist.
#[test]
fn avoid_scheduling_courses_in_blacklist() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(&temp_dir.path(), &LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    let course_blacklist = vec![TestId(0, None, None), TestId(3, None, None)];
    simulation.run_simulation(&mut trane, &course_blacklist, None)?;

    // Every exercise ID should be in `simulation.answer_history` except for those which belong to
    // courses in the blacklist.
    let exercise_ids = all_test_exercises(&LIBRARY);
    for exercise_id in exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if !course_blacklist
            .iter()
            .any(|course_id| exercise_id.exercise_in_course(&course_id))
        {
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
    Ok(())
}

/// Verifies that all the exercises are scheduled except for those belonging to the lessons in the
/// blacklist.
#[test]
fn avoid_scheduling_lessons_in_blacklist() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(&temp_dir.path(), &LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    let lesson_blacklist = vec![TestId(0, Some(1), None), TestId(3, Some(0), None)];
    simulation.run_simulation(&mut trane, &lesson_blacklist, None)?;

    // Every exercise ID should be in `simulation.answer_history` except for those which belong to
    // lessons in the blacklist.
    let exercise_ids = all_test_exercises(&LIBRARY);
    for exercise_id in exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if !lesson_blacklist
            .iter()
            .any(|lesson_id| exercise_id.exercise_in_lesson(&lesson_id))
        {
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
    Ok(())
}

/// Verifies that all the exercises are scheduled except for those in the blacklist.
#[test]
fn avoid_scheduling_exercises_in_blacklist() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(&temp_dir.path(), &LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    let exercise_blacklist = vec![
        TestId(2, Some(1), Some(0)),
        TestId(2, Some(1), Some(1)),
        TestId(2, Some(1), Some(2)),
        TestId(2, Some(1), Some(3)),
        TestId(2, Some(1), Some(4)),
        TestId(2, Some(1), Some(5)),
        TestId(2, Some(1), Some(6)),
        TestId(2, Some(1), Some(7)),
        TestId(2, Some(1), Some(8)),
        TestId(2, Some(1), Some(9)),
    ];
    simulation.run_simulation(&mut trane, &exercise_blacklist, None)?;

    // Every exercise ID should be in `simulation.answer_history` except for those in the blacklist.
    let exercise_ids = all_test_exercises(&LIBRARY);
    for exercise_id in exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if !exercise_blacklist
            .iter()
            .any(|blacklisted_id| *blacklisted_id == exercise_id)
        {
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
    Ok(())
}

/// Verifies that the score cache is invalidated when the blacklist is updated.
#[test]
fn invalidate_cache_on_blacklist_update() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(&temp_dir.path(), &LIBRARY)?;

    // Run the simulation with a valid blacklist and give each exercise a score of 5.
    // All exercises except for those in the blacklist should be scheduled.
    let exercise_blacklist = vec![
        TestId(0, Some(0), Some(0)),
        TestId(0, Some(0), Some(1)),
        TestId(0, Some(0), Some(2)),
        TestId(0, Some(0), Some(3)),
        TestId(0, Some(0), Some(4)),
        TestId(0, Some(0), Some(5)),
        TestId(0, Some(0), Some(6)),
        TestId(0, Some(0), Some(7)),
        TestId(0, Some(0), Some(8)),
        TestId(0, Some(0), Some(9)),
        TestId(0, Some(1), Some(0)),
        TestId(0, Some(1), Some(1)),
        TestId(0, Some(1), Some(2)),
        TestId(0, Some(1), Some(3)),
        TestId(0, Some(1), Some(4)),
        TestId(0, Some(1), Some(5)),
        TestId(0, Some(1), Some(6)),
        TestId(0, Some(1), Some(7)),
        TestId(0, Some(1), Some(8)),
        TestId(0, Some(1), Some(9)),
    ];
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(&mut trane, &exercise_blacklist, None)?;

    // Every blacklisted exercise should not have been scheduled.
    let exercise_ids = all_test_exercises(&LIBRARY);
    for exercise_id in &exercise_ids {
        if exercise_blacklist
            .iter()
            .any(|blacklisted_id| *blacklisted_id == *exercise_id)
        {
            let exercise_ustr = exercise_id.to_ustr();
            assert!(
                !simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should not have been scheduled",
                exercise_id
            );
        } else {
            assert!(
                simulation
                    .answer_history
                    .contains_key(&exercise_id.to_ustr()),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
        }
    }

    // Remove those units from the blacklist and re-run the simulation, but this time assign a score
    // of one to all exercises.
    for exercise_id in &exercise_blacklist {
        trane.remove_from_blacklist(&exercise_id.to_ustr())?;
    }
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::One)));
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // Trane should not schedule any lesson or course depending on the lesson with ID `TestId(0,
    // Some(0), None)`.
    let unscheduled_lessons = vec![
        TestId(0, Some(1), None),
        TestId(1, Some(0), None),
        TestId(1, Some(1), None),
        TestId(2, Some(0), None),
        TestId(2, Some(1), None),
        TestId(2, Some(2), None),
        TestId(3, Some(0), None),
        TestId(3, Some(1), None),
    ];
    for exercise_id in &exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if exercise_id.exercise_in_lesson(&TestId(0, Some(0), None)) {
            // The first unit scheduled by Trane. Since all the scores are 1, Trane should not move
            // past this unit.
            assert!(
                simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
        } else if unscheduled_lessons
            .iter()
            .any(|lesson_id| exercise_id.exercise_in_lesson(&lesson_id))
        {
            // None of the units depending on lesson `TestId(0, Some(0), None)` should have been
            // scheduled.
            assert!(
                !simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should not have been scheduled",
                exercise_id
            );
        }
    }

    // Re-run the first simulation with the same blacklist and verify that the blacklisted exercises
    // are not scheduled anymore.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(&mut trane, &exercise_blacklist, None)?;

    // Every blacklisted exercise should not have been scheduled.
    for exercise_id in &exercise_blacklist {
        let exercise_ustr = exercise_id.to_ustr();
        assert!(
            !simulation.answer_history.contains_key(&exercise_ustr),
            "exercise {:?} should not have been scheduled",
            exercise_id
        );
    }
    Ok(())
}
