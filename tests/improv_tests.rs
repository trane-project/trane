//! End-to-end tests to verify that the Trane Improvisation course generator works as expected.

use anyhow::Result;
use std::{
    collections::HashMap,
    fs::{create_dir, File},
    io::Write,
    path::Path,
};
use tempfile::TempDir;
use trane::{
    course_builder::CourseBuilder,
    course_library::CourseLibrary,
    data::{
        course_generator::trane_improvisation::{
            ImprovisationPassage, TraneImprovisationConfig, TraneImprovisationPreferences,
        },
        CourseGenerator, CourseManifest, LessonManifestBuilder, MasteryScore, UserPreferences,
    },
    testutil::{assert_simulation_scores, TraneSimulation},
    Trane, TRANE_CONFIG_DIR_PATH, USER_PREFERENCES_PATH,
};
use ustr::Ustr;

/// Returns a course builder with a Trane Improvisation generator.
fn trane_improvisation_builder(
    course_id: Ustr,
    course_index: usize,
    dependencies: Vec<Ustr>,
    num_passages: usize,
    rhythm_only: bool,
) -> CourseBuilder {
    let mut passages = HashMap::new();
    for i in 0..num_passages {
        let passage = ImprovisationPassage {
            soundslice_link: format!("https://www.soundslice.com/slices/{}/", i),
            music_xml_file: None,
        };
        passages.insert(i, passage);
    }

    CourseBuilder {
        directory_name: format!("improv_course_{}", course_index),
        course_manifest: CourseManifest {
            id: course_id,
            name: format!("Course {}", course_id),
            dependencies: vec![],
            description: None,
            authors: None,
            metadata: None,
            course_material: None,
            course_instructions: None,
            generator_config: Some(CourseGenerator::TraneImprovisation(
                TraneImprovisationConfig {
                    improvisation_dependencies: dependencies,
                    rhythm_only,
                    passages,
                },
            )),
        },
        lesson_manifest_template: LessonManifestBuilder::default().clone(),
        lesson_builders: vec![],
        asset_builders: vec![],
    }
}

/// Creates the courses, initializes the Trane library, and returns a Trane instance.
fn init_improv_simulation(
    library_root: &Path,
    course_builders: &Vec<CourseBuilder>,
    user_preferences: Option<&UserPreferences>,
) -> Result<Trane> {
    // Build the courses.
    course_builders
        .into_iter()
        .map(|course_builder| course_builder.build(library_root))
        .collect::<Result<()>>()?;

    // Write the user preferences if provided.
    if let Some(user_preferences) = user_preferences {
        let config_dir = library_root.join(TRANE_CONFIG_DIR_PATH);
        create_dir(config_dir.clone())?;
        let prefs_path = config_dir.join(USER_PREFERENCES_PATH);
        let mut file = File::create(prefs_path.clone())?;
        let prefs_json = serde_json::to_string_pretty(user_preferences)? + "\n";
        file.write_all(prefs_json.as_bytes())?;
    }

    // Initialize the Trane library.
    let trane = Trane::new(library_root)?;
    Ok(trane)
}

/// A test that verifies that all Trane Improvisation exercises are visited.
#[test]
fn all_exercises_visited() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let first_course_id = Ustr::from("trane::test::improv_course_0");
    let second_course_id = Ustr::from("trane::test::improv_course_1");
    let user_prefs = UserPreferences {
        trane_improvisation: Some(TraneImprovisationPreferences {
            instruments: vec!["guitar".to_string(), "piano".to_string()],
        }),
    };
    let mut trane = init_improv_simulation(
        &temp_dir.path(),
        &vec![
            trane_improvisation_builder(first_course_id, 0, vec![], 5, false),
            trane_improvisation_builder(second_course_id, 1, vec![first_course_id], 5, false),
        ],
        Some(&user_prefs),
    )?;

    // Run the simulation.
    let exercise_ids = trane.get_all_exercise_ids()?;
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

/// A test that verifies that all Trane Improvisation exercises are visited when no instruments are
/// specified.
#[test]
fn all_exercises_visited_no_instruments() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let first_course_id = Ustr::from("trane::test::improv_course_0");
    let second_course_id = Ustr::from("trane::test::improv_course_1");
    let mut trane = init_improv_simulation(
        &temp_dir.path(),
        &vec![
            trane_improvisation_builder(first_course_id, 0, vec![], 5, false),
            trane_improvisation_builder(second_course_id, 1, vec![first_course_id], 5, false),
        ],
        None,
    )?;

    // Run the simulation.
    let exercise_ids = trane.get_all_exercise_ids()?;
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

/// A test that verifies that all Trane Improvisation exercises are visited when only rhythm lessons
/// are specified in the configuration.
#[test]
fn all_exercises_visited_rhythm_only() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let first_course_id = Ustr::from("trane::test::improv_course_0");
    let second_course_id = Ustr::from("trane::test::improv_course_1");
    let user_prefs = UserPreferences {
        trane_improvisation: Some(TraneImprovisationPreferences {
            instruments: vec!["guitar".to_string(), "piano".to_string()],
        }),
    };
    let mut trane = init_improv_simulation(
        &temp_dir.path(),
        &vec![
            trane_improvisation_builder(first_course_id, 0, vec![], 5, true),
            trane_improvisation_builder(second_course_id, 1, vec![first_course_id], 5, true),
        ],
        Some(&user_prefs),
    )?;

    // Run the simulation.
    let exercise_ids = trane.get_all_exercise_ids()?;
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

/// A test that verifies that not making progress on the singing lessons blocks all further
/// progress.
#[test]
fn no_progress_past_singing_lessons() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let first_course_id = Ustr::from("trane::test::improv_course_0");
    let second_course_id = Ustr::from("trane::test::improv_course_1");
    let user_prefs = UserPreferences {
        trane_improvisation: Some(TraneImprovisationPreferences {
            instruments: vec!["guitar".to_string(), "piano".to_string()],
        }),
    };
    let mut trane = init_improv_simulation(
        &temp_dir.path(),
        &vec![
            trane_improvisation_builder(first_course_id, 0, vec![], 5, false),
            trane_improvisation_builder(second_course_id, 1, vec![first_course_id], 5, false),
        ],
        Some(&user_prefs),
    )?;

    // Run the simulation. Give every exercise a score of one, which should block all further
    // progress past the starting lessons.
    let exercise_ids = trane.get_all_exercise_ids()?;
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 5,
        Box::new(|_| Some(MasteryScore::One)),
    );
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // Only exercises from the singing lessons are in the answer history.
    for exercise_id in exercise_ids {
        if exercise_id.contains("singing") {
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

// A test that verifies that not mastering the basic harmony lessons blocks all progress on the
// advanced harmony and mastery lessons.
#[test]
fn basic_harmony_blocks_advanced_harmony() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let first_course_id = Ustr::from("trane::test::improv_course_0");
    let second_course_id = Ustr::from("trane::test::improv_course_1");
    let user_prefs = UserPreferences {
        trane_improvisation: Some(TraneImprovisationPreferences {
            instruments: vec!["guitar".to_string(), "piano".to_string()],
        }),
    };
    let mut trane = init_improv_simulation(
        &temp_dir.path(),
        &vec![
            trane_improvisation_builder(first_course_id, 0, vec![], 5, false),
            trane_improvisation_builder(second_course_id, 1, vec![first_course_id], 5, false),
        ],
        Some(&user_prefs),
    )?;

    // Run the simulation. Give every exercise a score of five, except for the basic harmony
    // exercises, which should block progress to the advanced harmony and mastery lessons.
    let exercise_ids = trane.get_all_exercise_ids()?;
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

// A test that verifies that not mastering the basic harmony lessons blocks all progress on the
// advanced harmony and mastery lessons.
#[test]
fn advanced_harmony_blocks_mastery() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let first_course_id = Ustr::from("trane::test::improv_course_0");
    let second_course_id = Ustr::from("trane::test::improv_course_1");
    let user_prefs = UserPreferences {
        trane_improvisation: Some(TraneImprovisationPreferences {
            instruments: vec!["guitar".to_string(), "piano".to_string()],
        }),
    };
    let mut trane = init_improv_simulation(
        &temp_dir.path(),
        &vec![
            trane_improvisation_builder(first_course_id, 0, vec![], 5, false),
            trane_improvisation_builder(second_course_id, 1, vec![first_course_id], 5, false),
        ],
        Some(&user_prefs),
    )?;

    // Run the simulation. Give every exercise a score of five, except for the advanced harmony
    // exercises, which should block progress to the mastery lessons.
    let exercise_ids = trane.get_all_exercise_ids()?;
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

// A test that verifies that not mastering the melody lessons blocks all progress on the mastery
// lessons.
#[test]
fn melody_blocks_mastery() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let first_course_id = Ustr::from("trane::test::improv_course_0");
    let second_course_id = Ustr::from("trane::test::improv_course_1");
    let user_prefs = UserPreferences {
        trane_improvisation: Some(TraneImprovisationPreferences {
            instruments: vec!["guitar".to_string(), "piano".to_string()],
        }),
    };
    let mut trane = init_improv_simulation(
        &temp_dir.path(),
        &vec![
            trane_improvisation_builder(first_course_id, 0, vec![], 5, false),
            trane_improvisation_builder(second_course_id, 1, vec![first_course_id], 5, false),
        ],
        Some(&user_prefs),
    )?;

    // Run the simulation. Give every exercise a score of five, except for the melody exercises,
    // which should block all further progress past the starting lessons.
    let exercise_ids = trane.get_all_exercise_ids()?;
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

// A test that verifies that not mastering the rhythm lessons blocks all progress on the mastery
// lessons.
#[test]
fn rhythm_blocks_mastery() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let first_course_id = Ustr::from("trane::test::improv_course_0");
    let second_course_id = Ustr::from("trane::test::improv_course_1");
    let user_prefs = UserPreferences {
        trane_improvisation: Some(TraneImprovisationPreferences {
            instruments: vec!["guitar".to_string(), "piano".to_string()],
        }),
    };
    let mut trane = init_improv_simulation(
        &temp_dir.path(),
        &vec![
            trane_improvisation_builder(first_course_id, 0, vec![], 5, false),
            trane_improvisation_builder(second_course_id, 1, vec![first_course_id], 5, false),
        ],
        Some(&user_prefs),
    )?;

    // Run the simulation. Give every exercise a score of five, except for the rhythm exercises,
    // which should block all further progress past the starting lessons.
    let exercise_ids = trane.get_all_exercise_ids()?;
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

// A test that verifies that not mastering the sight-singing lessons blocks all progress on the
// instrument lessons.
#[test]
fn sight_singing_lessons_block_instruments() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let first_course_id = Ustr::from("trane::test::improv_course_0");
    let second_course_id = Ustr::from("trane::test::improv_course_1");
    let user_prefs = UserPreferences {
        trane_improvisation: Some(TraneImprovisationPreferences {
            instruments: vec!["guitar".to_string(), "piano".to_string()],
        }),
    };
    let mut trane = init_improv_simulation(
        &temp_dir.path(),
        &vec![
            trane_improvisation_builder(first_course_id, 0, vec![], 5, false),
            trane_improvisation_builder(second_course_id, 1, vec![first_course_id], 5, false),
        ],
        Some(&user_prefs),
    )?;

    // Run the simulation. Give all the exercises involving sight-singing and not an instrument a
    // score of one. Give all other exercises a score of five.
    let exercise_ids = trane.get_all_exercise_ids()?;
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
        if exercise_id.contains("piano") || exercise_id.contains("guitar") {
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
