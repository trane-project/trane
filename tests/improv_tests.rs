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
