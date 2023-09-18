//! End-to-end tests to verify that the music piece course generator works as expected.

use anyhow::Result;
use lazy_static::lazy_static;
use tempfile::TempDir;
use trane::{
    course_builder::{AssetBuilder, CourseBuilder},
    course_library::CourseLibrary,
    data::{
        course_generator::music_piece::{MusicAsset, MusicPassage, MusicPieceConfig},
        CourseGenerator, CourseManifest, LessonManifestBuilder, MasteryScore,
    },
    testutil::{assert_simulation_scores, init_simulation, TraneSimulation},
};
use ustr::Ustr;

lazy_static! {
    static ref COURSE_ID: Ustr = Ustr::from("trane::test::music_piece_course");
    static ref SOUNDSLICE_MUSIC_ASSET: MusicAsset =
        MusicAsset::SoundSlice("soundslice_link".to_string());
    static ref LOCAL_MUSIC_ASSET: MusicAsset = MusicAsset::LocalFile("music_sheet.pdf".to_string());
    static ref COMPLEX_PASSAGE: TestPassage = TestPassage {
        sub_passages: vec![
            TestPassage {
                sub_passages: vec![
                    TestPassage {
                        sub_passages: vec![
                            TestPassage {
                                sub_passages: vec![],
                            },
                            TestPassage {
                                sub_passages: vec![],
                            },
                        ]
                    },
                    TestPassage {
                        sub_passages: vec![
                            TestPassage {
                                sub_passages: vec![]
                            },
                            TestPassage {
                                sub_passages: vec![]
                            },
                            TestPassage {
                                sub_passages: vec![]
                            },
                        ]
                    },
                ]
            },
            TestPassage {
                sub_passages: vec![
                    TestPassage {
                        sub_passages: vec![
                            TestPassage {
                                sub_passages: vec![]
                            },
                            TestPassage {
                                sub_passages: vec![]
                            },
                            TestPassage {
                                sub_passages: vec![]
                            },
                            TestPassage {
                                sub_passages: vec![]
                            },
                        ]
                    },
                    TestPassage {
                        sub_passages: vec![
                            TestPassage {
                                sub_passages: vec![]
                            },
                            TestPassage {
                                sub_passages: vec![]
                            },
                        ]
                    },
                    TestPassage {
                        sub_passages: vec![
                            TestPassage {
                                sub_passages: vec![]
                            },
                            TestPassage {
                                sub_passages: vec![]
                            },
                        ]
                    },
                ]
            },
        ]
    };
}

/// A simpler representation of a music passage for testing.
#[derive(Clone)]
struct TestPassage {
    sub_passages: Vec<TestPassage>,
}

impl From<TestPassage> for MusicPassage {
    fn from(test_passage: TestPassage) -> Self {
        MusicPassage {
            start: "passage start".to_string(),
            end: "passage end".to_string(),
            sub_passages: test_passage
                .sub_passages
                .into_iter()
                .enumerate()
                .map(|(index, passage)| (index, MusicPassage::from(passage)))
                .collect(),
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
            superseded: vec![],
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

/// Verifies that all music piece exercises are visited with a simple passage and a local file.
#[test]
fn all_exercises_visited_simple_local() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let passages = TestPassage {
        sub_passages: vec![],
    };
    let mut trane = init_simulation(
        &temp_dir.path(),
        &vec![music_piece_builder(
            *COURSE_ID,
            0,
            LOCAL_MUSIC_ASSET.clone(),
            MusicPassage::from(passages),
        )],
        None,
    )?;

    // Run the simulation.
    let exercise_ids = trane.get_all_exercise_ids(None);
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

/// Verifies that all music piece exercises are visited with a simple passage and a SoundSlice
/// asset.
#[test]
fn all_exercises_visited_simple_soundslice() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let passages = TestPassage {
        sub_passages: vec![],
    };
    let mut trane = init_simulation(
        &temp_dir.path(),
        &vec![music_piece_builder(
            *COURSE_ID,
            0,
            SOUNDSLICE_MUSIC_ASSET.clone(),
            MusicPassage::from(passages),
        )],
        None,
    )?;

    // Run the simulation.
    let exercise_ids = trane.get_all_exercise_ids(None);
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

/// Verifies that all music piece exercises are visited for a music piece with sub-passages.
#[test]
fn all_exercises_visited_complex() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_simulation(
        &temp_dir.path(),
        &vec![music_piece_builder(
            *COURSE_ID,
            0,
            SOUNDSLICE_MUSIC_ASSET.clone(),
            MusicPassage::from(COMPLEX_PASSAGE.clone()),
        )],
        None,
    )?;

    // Run the simulation.
    let exercise_ids = trane.get_all_exercise_ids(None);
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

/// Verifies that not all the exercises are visited when no progress is made with a complex passage.
#[test]
fn no_progress_complex() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let mut trane = init_simulation(
        &temp_dir.path(),
        &vec![music_piece_builder(
            *COURSE_ID,
            0,
            SOUNDSLICE_MUSIC_ASSET.clone(),
            MusicPassage::from(COMPLEX_PASSAGE.clone()),
        )],
        None,
    )?;

    // Run the simulation.
    let exercise_ids = trane.get_all_exercise_ids(None);
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

/// Verifies that all the exercises are visited when no progress is made with a simple passage,
/// since there is only one exercise in the course.
#[test]
fn no_progress_simple() -> Result<()> {
    // Initialize test course library.
    let temp_dir = TempDir::new()?;
    let passages = TestPassage {
        sub_passages: vec![],
    };
    let mut trane = init_simulation(
        &temp_dir.path(),
        &vec![music_piece_builder(
            *COURSE_ID,
            0,
            SOUNDSLICE_MUSIC_ASSET.clone(),
            MusicPassage::from(passages),
        )],
        None,
    )?;

    // Run the simulation.
    let exercise_ids = trane.get_all_exercise_ids(None);
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
