//! End-to-end tests to verify that the music piece course generator works as expected.

use anyhow::Result;
use lazy_static::lazy_static;
use std::path::Path;
use tempfile::TempDir;
use trane::{
    course_builder::{AssetBuilder, CourseBuilder},
    course_library::CourseLibrary,
    data::{
        course_generator::music_piece::{MusicAsset, MusicPassage, MusicPieceConfig},
        CourseGenerator, CourseManifest, LessonManifestBuilder, MasteryScore,
    },
    testutil::{assert_simulation_scores, TraneSimulation},
    Trane,
};
use ustr::Ustr;

lazy_static! {
    static ref COURSE_ID: Ustr = Ustr::from("trane::test::music_piece_course");
    static ref SOUNDSLICE_MUSIC_ASSET: MusicAsset =
        MusicAsset::SoundSlice("soundslice_link".to_string());
    static ref LOCAL_MUSIC_ASSET: MusicAsset = MusicAsset::LocalFile("music_sheet.pdf".to_string());
    static ref COMPLEX_PASSAGE: TestPassage = TestPassage::ComplexPassage(vec![
        TestPassage::ComplexPassage(vec![
            TestPassage::ComplexPassage(vec![
                TestPassage::SimplePassage,
                TestPassage::SimplePassage,
            ]),
            TestPassage::ComplexPassage(vec![
                TestPassage::SimplePassage,
                TestPassage::SimplePassage,
                TestPassage::SimplePassage,
            ]),
        ]),
        TestPassage::ComplexPassage(vec![
            TestPassage::ComplexPassage(vec![
                TestPassage::SimplePassage,
                TestPassage::SimplePassage,
                TestPassage::SimplePassage,
                TestPassage::SimplePassage,
            ]),
            TestPassage::ComplexPassage(vec![
                TestPassage::SimplePassage,
                TestPassage::SimplePassage,
            ]),
            TestPassage::ComplexPassage(vec![
                TestPassage::SimplePassage,
                TestPassage::SimplePassage,
            ]),
        ]),
    ]);
}

/// A simpler representation of a music passage for testing.
#[derive(Clone)]
enum TestPassage {
    SimplePassage,
    ComplexPassage(Vec<TestPassage>),
}

impl From<TestPassage> for MusicPassage {
    fn from(test_passage: TestPassage) -> Self {
        match test_passage {
            TestPassage::SimplePassage => MusicPassage::SimplePassage {
                start: "passage start".to_string(),
                end: "passage end".to_string(),
            },
            TestPassage::ComplexPassage(passages) => MusicPassage::ComplexPassage {
                start: "passage start".to_string(),
                end: "passage end".to_string(),
                sub_passages: passages
                    .into_iter()
                    .enumerate()
                    .map(|(index, passage)| (index, MusicPassage::from(passage)))
                    .collect(),
            },
        }
    }
}

/// Returns a course builder with a music piece course generator.
fn music_piece_builder(
    course_id: Ustr,
    course_index: usize,
    music_asset: MusicAsset,
    passages: MusicPassage,
) -> CourseBuilder {
    // If the music asset is a local file, generate its corresponding asset builder.
    let asset_builders = if let MusicAsset::LocalFile(path) = &music_asset {
        vec![AssetBuilder {
            file_name: path.clone(),
            contents: "music sheet contents".to_string(),
        }]
    } else {
        vec![]
    };

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
            generator_config: Some(CourseGenerator::MusicPiece(MusicPieceConfig {
                music_asset,
                passages,
            })),
        },
        lesson_manifest_template: LessonManifestBuilder::default().clone(),
        lesson_builders: vec![],
        asset_builders,
    }
}

/// Creates the courses, initializes the Trane library, and returns a Trane instance.
fn init_music_piece_simulation(
    library_root: &Path,
    course_builders: &Vec<CourseBuilder>,
) -> Result<Trane> {
    // Build the courses.
    course_builders
        .into_iter()
        .map(|course_builder| course_builder.build(library_root))
        .collect::<Result<()>>()?;

    // Initialize the Trane library.
    let trane = Trane::new(library_root)?;
    Ok(trane)
}

/// A test that verifies that all music piece exercises are visited with a simple passage and a
/// local file.
#[test]
fn all_exercises_visited_simple_local() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let passages = TestPassage::SimplePassage;
    let mut trane = init_music_piece_simulation(
        &temp_dir.path(),
        &vec![music_piece_builder(
            *COURSE_ID,
            0,
            LOCAL_MUSIC_ASSET.clone(),
            MusicPassage::from(passages),
        )],
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

/// A test that verifies that all music piece exercises are visited with a simple passage and a
/// soundslice asset.
#[test]
fn all_exercises_visited_simple_soundslice() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let passages = TestPassage::SimplePassage;
    let mut trane = init_music_piece_simulation(
        &temp_dir.path(),
        &vec![music_piece_builder(
            *COURSE_ID,
            0,
            SOUNDSLICE_MUSIC_ASSET.clone(),
            MusicPassage::from(passages),
        )],
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

/// A test that verifies that all music piece exercises are visited with a complex passage.
#[test]
fn all_exercises_visited_complex() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_music_piece_simulation(
        &temp_dir.path(),
        &vec![music_piece_builder(
            *COURSE_ID,
            0,
            SOUNDSLICE_MUSIC_ASSET.clone(),
            MusicPassage::from(COMPLEX_PASSAGE.clone()),
        )],
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

/// A test that verifies that not all the exercises are visited when no progress is made with a
/// complex passage.
#[test]
fn no_progress_complex() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_music_piece_simulation(
        &temp_dir.path(),
        &vec![music_piece_builder(
            *COURSE_ID,
            0,
            SOUNDSLICE_MUSIC_ASSET.clone(),
            MusicPassage::from(COMPLEX_PASSAGE.clone()),
        )],
    )?;

    // Run the simulation.
    let exercise_ids = trane.get_all_exercise_ids()?;
    assert!(exercise_ids.len() > 0);
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 5,
        Box::new(|_| Some(MasteryScore::One)),
    );
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // Find all the exercises in the simulation history. There should be less than the total number
    // of exercises.
    let visited_exercises = simulation.answer_history.keys().collect::<Vec<_>>();
    assert!(visited_exercises.len() < exercise_ids.len());
    Ok(())
}

/// A test that verifies that not all the exercises are visited when no progress is made with a
/// simple passage.
#[test]
fn no_progress_simple() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let passages = TestPassage::SimplePassage;
    let mut trane = init_music_piece_simulation(
        &temp_dir.path(),
        &vec![music_piece_builder(
            *COURSE_ID,
            0,
            SOUNDSLICE_MUSIC_ASSET.clone(),
            MusicPassage::from(passages),
        )],
    )?;

    // Run the simulation.
    let exercise_ids = trane.get_all_exercise_ids()?;
    assert!(exercise_ids.len() > 0);
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 5,
        Box::new(|_| Some(MasteryScore::One)),
    );
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // Find all the exercises in the simulation history. Given that there's only one exercise, it
    // has been visited.
    let visited_exercises = simulation.answer_history.keys().collect::<Vec<_>>();
    assert_eq!(visited_exercises.len(), exercise_ids.len());
    Ok(())
}
