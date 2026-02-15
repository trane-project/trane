//! Defines the logic for eliminating and reducing review of exercises of high scores that are also
//! highly encompassed by the other exercises in the initial batch. The main goal is to reduce the
//! number of exercises whose material is covered by many other exercises.

use std::collections::HashMap;
use ustr::{Ustr, UstrSet};

use crate::{
    graph::UnitGraph,
    scheduler::{
        Candidate,
        data::SchedulerData,
        reward_propagator::{MIN_ABS_REWARD, MIN_WEIGHT, REWARD_FACTOR, WEIGHT_FACTOR},
    },
};

/// The score threshold of exercises belonging to the very highly encompassed category.
const VERY_HIGHLY_SCORE: f32 = 4.0;

/// The frequency threshold of exercises belonging to the very highly encompassed category. The
/// frequency is the number of times other lessons or courses in the initial batch encompassed the
/// exercise's lesson.
const VERY_HIGHLY_FREQUENCY: u32 = 10;

/// The score threshold of exercises belonging to the highly encompassed category.
const HIGHLY_SCORE: f32 = 3.0;

/// The frequency threshold of exercises belonging to the highly encompassed category. The frequency
/// is defined exactly as for the very highly encompassed category.
const HIGHLY_FREQUENCY: u32 = 5;

/// The result of knocking out reviews from the initial batch.
pub(super) struct KnockoutResult {
    /// The batch of candidates after completely removing the exercises in the very highly
    /// encompassed category.
    processed_batch: Vec<Candidate>,

    /// The mapping of exercise IDs to the number of courses and lessons that encompassed them in
    /// the initial batch. Exercises in the same lesson should only be counted once. The candidate
    /// filter will use them as a component of the weight assigned to exercises inside mastery
    /// windows.
    pub frequency_map: HashMap<Ustr, u32>,

    /// A set containing the IDs of the highly encompassed exercises. These exercises are not
    /// removed from the initial batch, but they are used by the candidate filter to put them in a
    /// mastery window with a lower percentage of the final total than they would otherwise be in.
    pub highly_encompassed: UstrSet,
}

/// An item in the frequency propagation queue.
struct FrequencyQueueItem {
    /// The unit ID.
    unit_id: Ustr,

    /// The weight of the frequency for this unit.
    weight: f32,
}

pub(super) struct ReviewKnocker {
    /// The data needed to run the review knocker.
    data: SchedulerData,
}

impl ReviewKnocker {
    /// Computes the encompassing frequency of each exercise in the initial batch by walking through
    /// the encompassed graph and keeping track of how many times the exercise's lesson and course
    /// are reached. Exercises in the same lesson should not be counted more than once.
    fn compute_frequency_map(
        initial_batch: Vec<Candidate>,
        unit_graph: &dyn UnitGraph,
    ) -> HashMap<Ustr, u32> {
        // TODO(agent): Implement this function. The implementation MUST meet the following
        // requirements:
        // - The frequency traversal should take inspiration from the reward propagation traversal
        //   implemented in reward_propagator.rs.
        // - It should use the same termination criteria except for units being declarative.
        //   Completely ignore that.
        // - The initial unts to traverse are the set of lessons and courses in the batch.
        HashMap::new()
    }

    /// Removes the very highly encompassed exercises from the initial batch.
    fn remove_very_highly_encompassed(
        initial_batch: Vec<Candidate>,
        frequency_map: &HashMap<Ustr, u32>,
    ) -> Vec<Candidate> {
        initial_batch
            .into_iter()
            .filter(|candidate| {
                let frequency = frequency_map
                    .get(&candidate.exercise_id)
                    .copied()
                    .unwrap_or(0);
                !(frequency >= VERY_HIGHLY_FREQUENCY
                    && candidate.exercise_score >= VERY_HIGHLY_SCORE)
            })
            .collect()
    }

    /// Returns a set containing the highly encompassed exercises.
    fn get_highly_encompassed(
        initial_batch: Vec<Candidate>,
        frequency_map: &HashMap<Ustr, u32>,
    ) -> UstrSet {
        let mut highly_encompassed = UstrSet::default();
        for candidate in initial_batch {
            if let Some(&frequency) = frequency_map.get(&candidate.exercise_id) {
                if frequency >= HIGHLY_FREQUENCY && candidate.exercise_score >= HIGHLY_SCORE {
                    highly_encompassed.insert(candidate.exercise_id);
                }
            }
        }
        highly_encompassed
    }

    /// Performs the review knocking process on the initial batch of candidates and returns the
    /// result.
    pub fn knock_out_reviews(&self, initial_batch: Vec<Candidate>) -> KnockoutResult {
        let unit_graph = self.data.unit_graph.read();
        let frequency_map = Self::compute_frequency_map(initial_batch.clone(), &*unit_graph);
        let processed_batch = Self::remove_very_highly_encompassed(initial_batch, &frequency_map);
        let highly_encompassed =
            Self::get_highly_encompassed(processed_batch.clone(), &frequency_map);
        KnockoutResult {
            processed_batch,
            frequency_map,
            highly_encompassed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{data::UnitType, graph::InMemoryUnitGraph};
    use anyhow::Result;

    /// Verifies that the frequency map is computed correctly when there are serveral exercises that
    /// are encompassed by many other exercises in the initial batch.
    #[test]
    fn test_compute_frequency_many_encompassed() -> Result<()> {
        // Construct a graph with many encompassing relationships (equal to dependencies for now).
        // Many courses that depend on the previous course, whose lessons depend on the previous
        // lesson.
        let num_courses = 20;
        let num_lessons_per_course = 5;
        let mut unit_graph = InMemoryUnitGraph::default();
        for course_index in 0..num_courses {
            // Add course and dependency on previous course.
            let course_id = Ustr::from(&format!("course_{}", course_index));
            unit_graph.add_course(course_id)?;
            if course_index > 0 {
                unit_graph.add_dependencies(
                    course_id,
                    UnitType::Course,
                    &vec![Ustr::from(&format!("course_{}", course_index - 1))],
                )?;
            }

            // Add lessons and dependencies on the previous lesson.
            for lesson_index in 0..num_lessons_per_course {
                let lesson_id =
                    Ustr::from(&format!("course_{}_lesson_{}", course_index, lesson_index));
                unit_graph.add_lesson(lesson_id, course_id)?;
                if lesson_index > 0 {
                    unit_graph.add_dependencies(
                        lesson_id,
                        UnitType::Lesson,
                        &vec![Ustr::from(&format!(
                            "course_{}_lesson_{}",
                            course_index,
                            lesson_index - 1
                        ))],
                    )?;
                }
            }
        }

        // Compute the frequency map for an initial batch containing one exercises from the first
        // and one from the last lesson.
        let initial_batch = vec![
            Candidate {
                exercise_id: Ustr::from("ex1"),
                exercise_score: 4.5,
                lesson_id: Ustr::from("course_0_lesson_0"),
                course_id: Ustr::from("course_0"),
                course_score: 0.0,
                depth: 0.0,
                frequency: 0,
                lesson_score: 0.0,
                num_trials: 0,
            },
            Candidate {
                exercise_id: Ustr::from("ex2"),
                exercise_score: 3.5,
                lesson_id: Ustr::from(&format!(
                    "course_{}_lesson_{}",
                    num_courses - 1,
                    num_lessons_per_course - 1
                )),
                course_id: Ustr::from(&format!("course_{}", num_courses - 1)),
                course_score: 0.0,
                depth: 0.0,
                frequency: 0,
                lesson_score: 0.0,
                num_trials: 0,
            },
        ];
        let frequency_map = ReviewKnocker::compute_frequency_map(initial_batch, &unit_graph);
        assert_eq!(frequency_map.get(&Ustr::from("ex1")), Some(&19));
        assert_eq!(frequency_map.get(&Ustr::from("ex2")), Some(&0));
        Ok(())
    }

    /// Verifies that the frequency map is computed correctly when there are none or very few
    /// exercises that are encompassed by many other exercises in the initial batch.
    #[test]
    fn test_compute_frequency_few_encompassed() {}

    /// Verifies that the very highly encompassed exercises are removed from the initial batch
    /// correctly.
    #[test]
    fn test_remove_very_highly_encompassed() {
        let initial_batch = vec![
            Candidate {
                exercise_id: Ustr::from("ex1"),
                exercise_score: 4.5,
                lesson_id: Ustr::from("lesson1"),
                course_id: Ustr::from("course1"),
                course_score: 0.0,
                depth: 0.0,
                frequency: 0,
                lesson_score: 0.0,
                num_trials: 0,
            },
            Candidate {
                exercise_id: Ustr::from("ex2"),
                exercise_score: 3.5,
                lesson_id: Ustr::from("lesson2"),
                course_id: Ustr::from("course1"),
                course_score: 0.0,
                depth: 0.0,
                frequency: 0,
                lesson_score: 0.0,
                num_trials: 0,
            },
            Candidate {
                exercise_id: Ustr::from("ex3"),
                exercise_score: 2.0,
                lesson_id: Ustr::from("lesson3"),
                course_id: Ustr::from("course2"),
                course_score: 0.0,
                depth: 0.0,
                frequency: 0,
                lesson_score: 0.0,
                num_trials: 0,
            },
        ];

        let mut frequency_map = HashMap::new();
        frequency_map.insert(Ustr::from("ex1"), 12);
        frequency_map.insert(Ustr::from("ex2"), 8);
        frequency_map.insert(Ustr::from("ex3"), 2);

        let result = ReviewKnocker::remove_very_highly_encompassed(initial_batch, &frequency_map);

        assert_eq!(result.len(), 2);
        assert!(!result.iter().any(|c| c.exercise_id == Ustr::from("ex1")));
        assert!(result.iter().any(|c| c.exercise_id == Ustr::from("ex2")));
        assert!(result.iter().any(|c| c.exercise_id == Ustr::from("ex3")));
    }

    /// Verifies that the highly encompassed exercises are identified correctly.
    #[test]
    fn test_get_highly_encompassed() {
        let initial_batch = vec![
            Candidate {
                exercise_id: Ustr::from("ex1"),
                exercise_score: 4.5,
                lesson_id: Ustr::from("lesson1"),
                course_id: Ustr::from("course1"),
                course_score: 0.0,
                depth: 0.0,
                frequency: 0,
                lesson_score: 0.0,
                num_trials: 0,
            },
            Candidate {
                exercise_id: Ustr::from("ex2"),
                exercise_score: 3.5,
                lesson_id: Ustr::from("lesson2"),
                course_id: Ustr::from("course1"),
                course_score: 0.0,
                depth: 0.0,
                frequency: 0,
                lesson_score: 0.0,
                num_trials: 0,
            },
            Candidate {
                exercise_id: Ustr::from("ex3"),
                exercise_score: 2.0,
                lesson_id: Ustr::from("lesson3"),
                course_id: Ustr::from("course2"),
                course_score: 0.0,
                depth: 0.0,
                frequency: 0,
                lesson_score: 0.0,
                num_trials: 0,
            },
        ];

        let mut frequency_map = HashMap::new();
        frequency_map.insert(Ustr::from("ex1"), 12);
        frequency_map.insert(Ustr::from("ex2"), 8);
        frequency_map.insert(Ustr::from("ex3"), 2);

        let result = ReviewKnocker::get_highly_encompassed(initial_batch, &frequency_map);

        assert_eq!(result.len(), 2);
        assert!(result.contains(&Ustr::from("ex1")));
        assert!(result.contains(&Ustr::from("ex2")));
        assert!(!result.contains(&Ustr::from("ex3")));
    }
}
