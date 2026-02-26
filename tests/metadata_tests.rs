//! End-to-end tests for verifying the correctness of Trane with metadata filtering.
//!
//! For a more detailed explanation of the testing methodology, see the explanation in the
//! basic_tests module.

use anyhow::{Ok, Result};
use std::{collections::BTreeMap, sync::LazyLock};
use tempfile::TempDir;
use trane::{
    course_library::CourseLibrary,
    data::{
        filter::{ExerciseFilter, FilterOp, FilterType, KeyValueFilter, UnitFilter},
        MasteryScore,
    },
    scheduler::ExerciseScheduler,
    test_utils::*,
};

/// A simple set of courses to test the basic functionality of Trane.
static LIBRARY: LazyLock<Vec<TestCourse>> = LazyLock::new(|| {
    vec![
        TestCourse {
            id: TestId(0, None, None),
            dependencies: vec![],
            encompassed: vec![],
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
                    encompassed: vec![],
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
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(0, Some(1), None),
                    dependencies: vec![TestId(0, Some(0), None)],
                    encompassed: vec![],
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
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(1, None, None),
            dependencies: vec![TestId(0, None, None)],
            encompassed: vec![],
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
                    encompassed: vec![],
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
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(1, Some(1), None),
                    dependencies: vec![TestId(1, Some(0), None)],
                    encompassed: vec![],
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
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(2, None, None),
            dependencies: vec![TestId(0, None, None)],
            encompassed: vec![],
            superseded: vec![],
            metadata: BTreeMap::from([
                (
                    "course_key_1".to_string(),
                    vec!["course_key_1:value_2".to_string()],
                ),
                (
                    "course_key_2".to_string(),
                    vec!["course_key_2:value_2".to_string()],
                ),
            ]),
            lessons: vec![
                TestLesson {
                    id: TestId(2, Some(0), None),
                    dependencies: vec![],
                    encompassed: vec![],
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
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(2, Some(1), None),
                    dependencies: vec![TestId(2, Some(0), None)],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_4".to_string()],
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_4".to_string()],
                        ),
                    ]),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(2, Some(2), None),
                    dependencies: vec![TestId(2, Some(1), None)],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_4".to_string()],
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_4".to_string()],
                        ),
                    ]),
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(4, None, None),
            dependencies: vec![],
            encompassed: vec![],
            superseded: vec![],
            metadata: BTreeMap::from([
                (
                    "course_key_1".to_string(),
                    vec!["course_key_1:value_3".to_string()],
                ),
                (
                    "course_key_2".to_string(),
                    vec!["course_key_2:value_3".to_string()],
                ),
            ]),
            lessons: vec![
                TestLesson {
                    id: TestId(4, Some(0), None),
                    dependencies: vec![],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_5".to_string()],
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_5".to_string()],
                        ),
                    ]),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(4, Some(1), None),
                    dependencies: vec![TestId(4, Some(0), None)],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_6".to_string()],
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_6".to_string()],
                        ),
                    ]),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(4, Some(2), None),
                    dependencies: vec![TestId(4, Some(0), None)],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_5".to_string()],
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_5".to_string()],
                        ),
                    ]),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(4, Some(3), None),
                    dependencies: vec![TestId(4, Some(2), None)],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_5".to_string()],
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_5".to_string()],
                        ),
                    ]),
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(5, None, None),
            dependencies: vec![TestId(3, None, None), TestId(4, None, None)],
            encompassed: vec![],
            superseded: vec![],
            metadata: BTreeMap::from([
                (
                    "course_key_1".to_string(),
                    vec!["course_key_1:value_2".to_string()],
                ),
                (
                    "course_key_2".to_string(),
                    vec!["course_key_2:value_2".to_string()],
                ),
            ]),
            lessons: vec![
                TestLesson {
                    id: TestId(5, Some(0), None),
                    dependencies: vec![TestId(4, Some(1), None)],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_4".to_string()],
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_4".to_string()],
                        ),
                    ]),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(5, Some(1), None),
                    dependencies: vec![TestId(5, Some(0), None), TestId(3, Some(3), None)],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::from([
                        (
                            "lesson_key_1".to_string(),
                            vec!["lesson_key_1:value_5".to_string()],
                        ),
                        (
                            "lesson_key_2".to_string(),
                            vec!["lesson_key_2:value_5".to_string()],
                        ),
                    ]),
                    num_exercises: 10,
                },
            ],
        },
    ]
});

/// A small library used to verify dependency bridging with metadata filters.
static BRIDGE_LIBRARY: LazyLock<Vec<TestCourse>> = LazyLock::new(|| {
    vec![
        TestCourse {
            id: TestId(0, None, None),
            dependencies: vec![],
            encompassed: vec![],
            superseded: vec![],
            metadata: BTreeMap::new(),
            lessons: vec![TestLesson {
                id: TestId(0, Some(0), None),
                dependencies: vec![],
                encompassed: vec![],
                superseded: vec![],
                metadata: BTreeMap::from([(
                    "bridge_key".to_string(),
                    vec!["bridge_key:keep".to_string()],
                )]),
                num_exercises: 1,
            }],
        },
        TestCourse {
            id: TestId(1, None, None),
            dependencies: vec![],
            encompassed: vec![],
            superseded: vec![],
            metadata: BTreeMap::new(),
            lessons: vec![TestLesson {
                id: TestId(1, Some(0), None),
                dependencies: vec![TestId(0, Some(0), None)],
                encompassed: vec![],
                superseded: vec![],
                metadata: BTreeMap::new(),
                num_exercises: 1,
            }],
        },
        TestCourse {
            id: TestId(2, None, None),
            dependencies: vec![],
            encompassed: vec![],
            superseded: vec![],
            metadata: BTreeMap::new(),
            lessons: vec![TestLesson {
                id: TestId(2, Some(0), None),
                dependencies: vec![TestId(1, Some(0), None)],
                encompassed: vec![],
                superseded: vec![],
                metadata: BTreeMap::new(),
                num_exercises: 1,
            }],
        },
        TestCourse {
            id: TestId(3, None, None),
            dependencies: vec![],
            encompassed: vec![],
            superseded: vec![],
            metadata: BTreeMap::new(),
            lessons: vec![TestLesson {
                id: TestId(3, Some(0), None),
                dependencies: vec![TestId(2, Some(0), None)],
                encompassed: vec![],
                superseded: vec![],
                metadata: BTreeMap::from([(
                    "bridge_key".to_string(),
                    vec!["bridge_key:keep".to_string()],
                )]),
                num_exercises: 1,
            }],
        },
    ]
});

/// A small library used to verify course dependency bridging behavior with metadata filters.
///
/// It covers two cases:
/// 1. A filtered-out course dependency that has matching lessons in-course.
/// 2. A filtered-out course dependency with no matching lessons that depends on a lesson from
///    another course.
static BRIDGE_COURSE_LIBRARY: LazyLock<Vec<TestCourse>> = LazyLock::new(|| {
    vec![
        TestCourse {
            id: TestId(0, None, None),
            dependencies: vec![],
            encompassed: vec![],
            superseded: vec![],
            metadata: BTreeMap::new(),
            lessons: vec![
                TestLesson {
                    id: TestId(0, Some(0), None),
                    dependencies: vec![],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::from([(
                        "bridge_key".to_string(),
                        vec!["bridge_key:keep".to_string()],
                    )]),
                    num_exercises: 1,
                },
                TestLesson {
                    id: TestId(0, Some(1), None),
                    dependencies: vec![TestId(0, Some(0), None)],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::from([(
                        "bridge_key".to_string(),
                        vec!["bridge_key:keep".to_string()],
                    )]),
                    num_exercises: 1,
                },
            ],
        },
        TestCourse {
            id: TestId(1, None, None),
            dependencies: vec![],
            encompassed: vec![],
            superseded: vec![],
            metadata: BTreeMap::new(),
            lessons: vec![TestLesson {
                id: TestId(1, Some(0), None),
                dependencies: vec![TestId(0, None, None)],
                encompassed: vec![],
                superseded: vec![],
                metadata: BTreeMap::from([(
                    "bridge_key".to_string(),
                    vec!["bridge_key:keep".to_string()],
                )]),
                num_exercises: 1,
            }],
        },
        TestCourse {
            id: TestId(2, None, None),
            dependencies: vec![],
            encompassed: vec![],
            superseded: vec![],
            metadata: BTreeMap::new(),
            lessons: vec![TestLesson {
                id: TestId(2, Some(0), None),
                dependencies: vec![],
                encompassed: vec![],
                superseded: vec![],
                metadata: BTreeMap::from([(
                    "bridge_key".to_string(),
                    vec!["bridge_key:keep".to_string()],
                )]),
                num_exercises: 1,
            }],
        },
        TestCourse {
            id: TestId(3, None, None),
            dependencies: vec![TestId(2, Some(0), None)],
            encompassed: vec![],
            superseded: vec![],
            metadata: BTreeMap::new(),
            lessons: vec![TestLesson {
                id: TestId(3, Some(0), None),
                dependencies: vec![],
                encompassed: vec![],
                superseded: vec![],
                metadata: BTreeMap::new(),
                num_exercises: 1,
            }],
        },
        TestCourse {
            id: TestId(4, None, None),
            dependencies: vec![],
            encompassed: vec![],
            superseded: vec![],
            metadata: BTreeMap::new(),
            lessons: vec![TestLesson {
                id: TestId(4, Some(0), None),
                dependencies: vec![TestId(3, None, None)],
                encompassed: vec![],
                superseded: vec![],
                metadata: BTreeMap::from([(
                    "bridge_key".to_string(),
                    vec!["bridge_key:keep".to_string()],
                )]),
                num_exercises: 1,
            }],
        },
    ]
});

///  Verifies that only exercises in units that match the metadata filter using the logical op All
/// are scheduled.
#[test]
fn scheduler_respects_metadata_filter_op_all() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    let filter = UnitFilter::MetadataFilter {
        filter: KeyValueFilter::CombinedFilter {
            op: FilterOp::All,
            filters: vec![
                KeyValueFilter::CourseFilter {
                    filter_type: FilterType::Include,
                    key: "course_key_1".to_string(),
                    value: "course_key_1:value_2".to_string(),
                },
                KeyValueFilter::LessonFilter {
                    filter_type: FilterType::Include,
                    key: "lesson_key_2".to_string(),
                    value: "lesson_key_2:value_4".to_string(),
                },
            ],
        },
    };
    simulation.run_simulation(
        &mut trane,
        &vec![],
        &Some(ExerciseFilter::UnitFilter(filter)),
    )?;

    // Only exercises in the lessons that match the metadata filters should be scheduled.
    let matching_lessons = [
        TestId(2, Some(1), None),
        TestId(2, Some(2), None),
        TestId(5, Some(0), None),
    ];
    let exercise_ids = all_test_exercises(&LIBRARY);
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
            assert_simulation_scores(exercise_ustr, &trane, &simulation.answer_history)?;
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

/// Verifies that only exercises in units that match the metadata filter using the logical op Any
/// are scheduled.
#[test]
fn scheduler_respects_metadata_filter_op_any() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    let filter = UnitFilter::MetadataFilter {
        filter: KeyValueFilter::CombinedFilter {
            op: FilterOp::Any,
            filters: vec![
                KeyValueFilter::CourseFilter {
                    filter_type: FilterType::Include,
                    key: "course_key_1".to_string(),
                    value: "course_key_1:value_2".to_string(),
                },
                KeyValueFilter::LessonFilter {
                    filter_type: FilterType::Include,
                    key: "lesson_key_2".to_string(),
                    value: "lesson_key_2:value_4".to_string(),
                },
            ],
        },
    };
    simulation.run_simulation(
        &mut trane,
        &vec![],
        &Some(ExerciseFilter::UnitFilter(filter)),
    )?;

    // Only exercises in the lessons that match the metadata filters should be scheduled.
    let matching_lessons = [
        TestId(2, Some(0), None),
        TestId(2, Some(1), None),
        TestId(2, Some(2), None),
        TestId(5, Some(0), None),
        TestId(5, Some(1), None),
    ];
    let exercise_ids = all_test_exercises(&LIBRARY);
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
            assert_simulation_scores(exercise_ustr, &trane, &simulation.answer_history)?;
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

/// Verifies that only exercises in units that match the lesson metadata filter are scheduled.
#[test]
fn scheduler_respects_lesson_metadata_filter() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    let filter = UnitFilter::MetadataFilter {
        filter: KeyValueFilter::LessonFilter {
            filter_type: FilterType::Include,
            key: "lesson_key_2".to_string(),
            value: "lesson_key_2:value_4".to_string(),
        },
    };
    simulation.run_simulation(
        &mut trane,
        &vec![],
        &Some(ExerciseFilter::UnitFilter(filter)),
    )?;

    // Only exercises in the lessons that match the metadata filters should be scheduled.
    let matching_lessons = [
        TestId(2, Some(1), None),
        TestId(2, Some(2), None),
        TestId(5, Some(0), None),
    ];
    let exercise_ids = all_test_exercises(&LIBRARY);
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
            assert_simulation_scores(exercise_ustr, &trane, &simulation.answer_history)?;
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

/// Verifies that only exercises in units that match the course metadata filter are scheduled.
#[test]
fn scheduler_respects_course_metadata_filter() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    let filter = UnitFilter::MetadataFilter {
        filter: KeyValueFilter::CourseFilter {
            filter_type: FilterType::Include,
            key: "course_key_1".to_string(),
            value: "course_key_1:value_2".to_string(),
        },
    };
    simulation.run_simulation(
        &mut trane,
        &vec![],
        &Some(ExerciseFilter::UnitFilter(filter)),
    )?;

    // Only exercises in the lessons that match the metadata filters should be scheduled.
    let matching_courses = [TestId(2, None, None), TestId(5, None, None)];
    let exercise_ids = all_test_exercises(&LIBRARY);
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
            assert_simulation_scores(exercise_ustr, &trane, &simulation.answer_history)?;
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

/// Verifies that only exercises in units that match the metadata filter are scheduled but that they
/// are ignored if they are in the blacklist.
#[test]
fn scheduler_respects_metadata_filter_and_blacklist() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    let filter = UnitFilter::MetadataFilter {
        filter: KeyValueFilter::CombinedFilter {
            op: FilterOp::All,
            filters: vec![
                KeyValueFilter::CourseFilter {
                    filter_type: FilterType::Include,
                    key: "course_key_1".to_string(),
                    value: "course_key_1:value_2".to_string(),
                },
                KeyValueFilter::LessonFilter {
                    filter_type: FilterType::Include,
                    key: "lesson_key_2".to_string(),
                    value: "lesson_key_2:value_4".to_string(),
                },
            ],
        },
    };
    let blacklist = vec![TestId(2, None, None)];
    simulation.run_simulation(
        &mut trane,
        &blacklist,
        &Some(ExerciseFilter::UnitFilter(filter)),
    )?;

    // Only exercises in the lessons that match the metadata filters should be scheduled.
    let matching_lessons = [TestId(5, Some(0), None)];
    let exercise_ids = all_test_exercises(&LIBRARY);
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
            assert_simulation_scores(exercise_ustr, &trane, &simulation.answer_history)?;
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

/// Verifies that metadata filtering bridges filtered dependencies and still gates traversal.
#[test]
fn scheduler_bridges_filtered_dependency_chain() -> Result<()> {
    // Initialize a small chain where only the first and last lessons pass the metadata filter.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(temp_dir.path(), &BRIDGE_LIBRARY)?;

    // Build the metadata filter that selects only chain endpoints.
    let filter = ExerciseFilter::UnitFilter(UnitFilter::MetadataFilter {
        filter: KeyValueFilter::LessonFilter {
            filter_type: FilterType::Include,
            key: "bridge_key".to_string(),
            value: "bridge_key:keep".to_string(),
        },
    });

    // Run the first simulation with low scores so upstream dependencies never pass.
    let a_lesson = TestId(0, Some(0), None).to_ustr();
    let mut simulation = TraneSimulation::new(400, Box::new(|_| Some(MasteryScore::One)));
    simulation.run_simulation(&mut trane, &vec![], &Some(filter.clone()))?;

    // Verify that the upstream lesson is present and remains below passing score, and that the
    // downstream endpoint remains blocked.
    let a_exercise = TestId(0, Some(0), Some(0)).to_ustr();
    let d_exercise = TestId(3, Some(0), Some(0)).to_ustr();
    let passing_score = trane.get_scheduler_options().passing_score_v2.min_score;
    assert!(
        trane.get_exercise_ids(a_lesson).unwrap_or_default().len() > 0,
        "lesson {:?} should contain at least one exercise",
        a_lesson
    );
    let a_lesson_score = trane.get_unit_score(a_lesson)?;
    assert!(
        a_lesson_score.is_some(),
        "lesson {:?} should have a valid score",
        a_lesson
    );
    assert!(
        a_lesson_score.unwrap_or_default() < passing_score,
        "lesson {:?} should remain below passing score with only low answers",
        a_lesson
    );
    assert!(
        simulation.answer_history.contains_key(&a_exercise),
        "exercise {:?} should have been scheduled",
        a_exercise
    );
    assert!(
        !simulation.answer_history.contains_key(&d_exercise),
        "exercise {:?} should not have been scheduled when dependencies are never mastered",
        d_exercise
    );

    // Run the second simulation with high scores and verify the downstream endpoint unlocks.
    let mut simulation = TraneSimulation::new(400, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(&mut trane, &vec![], &Some(filter))?;
    assert!(
        simulation.answer_history.contains_key(&d_exercise),
        "exercise {:?} should have been scheduled after dependencies are mastered",
        d_exercise
    );
    Ok(())
}

/// Verifies that filtered-out course dependencies bridge to the last matching lesson in the
/// course, and that courses depending on lessons in other courses are also bridged correctly.
#[test]
fn scheduler_bridges_filtered_course_dependencies() -> Result<()> {
    // Initialize a library that exercises both in-course and cross-course dependency bridging.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(temp_dir.path(), &BRIDGE_COURSE_LIBRARY)?;

    // Build the metadata filter used by both simulation passes.
    let filter = ExerciseFilter::UnitFilter(UnitFilter::MetadataFilter {
        filter: KeyValueFilter::LessonFilter {
            filter_type: FilterType::Include,
            key: "bridge_key".to_string(),
            value: "bridge_key:keep".to_string(),
        },
    });

    // Run the first simulation with selective mastery so dependents stay blocked.
    // Keep the first matching lesson in course 0 mastered while leaving the last one unmastered.
    // Dependents of that filtered-out course must stay blocked.
    let mut simulation = TraneSimulation::new(
        500,
        Box::new(|id| {
            if id.starts_with("0::0::") {
                Some(MasteryScore::Five)
            } else {
                Some(MasteryScore::One)
            }
        }),
    );
    simulation.run_simulation(&mut trane, &vec![], &Some(filter.clone()))?;

    // Verify that both the in-course and cross-course dependents are blocked.
    let course_dependent_exercise = TestId(1, Some(0), Some(0)).to_ustr();
    let external_course_dependent_exercise = TestId(4, Some(0), Some(0)).to_ustr();
    assert!(
        !simulation
            .answer_history
            .contains_key(&course_dependent_exercise),
        "exercise {:?} should not have been scheduled while the last matching lesson is unmastered",
        course_dependent_exercise
    );
    assert!(
        !simulation
            .answer_history
            .contains_key(&external_course_dependent_exercise),
        "exercise {:?} should not have been scheduled while the external lesson dependency is unmastered",
        external_course_dependent_exercise
    );

    // Run the second simulation with high scores and verify both dependents become reachable.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(&mut trane, &vec![], &Some(filter))?;
    assert!(
        simulation
            .answer_history
            .contains_key(&course_dependent_exercise),
        "exercise {:?} should have been scheduled after course dependency was satisfied",
        course_dependent_exercise
    );
    assert!(
        simulation
            .answer_history
            .contains_key(&external_course_dependent_exercise),
        "exercise {:?} should have been scheduled after external lesson dependency was satisfied",
        external_course_dependent_exercise
    );
    Ok(())
}

/// Verifies searching for courses via metadata in the course library.
#[test]
fn course_library_search_course_metadata() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Search for a course's metadata.
    let search_results = trane.search("\"course_key_2:value_2\"")?;
    let expected_id = TestId(2, None, None).to_ustr();
    assert!(search_results.contains(&expected_id));
    Ok(())
}

/// Verifies searching for lessons via metadata in the course library.
#[test]
fn course_library_search_lesson_metadata() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Search for a lesson's metadata.
    let search_results = trane.search("\"lesson_key_2:value_4\"")?;
    let expected_id = TestId(2, Some(1), None).to_ustr();
    assert!(search_results.contains(&expected_id));
    Ok(())
}
