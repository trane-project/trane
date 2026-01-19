//! End-to-end tests to verify that the transcription course generator works as expected.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::LazyLock;
use tempfile::TempDir;
use trane::{
    course_builder::{AssetBuilder, CourseBuilder},
    course_library::CourseLibrary,
    data::{
        CourseGenerator, CourseManifest, LessonManifestBuilder, MasteryScore, UserPreferences,
        course_generator::{
            Instrument,
            transcription::{
                TranscriptionAsset, TranscriptionConfig, TranscriptionPassages,
                TranscriptionPreferences,
            },
        },
    },
    test_utils::{TraneSimulation, assert_simulation_scores, init_simulation},
};
use ustr::Ustr;

static COURSE0_ID: LazyLock<Ustr> =
    LazyLock::new(|| Ustr::from("trane::test::transcription_course_0"));
static COURSE1_ID: LazyLock<Ustr> =
    LazyLock::new(|| Ustr::from("trane::test::transcription_course_1"));
static USER_PREFS: LazyLock<UserPreferences> = LazyLock::new(|| UserPreferences {
    transcription: Some(TranscriptionPreferences {
        instruments: vec![
            Instrument {
                name: "Guitar".to_string(),
                id: "guitar".to_string(),
            },
            Instrument {
                name: "Piano".to_string(),
                id: "piano".to_string(),
            },
        ],
        ..Default::default()
    }),
    ignored_paths: vec![],
    scheduler: None,
});

/// Returns a course builder with a transcription generator.
fn transcription_builder(
    course_id: Ustr,
    course_index: usize,
    dependencies: Vec<Ustr>,
    num_passages: usize,
    skip_singing_lessons: bool,
    skip_advanced_lessons: bool,
) -> CourseBuilder {
    // Create the passages for the course. Half of the passages will be stored in the `passages`
    // directory, and the other half will be inlined in the course manifest.
    let mut asset_builders = Vec::new();
    let mut inlined_passages = Vec::new();
    for i in 0..num_passages {
        // Create the passages. Create half of them with explicit intervals and half without.
        let passages = TranscriptionPassages {
            asset: TranscriptionAsset::Track {
                short_id: format!("passages_{}", i),
                track_name: format!("Track {}", i),
                artist_name: None,
                album_name: None,
                duration: None,
                external_link: None,
            },
            intervals: if i % 2 == 0 {
                HashMap::from([
                    (0, ("0:00".to_string(), "0:01".to_string())),
                    (1, ("0:05".to_string(), "0:10".to_string())),
                ])
            } else {
                HashMap::new()
            },
        };

        // In odd iterations, add the passage to the inlined passages.
        if i % 2 == 1 {
            inlined_passages.push(passages);
            continue;
        }

        // In even iterations, write the passage to the `passages` directory.
        let passage_path = format!("passages/passages_{}.json", i);
        asset_builders.push(AssetBuilder {
            file_name: passage_path.clone(),
            contents: serde_json::to_string_pretty(&passages).unwrap(),
        });
    }

    CourseBuilder {
        directory_name: format!("transcription_course_{}", course_index),
        course_manifest: CourseManifest {
            id: course_id,
            name: format!("Course {}", course_id),
            dependencies: vec![],
            superseded: vec![],
            description: None,
            authors: None,
            metadata: None,
            course_material: None,
            course_instructions: None,
            default_exercise_type: None,
            generator_config: Some(CourseGenerator::Transcription(TranscriptionConfig {
                transcription_dependencies: dependencies,
                passage_directory: "passages".to_string(),
                inlined_passages,
                skip_singing_lessons,
                skip_advanced_lessons,
            })),
        },
        lesson_manifest_template: LessonManifestBuilder::default().clone(),
        lesson_builders: vec![],
        asset_builders,
    }
}

/// Verifies that all transcription exercises are visited.
#[test]
fn all_exercises_visited() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_simulation(
        temp_dir.path(),
        &[
            transcription_builder(*COURSE0_ID, 0, vec![], 5, false, false),
            transcription_builder(*COURSE1_ID, 1, vec![*COURSE0_ID], 5, false, false),
        ],
        Some(&USER_PREFS),
    )?;

    // Run the simulation.
    let exercise_ids = trane.get_all_exercise_ids(None);
    assert!(!exercise_ids.is_empty());
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 5,
        Box::new(|_| Some(MasteryScore::Five)),
    );
    simulation.run_simulation(&mut trane, &vec![], &None)?;

    // Every exercise in the advanced singing and advanced transcription lessons should be in
    // `simulation.answer_history`. Most of the exercises in the singing and transcription lessons
    // should be there as well, but since they are superseded by the advanced lessons, it' not
    // guaranteed that all of them will be there.
    for exercise_id in exercise_ids {
        assert!(
            simulation.answer_history.contains_key(&exercise_id),
            "exercise {:?} should have been scheduled",
            exercise_id
        );
        assert_simulation_scores(exercise_id, &trane, &simulation.answer_history)?;
    }
    Ok(())
}

/// Verifies that not making progress on the singing lessons blocks all further progress.
#[test]
fn no_progress_past_singing_lessons() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_simulation(
        temp_dir.path(),
        &[
            transcription_builder(*COURSE0_ID, 0, vec![], 5, false, false),
            transcription_builder(*COURSE1_ID, 1, vec![*COURSE0_ID], 5, false, false),
        ],
        Some(&USER_PREFS),
    )?;

    // Run the simulation. Give every exercise a score of one, which should block all further
    // progress past the starting lessons.
    let exercise_ids = trane.get_all_exercise_ids(None);
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 5,
        Box::new(|_| Some(MasteryScore::One)),
    );
    simulation.run_simulation(&mut trane, &vec![], &None)?;

    // Only exercises from the singing lessons of the first are in the answer history.
    for exercise_id in exercise_ids {
        if exercise_id.contains("transcription_course_0::singing") {
            assert!(
                simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_simulation_scores(exercise_id, &trane, &simulation.answer_history)?;
        } else {
            assert!(
                !simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should not have been scheduled",
                exercise_id
            );
        }
    }
    Ok(())
}

/// Verifies that not making progress on the advanced singing lessons blocks the advanced
/// transcription lessons.
#[test]
fn advanced_singing_blocks_advanced_transcription() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_simulation(
        temp_dir.path(),
        &[
            transcription_builder(*COURSE0_ID, 0, vec![], 5, false, false),
            transcription_builder(*COURSE1_ID, 1, vec![*COURSE0_ID], 5, false, false),
        ],
        Some(&USER_PREFS),
    )?;

    // Run the simulation. Give every advanced singing exercise a score of one, which should block
    // all progress on the advanced transcription lessons.
    let exercise_ids = trane.get_all_exercise_ids(None);
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 5,
        Box::new(|exercise_id| {
            if exercise_id.contains("advanced_singing") {
                Some(MasteryScore::One)
            } else {
                Some(MasteryScore::Five)
            }
        }),
    );
    simulation.run_simulation(&mut trane, &vec![], &None)?;

    // Exercises from the advanced transcription lessons should not be in the answer history.
    for exercise_id in exercise_ids {
        if exercise_id.contains("advanced_transcription") {
            assert!(
                !simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should not have been scheduled",
                exercise_id
            );
            assert_simulation_scores(exercise_id, &trane, &simulation.answer_history)?;
        } else {
            assert!(
                simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
        }
    }
    Ok(())
}

/// Verifies that not making progress on the transcription lessons blocks the advanced transcription
/// lessons from the same course and the transcription lessons from the next course.
#[test]
fn transcription_blocks_advanced_transcription_and_dependents() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_simulation(
        temp_dir.path(),
        &[
            transcription_builder(*COURSE0_ID, 0, vec![], 5, false, false),
            transcription_builder(*COURSE1_ID, 1, vec![*COURSE0_ID], 5, false, false),
        ],
        Some(&USER_PREFS),
    )?;

    // Run the simulation. Give every transcription exercise from the first course a score of one,
    // which should block all progress on the advanced transcription lessons. It also blocks the
    // transcription lessons from the second course.
    let exercise_ids = trane.get_all_exercise_ids(None);
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 5,
        Box::new(|exercise_id| {
            if exercise_id.contains("trane::test::transcription_course_0::transcription::") {
                Some(MasteryScore::One)
            } else {
                Some(MasteryScore::Five)
            }
        }),
    );
    simulation.run_simulation(&mut trane, &vec![], &None)?;

    // Exercises from the advanced transcription lesson from the first and the transcription lesson
    // from the dependent course should not be in the answer history.
    for exercise_id in exercise_ids {
        if exercise_id.contains("advanced_transcription")
            || exercise_id.contains("transcription_course_1::transcription")
        {
            assert!(
                !simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should not have been scheduled",
                exercise_id
            );
        } else {
            assert!(
                simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_simulation_scores(exercise_id, &trane, &simulation.answer_history)?;
        }
    }
    Ok(())
}

/// Verifies that all transcription exercises are visited when the advanced lessons are skipped.
#[test]
fn skip_advanced_lessons() -> Result<()> {
    // Initialize test course library. Skip the advanced lessons.
    let temp_dir = TempDir::new()?;
    let mut trane = init_simulation(
        temp_dir.path(),
        &[
            transcription_builder(*COURSE0_ID, 0, vec![], 5, false, true),
            transcription_builder(*COURSE1_ID, 1, vec![*COURSE0_ID], 5, false, true),
        ],
        Some(&USER_PREFS),
    )?;

    // Run the simulation.
    let exercise_ids = trane.get_all_exercise_ids(None);
    assert!(!exercise_ids.is_empty());
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 5,
        Box::new(|_| Some(MasteryScore::Five)),
    );
    simulation.run_simulation(&mut trane, &vec![], &None)?;

    // Every exercise ID should be in `simulation.answer_history`.
    for exercise_id in &exercise_ids {
        assert!(
            simulation.answer_history.contains_key(exercise_id),
            "exercise {:?} should have been scheduled",
            exercise_id
        );
        assert_simulation_scores(*exercise_id, &trane, &simulation.answer_history)?;
    }

    // No exercises from the advanced lessons should have been generated.
    for exercise_id in exercise_ids {
        assert!(
            !exercise_id.contains("advanced_"),
            "exercise {:?} should not have been generated",
            exercise_id
        );
    }
    Ok(())
}

/// Verifies that all transcription exercises are visited when the singing lessons are skipped.
#[test]
fn skip_singing_lessons() -> Result<()> {
    // Initialize test course library. Skip the advanced lessons.
    let temp_dir = TempDir::new()?;
    let mut trane = init_simulation(
        temp_dir.path(),
        &[
            transcription_builder(*COURSE0_ID, 0, vec![], 5, true, false),
            transcription_builder(*COURSE1_ID, 1, vec![*COURSE0_ID], 5, true, false),
        ],
        Some(&USER_PREFS),
    )?;

    // Run the simulation.
    let exercise_ids = trane.get_all_exercise_ids(None);
    assert!(!exercise_ids.is_empty());
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 5,
        Box::new(|_| Some(MasteryScore::Five)),
    );
    simulation.run_simulation(&mut trane, &vec![], &None)?;

    // Every exercise ID should be in `simulation.answer_history`.
    for exercise_id in &exercise_ids {
        assert!(
            simulation.answer_history.contains_key(exercise_id),
            "exercise {:?} should have been scheduled",
            exercise_id
        );
        assert_simulation_scores(*exercise_id, &trane, &simulation.answer_history)?;
    }

    // No exercises from the singing lessons should have been generated.
    for exercise_id in exercise_ids {
        assert!(
            !exercise_id.contains("singing"),
            "exercise {:?} should not have been generated",
            exercise_id
        );
    }
    Ok(())
}
