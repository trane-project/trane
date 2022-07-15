use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
};

use anyhow::{anyhow, Result};
use chrono::Utc;
use trane::{
    blacklist::Blacklist,
    course_builder::{AssetBuilder, CourseBuilder, ExerciseBuilder, LessonBuilder},
    data::{
        filter::UnitFilter, CourseManifest, ExerciseAsset, ExerciseManifest,
        ExerciseManifestBuilder, ExerciseType, LessonManifestBuilder, MasteryScore,
    },
    practice_stats::PracticeStats,
    scheduler::ExerciseScheduler,
    Trane,
};

/// Represents the ID of a tests unit. First element is the course ID, followed by optional lesson
/// and exercise IDs.
#[derive(Debug, Clone, PartialEq)]
pub struct TestId(pub u32, pub Option<u32>, pub Option<u32>);

impl TestId {
    /// Returns whether the exercise ID is part of the given lesson.
    #[allow(dead_code)]
    pub fn exercise_in_lesson(&self, lesson: &TestId) -> bool {
        self.0 == lesson.0 && self.1 == lesson.1 && self.2.is_some()
    }

    /// Returns whether the exercise ID is part of the given course.
    #[allow(dead_code)]
    pub fn exercise_in_course(&self, course: &TestId) -> bool {
        self.0 == course.0 && self.1.is_some() && self.2.is_some()
    }
}

impl ToString for TestId {
    fn to_string(&self) -> String {
        let mut s = self.0.to_string();
        if let Some(lesson_id) = &self.1 {
            s.push_str("::");
            s.push_str(&lesson_id.to_string());
        }
        if let Some(exercise_id) = &self.2 {
            s.push_str("::");
            s.push_str(&exercise_id.to_string());
        }
        s
    }
}

/// Represents a test lesson, containing some number of dummy exercises.
pub struct TestLesson {
    /// ID of the lesson.
    pub id: TestId,

    /// Dependencies of the lesson.
    pub dependencies: Vec<TestId>,

    /// Metadata of the lesson.
    pub metadata: BTreeMap<String, Vec<String>>,

    /// Number of exercises in the lesson.
    pub num_exercises: usize,
}

impl TestLesson {
    /// Returns the lesson builder needed to generate the files for the lesson.
    fn lesson_builder(&self) -> Result<LessonBuilder> {
        if self.id.1.is_none() {
            return Err(anyhow!("Lesson ID is missing"));
        }
        if self.id.2.is_some() {
            return Err(anyhow!("Exercise ID is present"));
        }

        let exercise_builders = (0..self.num_exercises)
            .map(|i| {
                let id_clone = self.id.clone();
                ExerciseBuilder {
                    directory_name: format! {"exercise_{}", i.to_string()},
                    manifest_closure: Box::new(move |m| {
                        m.clone()
                            .id(TestId(id_clone.0, id_clone.1, Some(i as u32)).to_string())
                            .name(format! {"Exercise {}", i})
                            .clone()
                    }),
                    asset_builders: vec![
                        AssetBuilder {
                            file_name: "question.md".to_string(),
                            contents: "question".to_string(),
                        },
                        AssetBuilder {
                            file_name: "answer.md".to_string(),
                            contents: "answer".to_string(),
                        },
                    ],
                }
            })
            .collect::<Vec<_>>();

        let metadata_clone = self.metadata.clone();
        let id_clone = self.id.clone();
        let dependencies_clone = self.dependencies.clone();
        Ok(LessonBuilder {
            directory_name: format!("lesson_{}", self.id.1.unwrap()),
            manifest_closure: Box::new(move |m| {
                m.clone()
                    .id(id_clone.to_string())
                    .name(format! {"Lesson {}", id_clone.1.unwrap()})
                    .dependencies(dependencies_clone.iter().map(|id| id.to_string()).collect())
                    .metadata(Some(metadata_clone.clone()))
                    .clone()
            }),
            exercise_manifest_template: ExerciseManifestBuilder::default()
                .course_id(TestId(self.id.0, None, None).to_string())
                .lesson_id(self.id.to_string())
                .exercise_type(ExerciseType::Procedural)
                .exercise_asset(ExerciseAsset::FlashcardAsset {
                    front_path: "question.md".to_string(),
                    back_path: "answer.md".to_string(),
                })
                .clone(),
            asset_builders: vec![],
            exercise_builders: exercise_builders,
        })
    }
}

/// Represents a test course.
pub struct TestCourse {
    /// The ID of the course.
    pub id: TestId,

    /// The dependencies of the course.
    pub dependencies: Vec<TestId>,

    /// The metadata of the course.
    pub metadata: BTreeMap<String, Vec<String>>,

    /// The lessons in the course.
    pub lessons: Vec<TestLesson>,
}

impl TestCourse {
    pub fn course_builder(&self) -> Result<CourseBuilder> {
        if self.id.1.is_some() {
            return Err(anyhow!("Lesson ID is present"));
        }
        if self.id.2.is_some() {
            return Err(anyhow!("Exercise ID is present"));
        }
        for lesson in &self.lessons {
            if lesson.id.0 != self.id.0 {
                return Err(anyhow!("Course ID in lesson does not match course ID"));
            }
        }

        let lesson_builders = self
            .lessons
            .iter()
            .map(|lesson| lesson.lesson_builder())
            .collect::<Result<Vec<_>>>()?;
        Ok(CourseBuilder {
            directory_name: format!("course_{}", self.id.0),
            course_manifest: CourseManifest {
                id: self.id.to_string(),
                name: format!("Course {}", self.id.0),
                dependencies: self.dependencies.iter().map(|id| id.to_string()).collect(),
                description: None,
                authors: None,
                metadata: Some(self.metadata.clone()),
                course_material: None,
                course_instructions: None,
            },
            lesson_manifest_template: LessonManifestBuilder::default()
                .course_id(self.id.to_string())
                .clone(),
            lesson_builders,
            asset_builders: vec![],
        })
    }

    /// Returns the IDs of all the exercises in the course.
    fn all_exercises(&self) -> Vec<TestId> {
        let mut exercises = vec![];
        for lesson in &self.lessons {
            for exercise in 0..lesson.num_exercises {
                exercises.push(TestId(
                    self.id.0,
                    Some(lesson.id.1.unwrap()),
                    Some(exercise as u32),
                ));
            }
        }
        exercises
    }
}

pub fn all_exercises(courses: &Vec<TestCourse>) -> Vec<TestId> {
    let mut exercises = vec![];
    for course in courses {
        exercises.extend(course.all_exercises());
    }
    exercises
}

/// Simulates the responses to questions that are presented to the user and analyzes the results.
pub struct TraneSimulation {
    /// Number of exercises that will be presented to the user during the simulation.
    pub num_exercises: usize,

    /// Given an exercise ID, returns the mastery score for the exercise. A return value of None
    /// indicates that the exercise should be skipped.
    pub answer_closure: Box<dyn Fn(&str) -> Option<MasteryScore>>,

    /// Stores the entire history of exercises and their answers during the simulation.
    pub answer_history: HashMap<String, Vec<MasteryScore>>,
}

impl TraneSimulation {
    /// Constructs a new simulation object.
    pub fn new(
        num_questions: usize,
        answer_closure: Box<dyn Fn(&str) -> Option<MasteryScore>>,
    ) -> Self {
        Self {
            num_exercises: num_questions,
            answer_closure,
            answer_history: HashMap::new(),
        }
    }

    /// Runs the simulation with the given instance of Trane, unit blacklist, and filter.
    pub fn run_simulation(
        &mut self,
        trane: &mut Trane,
        blacklist: &Vec<TestId>,
        filter: Option<&UnitFilter>,
    ) -> Result<()> {
        for unit_id in blacklist {
            trane.add_unit(&unit_id.to_string())?;
        }

        let mut completed_exercises = 0;
        let mut batch: Vec<(String, ExerciseManifest)> = vec![];
        while completed_exercises < self.num_exercises {
            completed_exercises += 1;
            if batch.is_empty() {
                batch = trane.get_exercise_batch(filter)?;
            }
            if batch.is_empty() {
                break;
            }

            let (exercise_id, _) = batch.pop().unwrap();
            let score = (self.answer_closure)(&exercise_id);
            if score.is_some() {
                trane.score_exercise(
                    &exercise_id,
                    score.clone().unwrap(),
                    Utc::now().timestamp(),
                )?;
                self.answer_history
                    .entry(exercise_id.to_string())
                    .or_insert(vec![])
                    .push(score.unwrap());
            }
        }

        Ok(())
    }
}

/// Takes the given courses and builds them in the given directory. Returns a fully initialized
/// instance of Trane with the courses loaded.
pub fn init_trane(library_directory: &PathBuf, courses: &Vec<TestCourse>) -> Result<Trane> {
    for course in courses.iter() {
        course.course_builder()?.build(library_directory)?;
    }
    let trane = Trane::new(library_directory.to_str().unwrap())?;
    Ok(trane)
}

/// Asserts that the scores in the simulation match the scores reported by Trane for the given
/// exercise.
pub fn assert_scores(
    exercise_id: &TestId,
    trane: &Trane,
    simulation_scores: &HashMap<String, Vec<MasteryScore>>,
) -> Result<()> {
    // Get the last ten scores in the interest of saving time.
    let trane_scores = trane.get_scores(&exercise_id.to_string(), 10)?;

    // Check that the last ten simulation scores equal trane_scores.
    let simulation_scores = simulation_scores
        .get(&exercise_id.to_string())
        .ok_or(anyhow!(
            "No simulation scores for exercise with ID {:?}",
            exercise_id
        ))?;
    let most_recent_scores = simulation_scores.iter().rev().take(trane_scores.len());
    let _: Vec<()> =
        most_recent_scores
            .zip(trane_scores.iter())
            .map(|(simulation_score, trial)| {
                assert_eq!(
                trial.score,
                simulation_score.float_score(),
                "Score from Trane ({}) does not match score from simulation ({}) for exercise {}",
                trial.score, simulation_score.float_score(), exercise_id.to_string()
            );
            })
            .collect();

    Ok(())
}
