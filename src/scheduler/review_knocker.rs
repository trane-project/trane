//! Defines the logic for eliminating and reducing review of exercises of high scores that are also
//! highly encompassed by the other exercises in the initial batch. The main goal is to reduce the
//! number of exercises whose material is covered by many other exercises.

use ustr::{Ustr, UstrMap, UstrSet};

use crate::{
    graph::UnitGraph,
    scheduler::{
        Candidate, RewardPropagator,
        data::SchedulerData,
        reward_propagator::{REWARD_FACTOR, WEIGHT_FACTOR},
    },
};

/// The score threshold of exercises belonging to the very highly encompassed category.
const VERY_HIGHLY_SCORE: f32 = 4.5;

/// The frequency threshold of exercises belonging to the very highly encompassed category. The
/// frequency is the number of times other lessons or courses in the initial batch encompassed the
/// exercise's lesson.
const VERY_HIGHLY_FREQUENCY: u32 = 10;

/// The score threshold of exercises belonging to the highly encompassed category.
const HIGHLY_SCORE: f32 = 3.75;

/// The frequency threshold of exercises belonging to the highly encompassed category. The frequency
/// is defined exactly as for the very highly encompassed category.
const HIGHLY_FREQUENCY: u32 = 5;

/// The result of knocking out reviews from the initial batch.
pub(super) struct KnockoutResult {
    /// The batch of candidates after completely removing the exercises in the very highly
    /// encompassed category.
    pub candidates: Vec<Candidate>,

    /// The mapping of exercise IDs to the number of courses and lessons that encompassed them in
    /// the initial batch. Exercises in the same lesson should only be counted once. The candidate
    /// filter will use them as a component of the weight assigned to exercises inside mastery
    /// windows.
    pub frequency_map: UstrMap<u32>,

    /// The candidates in the highly encompassed category. These exercises are not removed from the
    /// initial batch, but they are used by the candidate filter to put them in a mastery window
    /// with a lower percentage of the final total than they would otherwise be in.
    pub highly_encompassed: Vec<Candidate>,
}

/// An item in the frequency propagation queue.
struct FrequencyQueueItem {
    /// The unit ID.
    unit_id: Ustr,

    /// The value of the reward at the current step of propagation.
    reward: f32,

    /// The weight of the reward at the current step of propagation.
    weight: f32,
}

pub(super) struct ReviewKnocker {
    /// The data needed to run the review knocker.
    data: SchedulerData,
}

impl ReviewKnocker {
    pub fn new(data: SchedulerData) -> Self {
        Self { data }
    }

    /// Computes the encompassing frequency of each exercise in the initial batch by walking through
    /// the encompassed graph and keeping track of how many times the exercise's lesson and course
    /// are reached. Exercises in the same lesson should not be counted more than once.
    fn compute_frequency_map(
        initial_batch: &[Candidate],
        unit_graph: &dyn UnitGraph,
    ) -> UstrMap<u32> {
        // Initialize the frequency map and the set of set of lessons and courses to traverse.
        let mut unit_frequency_map = UstrMap::default();
        let unit_set = initial_batch
            .iter()
            .flat_map(|candidate| vec![candidate.lesson_id, candidate.course_id])
            .collect::<UstrSet>();

        // For each, find all their encompassed lessons and courses.
        for unit_id in unit_set {
            // Initialize the stack and set of visited units.
            let mut stack: Vec<FrequencyQueueItem> = Vec::new();
            stack.push(FrequencyQueueItem {
                unit_id,
                reward: 1.0,
                weight: 1.0,
            });
            let mut visited = UstrSet::default();

            // Traverse the graph to propagate and accumulate the encompassing frequency.
            while let Some(FrequencyQueueItem {
                unit_id,
                reward,
                weight,
            }) = stack.pop()
            {
                // Skip if the unit has already been visited.
                if visited.contains(&unit_id) {
                    continue;
                }
                visited.insert(unit_id);

                // Get the units encompassed by the unit and update their frequencies.
                if let Some(encompassed_units) = unit_graph.get_encompasses(unit_id) {
                    for (encompassed_id, encompassed_weight) in encompassed_units {
                        // Ignore edge if the weight is 0 or propagation should be stopped.
                        if encompassed_weight == 0.0
                            || RewardPropagator::stop_propagation(reward, weight)
                        {
                            continue;
                        }

                        // Update the frequency and update the stack with the next units.
                        let entry = unit_frequency_map.entry(encompassed_id).or_insert(0);
                        *entry += 1;
                        stack.push(FrequencyQueueItem {
                            unit_id: encompassed_id,
                            reward: reward * REWARD_FACTOR,
                            weight: encompassed_weight * weight * WEIGHT_FACTOR,
                        });
                        if let Some(course_id) = unit_graph.get_lesson_course(encompassed_id) {
                            stack.push(FrequencyQueueItem {
                                unit_id: course_id,
                                reward: reward * REWARD_FACTOR,
                                weight: encompassed_weight * weight * WEIGHT_FACTOR,
                            });
                        }
                    }
                }
            }
        }

        // Convert the unit frequency map to an exercise frequency map by mapping each exercise to
        // the maximum frequency of its lesson and course.
        let mut exercise_frequency_map = UstrMap::default();
        for candidate in initial_batch {
            let lesson_frequency = unit_frequency_map
                .get(&candidate.lesson_id)
                .copied()
                .unwrap_or(0);
            let course_frequency = unit_frequency_map
                .get(&candidate.course_id)
                .copied()
                .unwrap_or(0);
            let frequency = lesson_frequency.max(course_frequency);
            exercise_frequency_map.insert(candidate.exercise_id, frequency);
        }
        exercise_frequency_map
    }

    /// Removes the very highly encompassed exercises from the initial batch.
    fn remove_very_highly_encompassed(
        initial_batch: Vec<Candidate>,
        frequency_map: &UstrMap<u32>,
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
        frequency_map: &UstrMap<u32>,
    ) -> Vec<Candidate> {
        let mut highly_encompassed = Vec::new();
        for candidate in initial_batch {
            if let Some(&frequency) = frequency_map.get(&candidate.exercise_id) {
                if frequency >= VERY_HIGHLY_FREQUENCY
                    && candidate.exercise_score >= VERY_HIGHLY_SCORE
                {
                    continue;
                }
                if frequency >= HIGHLY_FREQUENCY && candidate.exercise_score >= HIGHLY_SCORE {
                    highly_encompassed.push(candidate);
                }
            }
        }
        highly_encompassed
    }

    /// Performs the review knocking process on the initial batch of candidates and returns the
    /// result.
    pub(super) fn knock_out_reviews(&self, initial_batch: Vec<Candidate>) -> KnockoutResult {
        let unit_graph = self.data.unit_graph.read();
        let frequency_map = Self::compute_frequency_map(&initial_batch, &*unit_graph);
        let processed_batch = Self::remove_very_highly_encompassed(initial_batch, &frequency_map);
        let highly_encompassed =
            Self::get_highly_encompassed(processed_batch.clone(), &frequency_map);
        KnockoutResult {
            candidates: processed_batch,
            frequency_map,
            highly_encompassed,
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests {
    use super::*;
    use crate::graph::InMemoryUnitGraph;
    use anyhow::Result;

    /// Verifies that the frequency map is computed correctly when there are several exercises that
    /// are encompassed by many other exercises in the initial batch.
    #[test]
    fn test_compute_frequency_many_encompassed() -> Result<()> {
        // Construct a graph with many encompassing relationships.
        // Many courses that depend on the previous course, whose lessons depend on the previous
        // lesson.
        let num_courses = 20;
        let num_lessons_per_course = 5;
        let mut unit_graph = InMemoryUnitGraph::default();
        for course_index in 0..num_courses {
            // Add course and an encompassing relationship to the previous course.
            let course_id = Ustr::from(&format!("course_{}", course_index));
            unit_graph.add_course(course_id)?;
            if course_index > 0 {
                unit_graph.add_encompassed(
                    course_id,
                    &vec![Ustr::from(&format!("course_{}", course_index - 1))],
                    &vec![],
                )?;
            }

            // Add lessons and dependencies on the previous lesson.
            for lesson_index in 0..num_lessons_per_course {
                let lesson_id =
                    Ustr::from(&format!("course_{}_lesson_{}", course_index, lesson_index));
                unit_graph.add_lesson(lesson_id, course_id)?;
                if lesson_index > 0 {
                    unit_graph.add_encompassed(
                        lesson_id,
                        &vec![Ustr::from(&format!(
                            "course_{}_lesson_{}",
                            course_index,
                            lesson_index - 1
                        ))],
                        &vec![],
                    )?;
                }
            }
        }

        // Compute the frequency map for an initial batch containing one exercises from each lesson.
        let initial_batch = (0..num_courses)
            .flat_map(|course_index| {
                (0..num_lessons_per_course).map(move |lesson_index| Candidate {
                    exercise_id: Ustr::from(&format!("ex{}_{}", course_index, lesson_index)),
                    exercise_score: 4.5,
                    lesson_id: Ustr::from(&format!(
                        "course_{}_lesson_{}",
                        course_index, lesson_index
                    )),
                    course_id: Ustr::from(&format!("course_{}", course_index)),
                    course_score: 0.0,
                    depth: 0.0,
                    frequency: 0,
                    dead_end: false,
                    lesson_score: 0.0,
                    num_trials: 0,
                    last_seen: 0.0,
                })
            })
            .collect::<Vec<Candidate>>();

        // Compute the map and assert some heuristics are true. All exercise in the first five
        // courses should have a frequency higher than the threshold
        let frequency_map = ReviewKnocker::compute_frequency_map(&initial_batch, &unit_graph);
        for course_id in 0..5 {
            for lesson_id in 0..num_lessons_per_course {
                let exercise_id = Ustr::from(&format!("ex{}_{}", course_id, lesson_id));
                let frequency = frequency_map.get(&exercise_id).copied().unwrap_or(0);
                assert!(
                    frequency >= VERY_HIGHLY_FREQUENCY,
                    "Exercise {} should have frequency higher than the threshold, but has frequency {}",
                    exercise_id,
                    frequency
                );
            }
        }

        // Exercises in the very last course and lesson should have frequency 0, since they are not
        // encompassed by any other course or lesson.
        let last_exercise_id = Ustr::from(&format!(
            "ex{}_{}",
            num_courses - 1,
            num_lessons_per_course - 1
        ));
        let frequency = frequency_map.get(&last_exercise_id).copied().unwrap_or(0);
        assert_eq!(
            frequency, 0,
            "Exercise {} should have frequency 0, but has frequency {}",
            last_exercise_id, frequency
        );
        Ok(())
    }

    /// Verifies that the frequency map is computed correctly when there are none or very few
    /// exercises that are encompassed by many other exercises in the initial batch.
    #[test]
    fn test_compute_frequency_few_encompassed() -> Result<()> {
        // Construct a graph with multiple courses and lessons but no encompassing relationships.
        let num_courses = 5;
        let num_lessons_per_course = 3;
        let mut unit_graph = InMemoryUnitGraph::default();
        for course_index in 0..num_courses {
            let course_id = Ustr::from(&format!("course_{}", course_index));
            unit_graph.add_course(course_id)?;

            for lesson_index in 0..num_lessons_per_course {
                let lesson_id =
                    Ustr::from(&format!("course_{}_lesson_{}", course_index, lesson_index));
                unit_graph.add_lesson(lesson_id, course_id)?;
            }
        }

        // Compute the frequency map for an initial batch containing one exercise from each lesson.
        let initial_batch = (0..num_courses)
            .flat_map(|course_index| {
                (0..num_lessons_per_course).map(move |lesson_index| Candidate {
                    exercise_id: Ustr::from(&format!("ex{}_{}", course_index, lesson_index)),
                    exercise_score: 4.5,
                    lesson_id: Ustr::from(&format!(
                        "course_{}_lesson_{}",
                        course_index, lesson_index
                    )),
                    course_id: Ustr::from(&format!("course_{}", course_index)),
                    course_score: 0.0,
                    depth: 0.0,
                    frequency: 0,
                    dead_end: false,
                    lesson_score: 0.0,
                    num_trials: 0,
                    last_seen: 0.0,
                })
            })
            .collect::<Vec<Candidate>>();

        // Compute the map and assert that all exercises have frequency 0, since there are no
        // encompassing relationships.
        let frequency_map = ReviewKnocker::compute_frequency_map(&initial_batch, &unit_graph);
        for course_index in 0..num_courses {
            for lesson_index in 0..num_lessons_per_course {
                let exercise_id = Ustr::from(&format!("ex{}_{}", course_index, lesson_index));
                let frequency = frequency_map.get(&exercise_id).copied().unwrap_or(0);
                assert_eq!(
                    frequency, 0,
                    "Exercise {} should have frequency 0, but has frequency {}",
                    exercise_id, frequency
                );
            }
        }
        Ok(())
    }

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
                dead_end: false,
                lesson_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
            },
            Candidate {
                exercise_id: Ustr::from("ex2"),
                exercise_score: 3.5,
                lesson_id: Ustr::from("lesson2"),
                course_id: Ustr::from("course1"),
                course_score: 0.0,
                depth: 0.0,
                frequency: 0,
                dead_end: false,
                lesson_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
            },
            Candidate {
                exercise_id: Ustr::from("ex3"),
                exercise_score: 2.0,
                lesson_id: Ustr::from("lesson3"),
                course_id: Ustr::from("course2"),
                course_score: 0.0,
                depth: 0.0,
                frequency: 0,
                dead_end: false,
                lesson_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
            },
        ];

        let mut frequency_map = UstrMap::default();
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
                dead_end: false,
                lesson_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
            },
            Candidate {
                exercise_id: Ustr::from("ex2"),
                exercise_score: 3.9,
                lesson_id: Ustr::from("lesson2"),
                course_id: Ustr::from("course1"),
                course_score: 0.0,
                depth: 0.0,
                frequency: 0,
                dead_end: false,
                lesson_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
            },
            Candidate {
                exercise_id: Ustr::from("ex3"),
                exercise_score: 2.0,
                lesson_id: Ustr::from("lesson3"),
                course_id: Ustr::from("course2"),
                course_score: 0.0,
                depth: 0.0,
                frequency: 0,
                dead_end: false,
                lesson_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
            },
            Candidate {
                exercise_id: Ustr::from("ex4"),
                exercise_score: 3.8,
                lesson_id: Ustr::from("lesson2"),
                course_id: Ustr::from("course1"),
                course_score: 0.0,
                depth: 0.0,
                frequency: 0,
                dead_end: false,
                lesson_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
            },
            Candidate {
                exercise_id: Ustr::from("ex5"),
                exercise_score: 4.0,
                lesson_id: Ustr::from("lesson4"),
                course_id: Ustr::from("course2"),
                course_score: 0.0,
                depth: 0.0,
                frequency: 0,
                dead_end: false,
                lesson_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
            },
        ];

        let mut frequency_map = UstrMap::default();
        frequency_map.insert(Ustr::from("ex1"), 12);
        frequency_map.insert(Ustr::from("ex2"), 8);
        frequency_map.insert(Ustr::from("ex3"), 2);
        frequency_map.insert(Ustr::from("ex4"), 3);
        frequency_map.insert(Ustr::from("ex5"), 12);
        let result = ReviewKnocker::get_highly_encompassed(initial_batch, &frequency_map);
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|c| c.exercise_id == Ustr::from("ex2")));
        assert!(result.iter().any(|c| c.exercise_id == Ustr::from("ex5")));
        assert!(!result.iter().any(|c| c.exercise_id == Ustr::from("ex1")));
        assert!(!result.iter().any(|c| c.exercise_id == Ustr::from("ex3")));
        assert!(!result.iter().any(|c| c.exercise_id == Ustr::from("ex4")));
    }
}
