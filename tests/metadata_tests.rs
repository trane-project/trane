//! End-to-end tests for verifying the correctness of Trane with metadata filtering.
//!
//! For a more detailed explanation of the testing methodology, see the explanation in the
//! basic_tests module.

use std::collections::BTreeMap;

use anyhow::{Ok, Result};
use lazy_static::lazy_static;
use tempfile::TempDir;
use trane::{
    course_library::CourseLibrary,
    data::{
        filter::{ExerciseFilter, FilterOp, FilterType, KeyValueFilter, UnitFilter},
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
                    superseded: vec![],
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
                    superseded: vec![],
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
            superseded: vec![],
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
                    superseded: vec![],
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
                    superseded: vec![],
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
            superseded: vec![],
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
                    superseded: vec![],
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
                    superseded: vec![],
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
                    superseded: vec![],
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
            superseded: vec![],
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
                    superseded: vec![],
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
                    superseded: vec![],
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
                    superseded: vec![],
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
                    superseded: vec![],
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
            dependencies: vec![
                TestId(3, None, None),
                TestId(4, None, None)
            ],
            superseded: vec![],
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
                    dependencies: vec![TestId(4, Some(1), None)],
                    superseded: vec![],
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
                    dependencies: vec![
                        TestId(5, Some(0), None),
                        TestId(3, Some(3), None),
                    ],
                    superseded: vec![],
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
    ];
}

///  Verifies that only exercises in units that match the metadata filter using the logical op All
/// are scheduled.
#[test]
fn scheduler_respects_metadata_filter_op_all() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(&temp_dir.path(), &LIBRARY)?;

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
    let matching_lessons = vec![
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
    let mut trane = init_test_simulation(&temp_dir.path(), &LIBRARY)?;

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
    let matching_lessons = vec![
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
    let mut trane = init_test_simulation(&temp_dir.path(), &LIBRARY)?;

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
    let matching_lessons = vec![
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
    let mut trane = init_test_simulation(&temp_dir.path(), &LIBRARY)?;

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
    let matching_courses = vec![TestId(2, None, None), TestId(5, None, None)];
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
    let mut trane = init_test_simulation(&temp_dir.path(), &LIBRARY)?;

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
    let matching_lessons = vec![TestId(5, Some(0), None)];
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

/// Verifies searching for courses via metadata in the course library.
#[test]
fn course_library_search_course_metadata() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let trane = init_test_simulation(&temp_dir.path(), &LIBRARY)?;

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
    let trane = init_test_simulation(&temp_dir.path(), &LIBRARY)?;

    // Search for a lesson's metadata.
    let search_results = trane.search("\"lesson_key_2:value_4\"")?;
    let expected_id = TestId(2, Some(1), None).to_ustr();
    assert!(search_results.contains(&expected_id));
    Ok(())
}
