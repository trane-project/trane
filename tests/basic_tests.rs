//! End-to-end tests to test basic scenarios.
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
//!
//! The end-to-end tests in this file all use the same hand-coded course library and perform basic
//! checks, ensuring, among other things, that Trane makes progress when good scores are entered by
//! the student, that bad scores cause progress to stall, and that course and lesson filters are
//! respected.

use std::{collections::BTreeMap, sync::LazyLock};

use anyhow::{Ok, Result};
use chrono::{Duration, Utc};
use tempfile::TempDir;
use trane::{
    Trane,
    course_library::{CourseLibrary, LocalCourseLibrary, SerializedCourseLibrary},
    data::{
        MasteryScore, SchedulerOptions, UnitType, UserPreferences,
        filter::{ExerciseFilter, SessionPart, StudySession, StudySessionData, UnitFilter},
    },
    review_list::ReviewList,
    scheduler::ExerciseScheduler,
    test_utils::*,
};
use ustr::Ustr;

/// A simple set of courses to test the basic functionality of Trane.
static LIBRARY: LazyLock<Vec<TestCourse>> = LazyLock::new(|| {
    vec![
        TestCourse {
            id: TestId(0, None, None),
            dependencies: vec![],
            encompassed: vec![],
            superseded: vec![],
            metadata: BTreeMap::default(),
            lessons: vec![
                TestLesson {
                    id: TestId(0, Some(0), None),
                    dependencies: vec![],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(0, Some(1), None),
                    dependencies: vec![TestId(0, Some(0), None)],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(1, None, None),
            dependencies: vec![TestId(0, None, None)],
            encompassed: vec![],
            superseded: vec![],
            metadata: BTreeMap::default(),
            lessons: vec![
                TestLesson {
                    id: TestId(1, Some(0), None),
                    dependencies: vec![],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(1, Some(1), None),
                    dependencies: vec![TestId(1, Some(0), None)],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(2, None, None),
            dependencies: vec![TestId(0, None, None)],
            encompassed: vec![],
            superseded: vec![],
            metadata: BTreeMap::default(),
            lessons: vec![
                TestLesson {
                    id: TestId(2, Some(0), None),
                    dependencies: vec![],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(2, Some(1), None),
                    dependencies: vec![TestId(2, Some(0), None)],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(2, Some(2), None),
                    dependencies: vec![TestId(2, Some(1), None)],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(4, None, None),
            dependencies: vec![],
            encompassed: vec![],
            superseded: vec![],
            metadata: BTreeMap::default(),
            lessons: vec![
                TestLesson {
                    id: TestId(4, Some(0), None),
                    dependencies: vec![],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(4, Some(1), None),
                    dependencies: vec![TestId(4, Some(0), None)],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(4, Some(2), None),
                    dependencies: vec![TestId(4, Some(0), None)],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(4, Some(3), None),
                    dependencies: vec![TestId(4, Some(2), None)],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(5, None, None),
            dependencies: vec![
                // Depends on a missing course.
                TestId(3, None, None),
                TestId(4, None, None),
            ],
            encompassed: vec![],
            superseded: vec![],
            metadata: BTreeMap::default(),
            lessons: vec![
                TestLesson {
                    id: TestId(5, Some(0), None),
                    dependencies: vec![TestId(4, Some(1), None)],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(5, Some(1), None),
                    dependencies: vec![
                        TestId(5, Some(0), None),
                        // Depends on a missing lesson.
                        TestId(3, Some(3), None),
                    ],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(6, None, None),
            dependencies: vec![TestId(3, None, None)],
            encompassed: vec![],
            superseded: vec![],
            metadata: BTreeMap::default(),
            lessons: vec![
                TestLesson {
                    id: TestId(6, Some(0), None),
                    dependencies: vec![],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(6, Some(1), None),
                    dependencies: vec![TestId(6, Some(0), None)],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
            ],
        },
        TestCourse {
            id: TestId(7, None, None),
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
                    id: TestId(7, Some(0), None),
                    dependencies: vec![TestId(0, None, None)],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(7, Some(1), None),
                    dependencies: vec![
                        TestId(0, Some(0), None),
                        // Depends on a missing lesson.
                        TestId(6, Some(11), None),
                    ],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    num_exercises: 10,
                },
                TestLesson {
                    id: TestId(7, Some(2), None),
                    dependencies: vec![TestId(7, Some(1), None)],
                    encompassed: vec![],
                    superseded: vec![],
                    metadata: BTreeMap::default(),
                    // Lesson with no exercises.
                    num_exercises: 0,
                },
            ],
        },
        TestCourse {
            id: TestId(8, None, None),
            dependencies: vec![TestId(7, None, None)],
            encompassed: vec![],
            superseded: vec![],
            metadata: BTreeMap::default(),
            // Course with no lessons.
            lessons: vec![],
        },
    ]
});

/// A test that verifies that we retrieve the expected unit IDs.
#[test]
fn get_unit_ids() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Verify the course IDs.
    let course_ids = trane.get_course_ids();
    let expected_course_ids = vec![
        Ustr::from("0"),
        Ustr::from("1"),
        Ustr::from("2"),
        Ustr::from("4"),
        Ustr::from("5"),
        Ustr::from("6"),
        Ustr::from("7"),
        Ustr::from("8"),
    ];
    assert_eq!(course_ids, expected_course_ids);

    // Verify the lesson and exercise IDs.
    for course_id in course_ids {
        let course_test_id = TestId::from(&course_id);
        let lesson_ids = trane.get_lesson_ids(course_id).unwrap_or_default();
        for lesson_id in lesson_ids {
            let lesson_test_id = TestId::from(&lesson_id);
            assert_eq!(course_test_id.0, lesson_test_id.0);
            assert_eq!(lesson_test_id.2, None);
            let exercise_ids = trane.get_exercise_ids(lesson_id).unwrap_or_default();
            for exercise_id in exercise_ids {
                let exercise_test_id = TestId::from(&exercise_id);
                assert!(exercise_test_id.2.is_some());
                assert_eq!(course_test_id.0, exercise_test_id.0);
                assert_eq!(lesson_test_id.1, exercise_test_id.1);
            }
        }
    }

    Ok(())
}

/// A test that verifies retrieving all exercise IDs from the given unit.
#[test]
fn get_all_exercise_ids() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Get all exercises from a course.
    let exercise_ids = trane.get_all_exercise_ids(Some(Ustr::from("0")));
    assert!(!exercise_ids.is_empty());
    for exercise_id in exercise_ids {
        assert!(exercise_id.starts_with("0::"));
    }

    // Get all exercises from a lesson.
    let exercise_ids = trane.get_all_exercise_ids(Some(Ustr::from("0::0")));
    assert!(!exercise_ids.is_empty());
    for exercise_id in exercise_ids {
        assert!(exercise_id.starts_with("0::0::"));
    }

    // Get all exercises from an exercise. This should return only the exercise itself.
    let exercise_ids = trane.get_all_exercise_ids(Some(Ustr::from("0::0::0")));
    assert_eq!(exercise_ids.len(), 1);
    assert_eq!(exercise_ids[0], Ustr::from("0::0::0"));

    // Get all exercises from a non-existent unit. This should return an empty vector.
    let exercise_ids = trane.get_all_exercise_ids(Some(Ustr::from("0::0::100")));
    assert!(exercise_ids.is_empty());

    // Get all the exercises from the library.
    let exercise_ids = trane.get_all_exercise_ids(None);
    let expected_ids = all_test_exercises(&LIBRARY);
    assert_eq!(exercise_ids.len(), expected_ids.len());

    Ok(())
}

/// Verifies that all the exercises are scheduled with no blacklist or filter when the user gives a
/// score of five to every exercise.
#[test]
fn all_exercises_scheduled() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(&mut trane, &vec![], &None)?;

    // Every exercise ID should be in `simulation.answer_history`.
    let exercise_ids = all_test_exercises(&LIBRARY);
    for exercise_id in exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        assert!(
            simulation.answer_history.contains_key(&exercise_ustr),
            "exercise {:?} should have been scheduled",
            exercise_id
        );
        assert_simulation_scores(exercise_ustr, &trane, &simulation.answer_history)?;
    }
    Ok(())
}

/// Verifies no exercises past the first course and lesson are scheduled when the user scores every
/// exercise in that lesson with a mastery score of one.
#[test]
fn bad_score_prevents_advancing() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(200, Box::new(|_| Some(MasteryScore::One)));
    simulation.run_simulation(&mut trane, &vec![], &None)?;

    // Only the exercises in the first lessons should be in `simulation.answer_history`.
    let first_lessons = [
        TestId(0, Some(0), None),
        TestId(4, Some(0), None),
        TestId(6, Some(0), None),
    ];
    let exercise_ids = all_test_exercises(&LIBRARY);
    for exercise_id in exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if first_lessons
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

/// Verifies only exercises in the given course are scheduled when a course filter is provided.
#[test]
fn scheduler_respects_course_filter() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    let selected_courses = [
        TestId(1, None, None),
        TestId(5, None, None),
        // Missing course.
        TestId(3, None, None),
    ];
    let course_filter = UnitFilter::CourseFilter {
        course_ids: selected_courses.iter().map(|id| id.to_ustr()).collect(),
    };
    simulation.run_simulation(
        &mut trane,
        &vec![],
        &Some(ExerciseFilter::UnitFilter(course_filter)),
    )?;

    // Every exercise ID should be in `simulation.answer_history`.
    let exercise_ids = all_test_exercises(&LIBRARY);
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

/// Verifies only exercises in the given lesson are scheduled when a lesson filter is provided.
#[test]
fn scheduler_respects_lesson_filter() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    let selected_lessons = [
        TestId(2, Some(0), None),
        TestId(4, Some(1), None),
        // Missing lesson.
        TestId(3, Some(0), None),
    ];
    let lesson_filter = UnitFilter::LessonFilter {
        lesson_ids: selected_lessons.iter().map(|id| id.to_ustr()).collect(),
    };
    simulation.run_simulation(
        &mut trane,
        &vec![],
        &Some(ExerciseFilter::UnitFilter(lesson_filter)),
    )?;

    // Every exercise ID should be in `simulation.answer_history`.
    let exercise_ids = all_test_exercises(&LIBRARY);
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

/// Verifies that exercises in the review list are scheduled when using the review list filter.
#[test]
fn schedule_exercises_in_review_list() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Add some exercises to the review list.
    let review_exercises = vec![TestId(1, Some(0), Some(0)), TestId(2, Some(1), Some(7))];
    for unit_id in &review_exercises {
        let unit_ustr = unit_id.to_ustr();
        trane.add_to_review_list(unit_ustr)?;
    }

    // Run the simulation with the review list filter.
    let mut simulation = TraneSimulation::new(100, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(
        &mut trane,
        &vec![],
        &Some(ExerciseFilter::UnitFilter(UnitFilter::ReviewListFilter)),
    )?;

    // Only the exercises in the review list should have been scheduled.
    let exercise_ids = all_test_exercises(&LIBRARY);
    for exercise_id in exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if review_exercises
            .iter()
            .any(|review_exercise_id| *review_exercise_id == exercise_id)
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

/// Verifies that exercises from the lessons in the review list are scheduled when using the review
/// list filter.
#[test]
fn schedule_lessons_in_review_list() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Add some lessons to the review list.
    let review_lessons = vec![TestId(1, Some(0), None), TestId(2, Some(1), None)];
    for unit_id in &review_lessons {
        let unit_ustr = unit_id.to_ustr();
        trane.add_to_review_list(unit_ustr)?;
    }

    // Run the simulation with the review list filter.
    let mut simulation = TraneSimulation::new(100, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(
        &mut trane,
        &vec![],
        &Some(ExerciseFilter::UnitFilter(UnitFilter::ReviewListFilter)),
    )?;

    // Only the exercises from the lessons in the review list should have been scheduled.
    let exercise_ids = all_test_exercises(&LIBRARY);
    for exercise_id in exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if review_lessons
            .iter()
            .any(|lesson_id| exercise_id.exercise_in_lesson(lesson_id))
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

/// Verifies that exercises from the courses in the review list are scheduled when using the
/// review list filter.
#[test]
fn schedule_courses_in_review_list() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Add some courses to the review list.
    let review_courses = vec![TestId(1, None, None), TestId(2, None, None)];
    for unit_id in &review_courses {
        let unit_ustr = unit_id.to_ustr();
        trane.add_to_review_list(unit_ustr)?;
    }

    // Run the simulation with the review list filter.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(
        &mut trane,
        &vec![],
        &Some(ExerciseFilter::UnitFilter(UnitFilter::ReviewListFilter)),
    )?;

    // Only the exercises from the courses in the review list should have been scheduled.
    let exercise_ids = all_test_exercises(&LIBRARY);
    for exercise_id in exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if review_courses
            .iter()
            .any(|course_id| exercise_id.exercise_in_course(course_id))
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

/// Verifies scheduling exercises from the given units and their dependents.
#[test]
fn schedule_units_and_dependents() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Only schedule the exercises from the given units and their dependents.
    let starting_units = [TestId(5, Some(0), None)];
    let unit_and_dependents = [
        TestId(5, Some(0), None),
        TestId(5, Some(1), None),
        TestId(5, Some(2), None),
    ];

    // Run the simulation with the dependents filter.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(
        &mut trane,
        &vec![],
        &Some(ExerciseFilter::UnitFilter(UnitFilter::Dependents {
            unit_ids: starting_units
                .iter()
                .map(|unit_id| unit_id.to_ustr())
                .collect(),
        })),
    )?;

    // Only the exercises from the starting units and their dependents should have been scheduled.
    let exercise_ids = all_test_exercises(&LIBRARY);
    for exercise_id in exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if unit_and_dependents
            .iter()
            .any(|course_id| exercise_id.exercise_in_lesson(course_id))
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

/// Verifies scheduling exercises from the dependencies of a unit at a given depth.
#[test]
fn schedule_dependencies() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Only schedule the exercises from the dependencies of the unit at depth 1.
    let starting_units = [TestId(5, Some(1), None)];
    let depth = 1;
    let matching_lessons = [TestId(5, Some(0), None), TestId(5, Some(1), None)];

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(
        &mut trane,
        &vec![],
        &Some(ExerciseFilter::UnitFilter(UnitFilter::Dependencies {
            unit_ids: starting_units
                .iter()
                .map(|unit_id| unit_id.to_ustr())
                .collect(),
            depth,
        })),
    )?;

    // Only exercises that are dependencies of the starting units at the given depth or any of their
    // dependents should have been scheduled.
    let exercise_ids = all_test_exercises(&LIBRARY);
    for exercise_id in exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if matching_lessons
            .iter()
            .any(|course_id| exercise_id.exercise_in_lesson(course_id))
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

/// Verifies scheduling exercises from the dependencies of a unit at a depth that is larger than the
/// depth of the graph.
#[test]
fn schedule_dependencies_large_depth() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Only schedule the exercises from the dependencies of the unit at depth 5. The search should
    // stop earlier because the graph is not as deep.
    let starting_units = [TestId(2, None, None)];
    let depth = 5;
    let matching_courses = [
        TestId(0, None, None),
        TestId(1, None, None),
        TestId(2, None, None),
        TestId(7, None, None),
        TestId(8, None, None),
    ];

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(
        &mut trane,
        &vec![],
        &Some(ExerciseFilter::UnitFilter(UnitFilter::Dependencies {
            unit_ids: starting_units
                .iter()
                .map(|unit_id| unit_id.to_ustr())
                .collect(),
            depth,
        })),
    )?;

    // Only exercises that are dependencies of the starting units at the given depth or any of their
    // dependents should have been scheduled.
    let exercise_ids = all_test_exercises(&LIBRARY);
    for exercise_id in exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        if matching_courses
            .iter()
            .any(|course_id| exercise_id.exercise_in_course(course_id))
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

/// Verifies scheduling exercises from the dependencies of some unknown unit does not schedule any
/// exercises.
#[test]
fn schedule_dependencies_unknown_unit() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Only schedule the exercises from the dependencies of the unit at depth 5. Since the unit does
    // not exist, no exercises should be scheduled.
    let starting_units = [TestId(20, None, None)];
    let depth = 5;

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(
        &mut trane,
        &vec![],
        &Some(ExerciseFilter::UnitFilter(UnitFilter::Dependencies {
            unit_ids: starting_units
                .iter()
                .map(|unit_id| unit_id.to_ustr())
                .collect(),
            depth,
        })),
    )?;

    // Verify no exercises were scheduled.
    let exercise_ids = all_test_exercises(&LIBRARY);
    for exercise_id in exercise_ids {
        let exercise_ustr = exercise_id.to_ustr();
        assert!(
            !simulation.answer_history.contains_key(&exercise_ustr),
            "exercise {:?} should not have been scheduled",
            exercise_id
        );
    }
    Ok(())
}

/// Verifies scheduling exercises from a study session.
#[test]
fn schedule_study_session() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Create a study session with a couple of parts.
    let session_data = StudySessionData {
        start_time: Utc::now() - Duration::minutes(30),
        definition: StudySession {
            id: "session".into(),
            description: "session".into(),
            parts: vec![
                SessionPart::UnitFilter {
                    filter: UnitFilter::CourseFilter {
                        course_ids: vec!["0".into()],
                    },
                    duration: 15,
                },
                SessionPart::UnitFilter {
                    filter: UnitFilter::CourseFilter {
                        course_ids: vec!["1".into()],
                    },
                    duration: 30,
                },
            ],
        },
    };

    // Run the simulation.
    let mut simulation = TraneSimulation::new(500, Box::new(|_| Some(MasteryScore::Five)));
    simulation.run_simulation(
        &mut trane,
        &vec![],
        &Some(ExerciseFilter::StudySession(session_data)),
    )?;

    // The second part of the session is active, so only exercises from course 1 should have been
    // scheduled.
    let matching_courses = [TestId(1, None, None)];
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

/// Verifies matching the courses with the given prefix.
#[test]
fn get_matching_courses() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // The test will use the ID of course 0 as the prefix.
    let prefix = TestId(0, None, None).to_ustr();

    // Get all the courses that match the prefix.
    let matching_courses = trane.get_matching_prefix(&prefix, Some(UnitType::Course));
    assert_eq!(matching_courses.len(), 1);
    assert!(matching_courses.contains(&prefix));
    Ok(())
}

/// Verifies matching the lessons with the given prefix.
#[test]
fn get_matching_lessons() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // The test will use the ID of lesson 0::0 as the prefix.
    let prefix = TestId(0, Some(0), None).to_ustr();

    // Get all the lessons that match the prefix.
    let matching_lessons = trane.get_matching_prefix(&prefix, Some(UnitType::Lesson));
    assert_eq!(matching_lessons.len(), 1);
    assert!(matching_lessons.contains(&prefix));
    Ok(())
}

/// Verifies matching the exercises with the given prefix.
#[test]
fn get_matching_exercises() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // The test will use the ID of exercise 0::0::0 as the prefix.
    let prefix = TestId(0, Some(0), Some(0)).to_ustr();

    // Get all the exercises that match the prefix.
    let matching_exercises = trane.get_matching_prefix(&prefix, Some(UnitType::Exercise));
    assert_eq!(matching_exercises.len(), 1);
    assert!(matching_exercises.contains(&prefix));
    Ok(())
}

/// Verifies matching all units with the given prefix.
#[test]
fn get_matching_units() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // The test will use the ID of course 0 as the prefix.
    let prefix = TestId(0, None, None).to_ustr();

    // Get all the units that match the prefix. The 23 units are the course, the two lessons, and
    // the ten exercises in each lesson.
    let matching_units = trane.get_matching_prefix(&prefix, None);
    assert_eq!(matching_units.len(), 23);
    assert!(matching_units.contains(&prefix));
    assert!(matching_units.contains(&TestId(0, Some(0), None).to_ustr()));
    assert!(matching_units.contains(&TestId(0, Some(0), Some(0)).to_ustr()));
    Ok(())
}

/// Verifies searching for courses in the course library.
#[test]
fn course_library_search_courses() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Search for a course's ID.
    let search_results = trane.search("\"2\"")?;
    let expected_id = TestId(2, None, None).to_ustr();
    assert!(search_results.contains(&expected_id));

    // Search for a course's name.
    let search_results = trane.search("\"Course 2\"")?;
    let expected_id = TestId(2, None, None).to_ustr();
    assert!(search_results.contains(&expected_id));

    // Search for a course's description.
    let search_results = trane.search("\"Description for course 2\"")?;
    let expected_id = TestId(2, None, None).to_ustr();
    assert!(search_results.contains(&expected_id));
    Ok(())
}

/// Verifies searching for lessons in the course library.
#[test]
fn course_library_search_lessons() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Search for a lesson's ID.
    let search_results = trane.search("\"2::1\"")?;
    let expected_id = TestId(2, Some(1), None).to_ustr();
    assert!(search_results.contains(&expected_id));

    // Search for a lesson's name.
    let search_results = trane.search("\"Lesson 2::1\"")?;
    let expected_id = TestId(2, Some(1), None).to_ustr();
    assert!(search_results.contains(&expected_id));

    // Search for a lesson's description.
    let search_results = trane.search("\"Description for lesson 2::1\"")?;
    let expected_id = TestId(2, Some(1), None).to_ustr();
    assert!(search_results.contains(&expected_id));
    Ok(())
}

/// Verifies that searching for exercises in the course library.
#[test]
fn course_library_search_exercises() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Search for an exercise ID.
    let search_results = trane.search("\"2::1::7\"")?;
    let expected_id = TestId(2, Some(1), Some(7)).to_ustr();
    assert!(search_results.contains(&expected_id));

    // Search for an exercise name.
    let search_results = trane.search("\"Exercise 2::1::7\"")?;
    let expected_id = TestId(2, Some(1), Some(7)).to_ustr();
    assert!(search_results.contains(&expected_id));

    // Search for an exercise description.
    let search_results = trane.search("\"Description for exercise 2::1::7\"")?;
    let expected_id = TestId(2, Some(1), Some(7)).to_ustr();
    assert!(search_results.contains(&expected_id));
    Ok(())
}

/// Verifies setting the scheduler options.
#[test]
fn set_scheduler_options() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Set the scheduler options to have a batch size of ten.
    let scheduler_options = SchedulerOptions {
        batch_size: 10,
        ..Default::default()
    };
    trane.set_scheduler_options(scheduler_options);

    // Verify the scheduler options were set.
    let scheduler_options = trane.get_scheduler_options();
    assert_eq!(scheduler_options.batch_size, 10);
    Ok(())
}

// Verifies resetting the scheduler options.
#[test]
fn reset_scheduler_options() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Set the scheduler options to have a batch size of ten.
    let scheduler_options = SchedulerOptions {
        batch_size: 10,
        ..Default::default()
    };
    trane.set_scheduler_options(scheduler_options);

    // Reset the scheduler options and verify the scheduler options were reset.
    trane.reset_scheduler_options();
    let scheduler_options = trane.get_scheduler_options();
    assert_eq!(
        scheduler_options.batch_size,
        SchedulerOptions::default().batch_size
    );
    Ok(())
}

/// Verifies ignoring courses specified in the user preferences.
#[test]
fn ignored_paths() -> Result<()> {
    // Set the user preferences to ignore some courses.
    let user_preferences = UserPreferences {
        ignored_paths: vec!["course_0/".to_owned(), "course_5/".to_owned()],
        ..Default::default()
    };

    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let course_builders = LIBRARY
        .iter()
        .map(|c| c.course_builder())
        .collect::<Result<Vec<_>>>()?;
    let trane = init_simulation(temp_dir.path(), &course_builders, Some(&user_preferences))?;

    // Verify the courses in the list are ignored.
    let exercise_ids = trane.get_all_exercise_ids(None);
    assert!(!exercise_ids.is_empty());
    assert!(exercise_ids.iter().all(|id| !id.starts_with("0::")));
    assert!(exercise_ids.iter().all(|id| !id.starts_with("5::")));
    Ok(())
}

/// Verifies serializing a course library and opening a new one from the serialized data.
#[test]
fn serialized_course_library() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let _ = init_test_simulation(temp_dir.path(), &LIBRARY)?;

    // Serialize the course library. This requires opening the course library again. The `Trane`
    // struct cannot be used because it uses dynamic traits.
    let course_library = LocalCourseLibrary::new(temp_dir.path(), UserPreferences::default())?;
    let serialized_data = SerializedCourseLibrary::from(&course_library);

    // Decode and encode the data.
    let encoded_data =
        bincode::encode_to_vec(serialized_data.clone(), bincode::config::standard())?;
    let decoded_data: SerializedCourseLibrary =
        bincode::decode_from_slice(&encoded_data, bincode::config::standard())?.0;
    assert_eq!(serialized_data, decoded_data);

    // Open a new library using the serialized data and verify it matches the original.
    let from_serialized_library = LocalCourseLibrary::new_from_serialized(
        serialized_data.clone(),
        UserPreferences::default(),
    )?;
    assert_eq!(
        course_library.course_map,
        from_serialized_library.course_map,
    );
    assert_eq!(
        course_library.lesson_map,
        from_serialized_library.lesson_map,
    );
    assert_eq!(
        course_library.exercise_map,
        from_serialized_library.exercise_map,
    );
    assert_eq!(
        *course_library.unit_graph.read(),
        *from_serialized_library.unit_graph.read(),
    );

    // Verify the search index of the new library works.
    let search_results = from_serialized_library.search("\"Exercise 2::1::7\"")?;
    let expected_id = TestId(2, Some(1), Some(7)).to_ustr();
    assert!(search_results.contains(&expected_id));

    // Open a new trane instance using the serialized library.
    let trane = Trane::new_local_from_serialized(temp_dir.path(), serialized_data)?;
    assert!(trane.get_all_exercise_ids(None).len() > 0);

    Ok(())
}
