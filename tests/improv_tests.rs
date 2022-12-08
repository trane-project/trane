//! End-to-end tests to verify that the Trane Improvisation course generator works as expected.

use anyhow::Result;
use std::{collections::HashMap, fs::File, io::Write, path::Path};
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
    Trane,
};
use ustr::Ustr;

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

pub fn init_improv_simulation(
    library_directory: &Path,
    course_builders: &Vec<CourseBuilder>,
    user_preferences: &UserPreferences,
) -> Result<Trane> {
    // Build the courses.
    course_builders
        .into_iter()
        .map(|course_builder| course_builder.build(library_directory))
        .collect::<Result<()>>()?;

    // Write the user preferences.
    let prefs_path = library_directory.join("user_preferences.json");
    let mut file = File::create(prefs_path.clone())?;
    let prefs_json = serde_json::to_string_pretty(user_preferences)? + "\n";
    file.write_all(prefs_json.as_bytes())?;

    // Initialize the Trane library.
    let trane = Trane::new(library_directory)?;
    Ok(trane)
}

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
        &user_prefs,
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
        &user_prefs,
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
