//! End-to-end tests to verify that the improvisation course generator works as expected.

use anyhow::Result;
use lazy_static::lazy_static;
use tempfile::TempDir;
use trane::{
    course_builder::{AssetBuilder, CourseBuilder},
    course_library::CourseLibrary,
    data::{
        course_generator::{
            improvisation::{ImprovisationConfig, ImprovisationPreferences},
            Instrument,
        },
        CourseGenerator, CourseManifest, LessonManifestBuilder, MasteryScore, UserPreferences,
    },
    testutil::{assert_simulation_scores, init_simulation, TraneSimulation},
};
use ustr::Ustr;

lazy_static! {
    static ref COURSE0_ID: Ustr = Ustr::from("improvisation_course_0");
    static ref COURSE1_ID: Ustr = Ustr::from("improvisation_course_1");
    static ref USER_PREFS: UserPreferences = UserPreferences {
        improvisation: Some(ImprovisationPreferences {
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
            rhythm_instruments: vec![Instrument {
                name: "Drums".to_string(),
                id: "drums".to_string(),
            }],
        }),
        ignored_paths: vec![],
        scheduler: None,
        transcription: None,
    };
}

/// Returns a course builder with an improvisation generator.
fn improvisation_builder(
    course_id: Ustr,
    course_index: usize,
    dependencies: Vec<Ustr>,
    num_passages: usize,
    rhythm_only: bool,
) -> CourseBuilder {
    let mut asset_builders = Vec::new();
    for i in 0..num_passages {
        // Create an asset builder for a file named `i.ly` in the `passages` directory.
        let passage_path = format!("passages/{}.ly", i);
        asset_builders.push(AssetBuilder {
            file_name: passage_path.clone(),
            contents: "".to_string(),
        });
    }

    CourseBuilder {
        directory_name: format!("improvisation_course_{}", course_index),
        course_manifest: CourseManifest {
            id: course_id,
            name: format!("Course {}", course_id),
            dependencies: vec![],
            description: None,
            authors: None,
            metadata: None,
            course_material: None,
            course_instructions: None,
            generator_config: Some(CourseGenerator::Improvisation(ImprovisationConfig {
                improvisation_dependencies: dependencies,
                rhythm_only,
                passage_directory: "passages".to_string(),
                file_extensions: vec!["ly".to_string()],
            })),
        },
        lesson_manifest_template: LessonManifestBuilder::default().clone(),
        lesson_builders: vec![],
        asset_builders: asset_builders,
    }
}

/// Verifies that the course generator fails when multiple passages have the same ID.
#[test]
fn duplicate_passage_ids_fail() -> Result<()> {
    // Generate a bad course with multiple passages with the same ID.
    let mut asset_builders = Vec::new();
    for extension in ["md", "pdf", "ly"].iter() {
        // Create an asset builder for a file named `passage.{}` in the `passages` directory for
        // each extension. They all have the same ID.
        let passage_path = format!("passages/passage.{}", extension);
        asset_builders.push(AssetBuilder {
            file_name: passage_path.clone(),
            contents: "".to_string(),
        });
    }
    let bad_course_builder = CourseBuilder {
        directory_name: "improvisation_course_0".to_string(),
        course_manifest: CourseManifest {
            id: *COURSE0_ID,
            name: format!("Course {}", *COURSE0_ID),
            dependencies: vec![],
            description: None,
            authors: None,
            metadata: None,
            course_material: None,
            course_instructions: None,
            generator_config: Some(CourseGenerator::Improvisation(ImprovisationConfig {
                improvisation_dependencies: vec![],
                rhythm_only: false,
                passage_directory: "passages".to_string(),
                file_extensions: vec!["md".to_string(), "pdf".to_string(), "ly".to_string()],
            })),
        },
        lesson_manifest_template: LessonManifestBuilder::default().clone(),
        lesson_builders: vec![],
        asset_builders: asset_builders,
    };

    // Initialize test course library. It should fail due to the duplicate passage IDs.
    let temp_dir = TempDir::new()?;
    let trane = init_simulation(
        &temp_dir.path(),
        &vec![bad_course_builder],
        Some(&USER_PREFS),
    );
    assert!(trane.is_err());
    Ok(())
}

/// Verifies that all improvisation exercises are visited.
#[test]
fn all_exercises_visited() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_simulation(
        &temp_dir.path(),
        &vec![
            improvisation_builder(*COURSE0_ID, 0, vec![], 5, false),
            improvisation_builder(*COURSE1_ID, 1, vec![*COURSE0_ID], 5, false),
        ],
        Some(&USER_PREFS),
    )?;

    // Run the simulation.
    let exercise_ids = trane.get_all_exercise_ids();
    assert!(exercise_ids.len() > 0);
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 5,
        Box::new(|_| Some(MasteryScore::Five)),
    );
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // Every exercise ID should be in `simulation.answer_history`.
    for exercise_id in exercise_ids {
        assert!(
            simulation.answer_history.contains_key(&exercise_id),
            "exercise {:?} should have been scheduled",
            exercise_id
        );
        assert_simulation_scores(&exercise_id, &trane, &simulation.answer_history)?;
    }
    Ok(())
}

/// Verifies that all improvisation exercises are visited when no instruments are specified.
#[test]
fn all_exercises_visited_no_instruments() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_simulation(
        &temp_dir.path(),
        &vec![
            improvisation_builder(*COURSE0_ID, 0, vec![], 5, false),
            improvisation_builder(*COURSE1_ID, 1, vec![*COURSE0_ID], 5, false),
        ],
        None,
    )?;

    // Run the simulation.
    let exercise_ids = trane.get_all_exercise_ids();
    assert!(exercise_ids.len() > 0);
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 5,
        Box::new(|_| Some(MasteryScore::Five)),
    );
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // Every exercise ID should be in `simulation.answer_history`.
    for exercise_id in exercise_ids {
        assert!(
            simulation.answer_history.contains_key(&exercise_id),
            "exercise {:?} should have been scheduled",
            exercise_id
        );
        assert_simulation_scores(&exercise_id, &trane, &simulation.answer_history)?;
    }
    Ok(())
}

/// Verifies that all improvisation exercises are visited when only rhythm lessons are specified in
/// the configuration.
#[test]
fn all_exercises_visited_rhythm_only() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_simulation(
        &temp_dir.path(),
        &vec![
            improvisation_builder(*COURSE0_ID, 0, vec![], 5, true),
            improvisation_builder(*COURSE1_ID, 1, vec![*COURSE0_ID], 5, true),
        ],
        Some(&USER_PREFS),
    )?;

    // Run the simulation.
    let exercise_ids = trane.get_all_exercise_ids();
    assert!(exercise_ids.len() > 0);
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 10,
        Box::new(|_| Some(MasteryScore::Five)),
    );
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // Every exercise ID should be in `simulation.answer_history`.
    for exercise_id in exercise_ids {
        assert!(
            simulation.answer_history.contains_key(&exercise_id),
            "exercise {:?} should have been scheduled",
            exercise_id
        );
        assert_simulation_scores(&exercise_id, &trane, &simulation.answer_history)?;
    }
    Ok(())
}

/// Verifies that not making progress on the singing lessons blocks all further progress.
#[test]
fn no_progress_past_singing_lessons() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_simulation(
        &temp_dir.path(),
        &vec![
            improvisation_builder(*COURSE0_ID, 0, vec![], 5, false),
            improvisation_builder(*COURSE1_ID, 1, vec![*COURSE0_ID], 5, false),
        ],
        Some(&USER_PREFS),
    )?;

    // Run the simulation. Give every exercise a score of one, which should block all further
    // progress past the starting lessons.
    let exercise_ids = trane.get_all_exercise_ids();
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 5,
        Box::new(|_| Some(MasteryScore::One)),
    );
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // Only exercises from the singing lessons of the course are in the answer history.
    for exercise_id in exercise_ids {
        if exercise_id.contains("improvisation_course_0::singing") {
            assert!(
                simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_simulation_scores(&exercise_id, &trane, &simulation.answer_history)?;
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

// Verifies that not mastering the basic harmony lessons blocks all progress on the advanced harmony
// and mastery lessons.
#[test]
fn basic_harmony_blocks_advanced_harmony() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_simulation(
        &temp_dir.path(),
        &vec![
            improvisation_builder(*COURSE0_ID, 0, vec![], 5, false),
            improvisation_builder(*COURSE1_ID, 1, vec![*COURSE0_ID], 5, false),
        ],
        Some(&USER_PREFS),
    )?;

    // Run the simulation. Give every exercise a score of five, except for the basic harmony
    // exercises, which should block progress to the advanced harmony and mastery lessons.
    let exercise_ids = trane.get_all_exercise_ids();
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 5,
        Box::new(|exercise_id| {
            if exercise_id.contains("basic_harmony") {
                Some(MasteryScore::One)
            } else {
                Some(MasteryScore::Five)
            }
        }),
    );
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // Exercises from the advanced harmony and mastery lessons should not be in the answer history.
    // Exercises from the singing, melody, and rhythm lessons should be in the answer history. The
    // first basic harmony lesson should be in the answer history.
    for exercise_id in exercise_ids {
        if exercise_id.contains("advanced_harmony") || exercise_id.contains("mastery") {
            assert!(
                !simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should not have been scheduled",
                exercise_id
            );
        } else if exercise_id.contains("singing")
            || exercise_id.contains("melody")
            || exercise_id.contains("rhythm")
            || exercise_id.contains("basic_harmony::c")
        {
            assert!(
                simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_simulation_scores(&exercise_id, &trane, &simulation.answer_history)?;
        }
    }
    Ok(())
}

// Verifies that not mastering the basic harmony lessons blocks all progress on the advanced harmony
// and mastery lessons.
#[test]
fn advanced_harmony_blocks_mastery() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_simulation(
        &temp_dir.path(),
        &vec![
            improvisation_builder(*COURSE0_ID, 0, vec![], 5, false),
            improvisation_builder(*COURSE1_ID, 1, vec![*COURSE0_ID], 5, false),
        ],
        Some(&USER_PREFS),
    )?;

    // Run the simulation. Give every exercise a score of five, except for the advanced harmony
    // exercises, which should block progress to the mastery lessons.
    let exercise_ids = trane.get_all_exercise_ids();
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 5,
        Box::new(|exercise_id| {
            if exercise_id.contains("advanced_harmony") {
                Some(MasteryScore::One)
            } else {
                Some(MasteryScore::Five)
            }
        }),
    );
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // Exercises from the mastery lessons should not be in the answer history. Exercises from the
    // singing, melody, rhythm, and basic harmony lessons should be in the answer history. The first
    // advanced harmony lesson should be in the answer history.
    for exercise_id in exercise_ids {
        if exercise_id.contains("mastery") {
            assert!(
                !simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should not have been scheduled",
                exercise_id
            );
        } else if exercise_id.contains("singing")
            || exercise_id.contains("melody")
            || exercise_id.contains("rhythm")
            || exercise_id.contains("basic_harmony")
            || exercise_id.contains("advanced_harmony::c")
        {
            assert!(
                simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_simulation_scores(&exercise_id, &trane, &simulation.answer_history)?;
        }
    }
    Ok(())
}

// Verifies that not mastering the melody lessons blocks all progress on the mastery lessons.
#[test]
fn melody_blocks_mastery() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_simulation(
        &temp_dir.path(),
        &vec![
            improvisation_builder(*COURSE0_ID, 0, vec![], 5, false),
            improvisation_builder(*COURSE1_ID, 1, vec![*COURSE0_ID], 5, false),
        ],
        Some(&USER_PREFS),
    )?;

    // Run the simulation. Give every exercise a score of five, except for the melody exercises,
    // which should block all further progress past the starting lessons.
    let exercise_ids = trane.get_all_exercise_ids();
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 5,
        Box::new(|exercise_id| {
            if exercise_id.contains("melody") {
                Some(MasteryScore::One)
            } else {
                Some(MasteryScore::Five)
            }
        }),
    );
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // Exercises from the mastery lessons should not be in the answer history. Exercises from the
    // singing, basic harmony, advanced harmony, and rhythm lessons should be in the answer history.
    // The first melody lesson should be in the answer history.
    for exercise_id in exercise_ids {
        if exercise_id.contains("mastery") {
            assert!(
                !simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should not have been scheduled",
                exercise_id
            );
        } else if exercise_id.contains("singing")
            || exercise_id.contains("basic_harmony")
            || exercise_id.contains("advanced_harmony")
            || exercise_id.contains("rhythm")
            || exercise_id.contains("melody::c")
        {
            assert!(
                simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_simulation_scores(&exercise_id, &trane, &simulation.answer_history)?;
        }
    }
    Ok(())
}

/// Verifies that not mastering the rhythm lessons blocks all progress on the mastery lessons.
#[test]
fn rhythm_blocks_mastery() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_simulation(
        &temp_dir.path(),
        &vec![
            improvisation_builder(*COURSE0_ID, 0, vec![], 5, false),
            improvisation_builder(*COURSE1_ID, 1, vec![*COURSE0_ID], 5, false),
        ],
        Some(&*USER_PREFS),
    )?;

    // Run the simulation. Give every exercise a score of five, except for the rhythm exercises,
    // which should block all further progress past the starting lessons.
    let exercise_ids = trane.get_all_exercise_ids();
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 5,
        Box::new(|exercise_id| {
            if exercise_id.contains("rhythm") {
                Some(MasteryScore::One)
            } else {
                Some(MasteryScore::Five)
            }
        }),
    );
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // Exercises from the mastery lessons should not be in the answer history. Exercises from the
    // singing, basic harmony, advanced harmony, and melody lessons should be in the answer history.
    // The rhythm lessons for individual instruments should not be in the answer history.
    for exercise_id in exercise_ids {
        if exercise_id.contains("mastery")
            || exercise_id.contains("rhythm::piano")
            || exercise_id.contains("rhythm::guitar")
            || exercise_id.contains("rhythm::drums")
        {
            assert!(
                !simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should not have been scheduled",
                exercise_id
            );
        } else if exercise_id.contains("singing")
            || exercise_id.contains("basic_harmony")
            || exercise_id.contains("advanced_harmony")
            || exercise_id.contains("melody")
            || exercise_id.contains("rhythm")
        {
            assert!(
                simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_simulation_scores(&exercise_id, &trane, &simulation.answer_history)?;
        }
    }
    Ok(())
}

// Verifies that not mastering the sight-singing lessons blocks all progress on the instrument
// lessons.
#[test]
fn sight_singing_lessons_block_instruments() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_simulation(
        &temp_dir.path(),
        &vec![
            improvisation_builder(*COURSE0_ID, 0, vec![], 5, false),
            improvisation_builder(*COURSE1_ID, 1, vec![*COURSE0_ID], 5, false),
        ],
        Some(&USER_PREFS),
    )?;

    // Run the simulation. Give all the exercises involving sight-singing and not an instrument a
    // score of one. Give all other exercises a score of five.
    let exercise_ids = trane.get_all_exercise_ids();
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 5,
        Box::new(|exercise_id| {
            if exercise_id.contains("singing") {
                Some(MasteryScore::Five)
            } else if !(exercise_id.contains("piano") || exercise_id.contains("guitar")) {
                Some(MasteryScore::One)
            } else {
                Some(MasteryScore::Five)
            }
        }),
    );
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // Verify that none of the exercises for piano or guitar are in the answer history.
    for exercise_id in exercise_ids {
        if exercise_id.contains("piano")
            || exercise_id.contains("guitar")
            || exercise_id.contains("drums")
        {
            assert!(
                !simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should not have been scheduled",
                exercise_id
            );
        } else if exercise_id.contains("mastery") {
            assert!(
                !simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should not have been scheduled",
                exercise_id
            );
        } else if exercise_id.contains("singing")
            || exercise_id.contains("rhythm")
            || exercise_id.contains("melody::c")
            || exercise_id.contains("basic_harmony::c")
            || exercise_id.contains("advanced_harmony::c")
        {
            assert!(
                simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
            assert_simulation_scores(&exercise_id, &trane, &simulation.answer_history)?;
        }
    }
    Ok(())
}

/// Verifies that not mastering the lessons for a particular key blocks progress on all the lessons
/// for the next keys in the circle of fifths.
#[test]
fn key_blocks_next_keys() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_simulation(
        &temp_dir.path(),
        &vec![
            improvisation_builder(*COURSE0_ID, 0, vec![], 5, false),
            improvisation_builder(*COURSE1_ID, 1, vec![*COURSE0_ID], 5, false),
        ],
        Some(&USER_PREFS),
    )?;

    // Run the simulation. Give all the exercises involving the key of C a score of one. Give all
    // other exercises a score of five. This should block progress on the exercises for all the
    // other keys.
    let exercise_ids = trane.get_all_exercise_ids();
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 5,
        Box::new(|exercise_id| {
            if exercise_id.contains("::C") {
                Some(MasteryScore::One)
            } else {
                Some(MasteryScore::Five)
            }
        }),
    );
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // Verify that none of the exercises for the other keys are in the answer history. All the
    // advanced harmony and mastery lessons are missing as well because they are blocked by the
    // basic harmony lessons. The lessons for the instruments are blocked by the sight-singing
    // lessons.
    for exercise_id in exercise_ids {
        if exercise_id.contains("singing") || exercise_id.contains("rhythm") {
            assert!(
                simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
        } else if exercise_id.contains("advanced_harmony")
            || exercise_id.contains("mastery")
            || exercise_id.contains("guitar")
            || exercise_id.contains("piano")
        {
            assert!(
                !simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should not have been scheduled",
                exercise_id
            );
            assert_simulation_scores(&exercise_id, &trane, &simulation.answer_history)?;
        } else if exercise_id.contains("::C") {
            assert!(
                simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should have been scheduled",
                exercise_id
            );
        } else {
            assert!(
                !simulation.answer_history.contains_key(&exercise_id),
                "exercise {:?} should not have been scheduled",
                exercise_id
            );
            assert_simulation_scores(&exercise_id, &trane, &simulation.answer_history)?;
        }
    }
    Ok(())
}
