//! End-to-end tests for the knowledge base course.

use std::path::Path;

use anyhow::Result;
use rand::Rng;
use tempfile::TempDir;
use trane::{
    course_builder::{
        knowledge_base_builder::{CourseBuilder, ExerciseBuilder, LessonBuilder},
        AssetBuilder,
    },
    course_library::CourseLibrary,
    data::{
        course_generator::knowledge_base::{
            KnowledgeBaseConfig, KnowledgeBaseExercise, KnowledgeBaseLesson,
        },
        CourseGenerator, CourseManifest, MasteryScore,
    },
    testutil::TraneSimulation,
    Trane,
};
use ustr::Ustr;

/// Generates a random number of dependencies for the lesson with the given index. All dependencies
/// will have a lower index to avoid cycles.
fn generate_lesson_dependencies(lesson_index: usize, rng: &mut impl Rng) -> Vec<Ustr> {
    let num_dependencies = rng.gen_range(0..=lesson_index) as usize;
    if num_dependencies == 0 {
        return vec![];
    }

    let mut dependencies = Vec::with_capacity(num_dependencies);
    for _ in 0..num_dependencies.min(lesson_index) {
        let dependency_id = Ustr::from(&format!("lesson_{}", rng.gen_range(0..lesson_index)));
        if dependencies.contains(&dependency_id) {
            continue;
        }
        dependencies.push(dependency_id);
    }
    dependencies
}

// Build a course with a given number of lessons and exercises per lesson. The dependencies are
// randomly generated.
fn knowledge_base_builder(
    directory_name: &str,
    course_manifest: CourseManifest,
    num_lessons: usize,
    num_exercises_per_lesson: usize,
) -> CourseBuilder {
    // Create the required number of lesson builders.
    let lessons = (0..num_lessons)
        .map(|lesson_index| {
            // Create the required number of exercise builders.
            let lesson_id = Ustr::from(&format!("lesson_{}", lesson_index));
            let exercises = (0..num_exercises_per_lesson)
                .map(|exercise_index| {
                    let front_path = format!("exercise_{}.front.md", exercise_index);
                    // Let even exercises have a back file and odds have none.
                    let back_path = if exercise_index % 2 == 0 {
                        Some(format!("exercise_{}.back.md", exercise_index))
                    } else {
                        None
                    };

                    // Create the asset and exercise builders.
                    let mut asset_builders = vec![AssetBuilder {
                        file_name: front_path.clone(),
                        contents: "Front".into(),
                    }];
                    if let Some(back_path) = &back_path {
                        asset_builders.push(AssetBuilder {
                            file_name: back_path.clone(),
                            contents: "Back".into(),
                        });
                    }
                    ExerciseBuilder {
                        exercise: KnowledgeBaseExercise {
                            short_id: format!("exercise_{}", exercise_index),
                            short_lesson_id: lesson_id,
                            course_id: course_manifest.id,
                            front_file: front_path,
                            back_file: back_path,
                            name: None,
                            description: None,
                            exercise_type: None,
                        },
                        asset_builders,
                    }
                })
                .collect();

            // Create the lesson builder.
            LessonBuilder {
                lesson: KnowledgeBaseLesson {
                    short_id: lesson_id,
                    course_id: course_manifest.id,
                    dependencies: Some(generate_lesson_dependencies(
                        lesson_index,
                        &mut rand::thread_rng(),
                    )),
                    name: None,
                    description: None,
                    metadata: None,
                    has_instructions: false,
                    has_material: false,
                },
                exercises,
                asset_builders: vec![],
            }
        })
        .collect();

    // Create the course builder.
    CourseBuilder {
        directory_name: directory_name.into(),
        lessons,
        assets: vec![],
        manifest: course_manifest,
    }
}

/// Creates the courses, initializes the Trane library, and returns a Trane instance.
fn init_knowledge_base_simulation(
    library_root: &Path,
    course_builders: &Vec<CourseBuilder>,
) -> Result<Trane> {
    // Build the courses.
    course_builders
        .into_iter()
        .map(|course_builder| course_builder.build(library_root))
        .collect::<Result<()>>()?;

    // Initialize the Trane library.
    let trane = Trane::new(library_root, library_root)?;
    Ok(trane)
}

// Verifies that generated knowledge base courses can be loaded and all their exercises can be
// reached.
#[test]
fn all_exercises_visited() -> Result<()> {
    let course1_builder = knowledge_base_builder(
        "course1",
        CourseManifest {
            id: Ustr::from("course1"),
            name: "Course 1".into(),
            description: None,
            dependencies: vec![],
            authors: None,
            metadata: None,
            course_material: None,
            course_instructions: None,
            generator_config: Some(CourseGenerator::KnowledgeBase(KnowledgeBaseConfig {})),
        },
        10,
        5,
    );
    let course2_builder = knowledge_base_builder(
        "course2",
        CourseManifest {
            id: Ustr::from("course2"),
            name: "Course 2".into(),
            description: None,
            dependencies: vec!["course1".into()],
            authors: None,
            metadata: None,
            course_material: None,
            course_instructions: None,
            generator_config: Some(CourseGenerator::KnowledgeBase(KnowledgeBaseConfig {})),
        },
        10,
        5,
    );

    // Initialize the Trane library.
    let temp_dir = TempDir::new()?;
    let mut trane =
        init_knowledge_base_simulation(&temp_dir.path(), &vec![course1_builder, course2_builder])?;

    // Run the simulation.
    let exercise_ids = trane.get_all_exercise_ids();
    assert!(exercise_ids.len() > 0);
    let mut simulation = TraneSimulation::new(
        exercise_ids.len() * 10,
        Box::new(|_| Some(MasteryScore::Five)),
    );
    simulation.run_simulation(&mut trane, &vec![], None)?;

    // Find all the exercises in the simulation history. All exercises should be visited.
    let visited_exercises = simulation.answer_history.keys().collect::<Vec<_>>();
    assert_eq!(visited_exercises.len(), exercise_ids.len());
    Ok(())
}
