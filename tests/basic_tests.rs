mod common;

use std::collections::BTreeMap;

use anyhow::{Ok, Result};
use lazy_static::lazy_static;
use tempfile::TempDir;
use trane::{
    blacklist::Blacklist,
    data::{
        filter::{FilterOp, FilterType, KeyValueFilter, MetadataFilter, UnitFilter},
        MasteryScore,
    },
};

use crate::common::*;

lazy_static! {
    /// A simple set of courses to test the basic functionality of Trane.
    static ref BASIC_LIBRARY: Vec<TestCourse> = vec![
        TestCourse {
            id: TestId(0, None, None),
            dependencies: vec![],
            metadata: BTreeMap::from([
                (
                    "course_key_1".to_string(),
                    vec!["course_key_1:value_1".to_string()]
                ),
                (
                    "course_key_2".to_string(),
                    vec!["course_key_2:value_1".to_string()]
                ),
            ]),
            lessons: vec![
                TestLesson {
                    id: TestId(0, Some(0), None),
                    dependencies: vec![],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_1".to_string()]
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_1".to_string()]
                        ),
                    ]),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(0, Some(1), None),
                    dependencies: vec![TestId(0, Some(0), None)],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_2".to_string()]
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_2".to_string()]
                        ),
                    ]),
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(1, None, None),
            dependencies: vec![TestId(0, None, None)],
            metadata: BTreeMap::from([
                (
                    "course_key_1".to_string(),
                    vec!["course_key_1:value_1".to_string()]
                ),
                (
                    "course_key_2".to_string(),
                    vec!["course_key_2:value_1".to_string()]
                ),
            ]),
            lessons: vec![
                TestLesson {
                    id: TestId(1, Some(0), None),
                    dependencies: vec![],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_3".to_string()]
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_3".to_string()]
                        ),
                    ]),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(1, Some(1), None),
                    dependencies: vec![TestId(1, Some(0), None)],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_3".to_string()]
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_3".to_string()]
                        ),
                    ]),
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(2, None, None),
            dependencies: vec![TestId(0, None, None)],
            metadata: BTreeMap::from([
                (
                    "course_key_1".to_string(),
                    vec!["course_key_1:value_2".to_string()]
                ),
                (
                    "course_key_2".to_string(),
                    vec!["course_key_2:value_2".to_string()]
                ),
            ]),
            lessons: vec![
                TestLesson {
                    id: TestId(2, Some(0), None),
                    dependencies: vec![],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_3".to_string()]
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_3".to_string()]
                        ),
                    ]),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(2, Some(1), None),
                    dependencies: vec![TestId(2, Some(0), None)],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_4".to_string()]
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_4".to_string()]
                        ),
                    ]),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(2, Some(2), None),
                    dependencies: vec![TestId(2, Some(1), None)],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_4".to_string()]
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_4".to_string()]
                        ),
                    ]),
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(4, None, None),
            dependencies: vec![],
            metadata: BTreeMap::from([
                (
                    "course_key_1".to_string(),
                    vec!["course_key_1:value_3".to_string()]
                ),
                (
                    "course_key_2".to_string(),
                    vec!["course_key_2:value_3".to_string()]
                ),
            ]),
            lessons: vec![
                TestLesson {
                    id: TestId(4, Some(0), None),
                    dependencies: vec![],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_5".to_string()]
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_5".to_string()]
                        ),
                    ]),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(4, Some(1), None),
                    dependencies: vec![TestId(4, Some(0), None)],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_6".to_string()]
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_6".to_string()]
                        ),
                    ]),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(4, Some(2), None),
                    dependencies: vec![TestId(4, Some(0), None)],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_5".to_string()]
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_5".to_string()]
                        ),
                    ]),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(4, Some(3), None),
                    dependencies: vec![TestId(4, Some(2), None)],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_5".to_string()]
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_5".to_string()]
                        ),
                    ]),
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(5, None, None),
            dependencies: vec![TestId(3, None, None), TestId(4, None, None)],
            metadata: BTreeMap::from([
                (
                    "course_key_1".to_string(),
                    vec!["course_key_1:value_2".to_string()]
                ),
                (
                    "course_key_2".to_string(),
                    vec!["course_key_2:value_2".to_string()]
                ),
            ]),
            lessons: vec![
                TestLesson {
                    id: TestId(5, Some(0), None),
                    dependencies: vec![],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_4".to_string()]
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_4".to_string()]
                        ),
                    ]),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(5, Some(1), None),
                    dependencies: vec![TestId(5, Some(0), None)],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_5".to_string()]
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_5".to_string()]
                        ),
                    ]),
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(6, None, None),
            dependencies: vec![TestId(3, None, None)],
            metadata: BTreeMap::from([
                (
                    "course_key_1".to_string(),
                    vec!["course_key_1:value_6".to_string()]
                ),
                (
                    "course_key_2".to_string(),
                    vec!["course_key_2:value_6".to_string()]
                ),
            ]),
            lessons: vec![
                TestLesson {
                    id: TestId(6, Some(0), None),
                    dependencies: vec![],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_6".to_string()]
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_6".to_string()]
                        ),
                    ]),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(6, Some(1), None),
                    dependencies: vec![TestId(6, Some(0), None)],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_7".to_string()]
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_7".to_string()]
                        ),
                    ]),
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(7, None, None),
            dependencies: vec![],
            metadata: BTreeMap::from([
                (
                    "course_key_1".to_string(),
                    vec!["course_key_1:value_1".to_string()]
                ),
                (
                    "course_key_2".to_string(),
                    vec!["course_key_2:value_1".to_string()]
                ),
            ]),
            lessons: vec![
                TestLesson {
                    id: TestId(7, Some(0), None),
                    dependencies: vec![TestId(0, None, None)],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_1".to_string()]
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_1".to_string()]
                        ),
                    ]),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(7, Some(1), None),
                    dependencies: vec![TestId(0, Some(0), None)],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_2".to_string()]
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_2".to_string()]
                        ),
                    ]),
                    num_exercises: 10,
                },
            ],
        },
    ];
}

/// A test that verifies that all the exercises are scheduled with no blacklist or filter when the
/// user gives a score of five to every exercise.
#[test]
fn all_exercises_scheduled() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_trane(&temp_dir.path().to_path_buf(), &BASIC_LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // Every exercise ID should be in simulation.answer_history.
    let exercise_ids = all_exercises(&BASIC_LIBRARY);
    for exercise_id in exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        assert!(
            simulation.answer_history.contains_key(&exercise_ustr),
            "exercise {:?} should have been scheduled",
            exercise_id
        );
        assert_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
    }
    Ok(())
}

/// A test that verifies no exercises past the first course and lesson are scheduled when the user
/// scores every exercise in that lesson with a mastery score of one.
#[test]
fn bad_score_prevents_advancing() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let mut trane = init_trane(&temp_dir.path().to_path_buf(), &BASIC_LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(100, Box::new(|_| Some(MasteryScore::One)));
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // Only the exercises in the first lessons should be in simulation.answer_history.
    let first_lessons = vec![
        TestId(0, Some(0), None),
        TestId(4, Some(0), None),
        TestId(6, Some(0), None),
    ];
    let exercise_ids = all_exercises(&BASIC_LIBRARY);
    for exercise_id in exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if first_lessons
            .iter()
            .any(|lesson| exercise_id.exercise_in_lesson(&lesson))
        {
            assert!(
                simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
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

/// A test that verifies that all the exercises are scheduled except for those belonging to the
/// courses in the blacklist.
#[test]
fn avoid_scheduling_courses_in_blacklist() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_trane(&temp_dir.path().to_path_buf(), &BASIC_LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    let course_blacklist = vec![TestId(0, None, None), TestId(4, None, None)];
    simulation.run_simulation(&mut trane, &course_blacklist, None)?;

    // Every exercise ID should be in simulation.answer_history except for those which belong to
    // courses in the blacklist.
    let exercise_ids = all_exercises(&BASIC_LIBRARY);
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
            assert_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
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

/// A test that verifies that all the exercises are scheduled except for those belonging to the
/// lessons in the blacklist.
#[test]
fn avoid_scheduling_lessons_in_blacklist() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_trane(&temp_dir.path().to_path_buf(), &BASIC_LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    let lesson_blacklist = vec![TestId(0, Some(1), None), TestId(4, Some(0), None)];
    simulation.run_simulation(&mut trane, &lesson_blacklist, None)?;

    // Every exercise ID should be in simulation.answer_history except for those which belong to
    // lessons in the blacklist.
    let exercise_ids = all_exercises(&BASIC_LIBRARY);
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
            assert_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
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

/// A test that verifies that all the exercises are scheduled except for those in the blacklist.
#[test]
fn avoid_scheduling_exercises_in_blacklist() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_trane(&temp_dir.path().to_path_buf(), &BASIC_LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    let exercise_blacklist = vec![TestId(2, Some(1), Some(7)), TestId(4, Some(0), Some(0))];
    simulation.run_simulation(&mut trane, &exercise_blacklist, None)?;

    // Every exercise ID should be in simulation.answer_history except for those in the blacklist.
    let exercise_ids = all_exercises(&BASIC_LIBRARY);
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
            assert_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
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

/// A test that verifies that the score cache is invalidated when the blacklist is updated.
#[test]
fn invalidate_cache_on_blacklist_update() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_trane(&temp_dir.path().to_path_buf(), &BASIC_LIBRARY)?;

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
    let exercise_ids = all_exercises(&BASIC_LIBRARY);
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
    // of one to all exercises. Trane should not schedule any lesson or course depending on the
    // lesson with ID TestId(0, Some(0), None).
    for exercise_id in &exercise_blacklist {
        trane.remove_from_blacklist(&exercise_id.to_ustr())?;
    }
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::One)));
    simulation.run_simulation(&mut trane, &vec![], None)?;

    let unscheduled_lessons = vec![
        TestId(0, Some(1), None),
        TestId(1, Some(0), None),
        TestId(1, Some(1), None),
        TestId(2, Some(0), None),
        TestId(2, Some(1), None),
        TestId(2, Some(0), None),
        TestId(7, Some(0), None),
        TestId(7, Some(1), None),
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
            // None of the units depending on lesson TestId(0, Some(0), None) should have been
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

/// A test that verifies only exercises in the given course are scheduled when a course filter is
/// provided.
#[test]
fn scheduler_respects_course_filter() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_trane(&temp_dir.path().to_path_buf(), &BASIC_LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    let selected_courses = vec![TestId(1, None, None), TestId(5, None, None)];
    let course_filter = UnitFilter::CourseFilter {
        course_ids: selected_courses.iter().map(|id| id.to_ustr()).collect(),
    };
    simulation.run_simulation(&mut trane, &vec![], Some(&course_filter))?;

    // Every exercise ID should be in simulation.answer_history.
    let exercise_ids = all_exercises(&BASIC_LIBRARY);
    for exercise_id in exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if selected_courses
            .iter()
            .any(|course_id| exercise_id.exercise_in_course(course_id))
        {
            assert!(
                simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
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

/// A test that verifies only exercises in the given lesson are scheduled when a lesson filter is
/// provided.
#[test]
fn scheduler_respects_lesson_filter() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_trane(&temp_dir.path().to_path_buf(), &BASIC_LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    let selected_lessons = vec![TestId(2, Some(0), None), TestId(4, Some(1), None)];
    let lesson_filter = UnitFilter::LessonFilter {
        lesson_ids: selected_lessons.iter().map(|id| id.to_ustr()).collect(),
    };
    simulation.run_simulation(&mut trane, &vec![], Some(&lesson_filter))?;

    // Every exercise ID should be in simulation.answer_history.
    let exercise_ids = all_exercises(&BASIC_LIBRARY);
    for exercise_id in exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if selected_lessons
            .iter()
            .any(|lesson_id| exercise_id.exercise_in_lesson(lesson_id))
        {
            assert!(
                simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
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

/// A test that verifies that only exercises in units that match the metadata filter using the
/// logical op All are scheduled.
#[test]
fn scheduler_respects_metadata_filter_op_all() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_trane(&temp_dir.path().to_path_buf(), &BASIC_LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    let filter = UnitFilter::MetadataFilter {
        filter: MetadataFilter {
            op: FilterOp::All,
            course_filter: Some(KeyValueFilter::BasicFilter {
                filter_type: FilterType::Include,
                key: "course_key_1".to_string(),
                value: "course_key_1:value_2".to_string(),
            }),
            lesson_filter: Some(KeyValueFilter::BasicFilter {
                filter_type: FilterType::Include,
                key: "lesson_key_2".to_string(),
                value: "lesson_key_2:value_4".to_string(),
            }),
        },
    };
    simulation.run_simulation(&mut trane, &vec![], Some(&filter))?;

    // Only exercises in the lessons that match the metadata filters should be scheduled.
    let matching_lessons = vec![
        TestId(2, Some(1), None),
        TestId(2, Some(2), None),
        TestId(5, Some(0), None),
    ];
    let exercise_ids = all_exercises(&BASIC_LIBRARY);
    for exercise_id in exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if matching_lessons
            .iter()
            .any(|lesson| exercise_id.exercise_in_lesson(lesson))
        {
            assert!(
                simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
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

/// A test that verifies that only exercises in units that match the metadata filter using the
/// logical op Any are scheduled.
#[test]
fn scheduler_respects_metadata_filter_op_any() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_trane(&temp_dir.path().to_path_buf(), &BASIC_LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    let filter = UnitFilter::MetadataFilter {
        filter: MetadataFilter {
            op: FilterOp::Any,
            course_filter: Some(KeyValueFilter::BasicFilter {
                filter_type: FilterType::Include,
                key: "course_key_1".to_string(),
                value: "course_key_1:value_2".to_string(),
            }),
            lesson_filter: Some(KeyValueFilter::BasicFilter {
                filter_type: FilterType::Include,
                key: "lesson_key_2".to_string(),
                value: "lesson_key_2:value_4".to_string(),
            }),
        },
    };
    simulation.run_simulation(&mut trane, &vec![], Some(&filter))?;

    // Only exercises in the lessons that match the metadata filters should be scheduled.
    let matching_lessons = vec![
        TestId(2, Some(0), None),
        TestId(2, Some(1), None),
        TestId(2, Some(2), None),
        TestId(5, Some(0), None),
        TestId(5, Some(1), None),
    ];
    let exercise_ids = all_exercises(&BASIC_LIBRARY);
    for exercise_id in exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if matching_lessons
            .iter()
            .any(|lesson| exercise_id.exercise_in_lesson(lesson))
        {
            assert!(
                simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
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

/// A test that verifies that only exercises in units that match the lesson metadata filter are
/// scheduled.
#[test]
fn scheduler_respects_lesson_metadata_filter() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_trane(&temp_dir.path().to_path_buf(), &BASIC_LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    let filter = UnitFilter::MetadataFilter {
        filter: MetadataFilter {
            op: FilterOp::All,
            course_filter: None,
            lesson_filter: Some(KeyValueFilter::BasicFilter {
                filter_type: FilterType::Include,
                key: "lesson_key_2".to_string(),
                value: "lesson_key_2:value_4".to_string(),
            }),
        },
    };
    simulation.run_simulation(&mut trane, &vec![], Some(&filter))?;

    // Only exercises in the lessons that match the metadata filters should be scheduled.
    let matching_lessons = vec![
        TestId(2, Some(1), None),
        TestId(2, Some(2), None),
        TestId(5, Some(0), None),
    ];
    let exercise_ids = all_exercises(&BASIC_LIBRARY);
    for exercise_id in exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if matching_lessons
            .iter()
            .any(|lesson| exercise_id.exercise_in_lesson(lesson))
        {
            assert!(
                simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
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

/// A test that verifies that only exercises in units that match the course metadata filter are
/// scheduled.
#[test]
fn scheduler_respects_course_metadata_filter() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_trane(&temp_dir.path().to_path_buf(), &BASIC_LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    let filter = UnitFilter::MetadataFilter {
        filter: MetadataFilter {
            op: FilterOp::All,
            course_filter: Some(KeyValueFilter::BasicFilter {
                filter_type: FilterType::Include,
                key: "course_key_1".to_string(),
                value: "course_key_1:value_2".to_string(),
            }),
            lesson_filter: None,
        },
    };
    simulation.run_simulation(&mut trane, &vec![], Some(&filter))?;

    // Only exercises in the lessons that match the metadata filters should be scheduled.
    let matching_courses = vec![TestId(2, None, None), TestId(5, None, None)];
    let exercise_ids = all_exercises(&BASIC_LIBRARY);
    for exercise_id in exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if matching_courses
            .iter()
            .any(|course| exercise_id.exercise_in_course(course))
        {
            assert!(
                simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
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

/// A test that verifies that only exercises in units that match the metadata filter are scheduled
/// but that they are ignored if they are in the blacklist.
#[test]
fn scheduler_respects_metadata_filter_and_blacklist() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_trane(&temp_dir.path().to_path_buf(), &BASIC_LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    let filter = UnitFilter::MetadataFilter {
        filter: MetadataFilter {
            op: FilterOp::All,
            course_filter: Some(KeyValueFilter::BasicFilter {
                filter_type: FilterType::Include,
                key: "course_key_1".to_string(),
                value: "course_key_1:value_2".to_string(),
            }),
            lesson_filter: Some(KeyValueFilter::BasicFilter {
                filter_type: FilterType::Include,
                key: "lesson_key_2".to_string(),
                value: "lesson_key_2:value_4".to_string(),
            }),
        },
    };
    let blacklist = vec![TestId(2, None, None)];
    simulation.run_simulation(&mut trane, &blacklist, Some(&filter))?;

    // Only exercises in the lessons that match the metadata filters should be scheduled.
    let matching_lessons = vec![TestId(5, Some(0), None)];
    let exercise_ids = all_exercises(&BASIC_LIBRARY);
    for exercise_id in exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if matching_lessons
            .iter()
            .any(|lesson| exercise_id.exercise_in_lesson(lesson))
        {
            assert!(
                simulation.answer_history.contains_key(&exercise_ustr),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_scores(&exercise_ustr, &trane, &simulation.answer_history)?;
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
