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

/// The weight threshold of exercises belonging to the very highly encompassed category. The weight
/// is the sum of the weights of the lessons or courses that reached the exercise through the
/// propagation process.
const VERY_HIGHLY_WEIGHT: f32 = 10.0;

/// The score threshold of exercises belonging to the highly encompassed category.
const HIGHLY_SCORE: f32 = 3.75;

/// The weight threshold of exercises belonging to the highly encompassed category. The weight
/// is defined exactly as for the very highly encompassed category.
const HIGHLY_WEIGHT: f32 = 5.0;

/// The result of knocking out reviews from the initial batch.
pub(super) struct KnockoutResult {
    /// The batch of candidates after completely removing the exercises in the very highly
    /// encompassed category.
    pub candidates: Vec<Candidate>,

    /// The candidates in the highly encompassed category. These exercises are not removed from the
    /// initial batch, but they are used by the candidate filter to put them in a mastery window
    /// with a lower percentage of the final total than they would otherwise be in.
    pub highly_encompassed: Vec<Candidate>,
}

/// An item in the weight propagation queue.
struct WeightQueueItem {
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

    /// Computes the encompassing weight of each exercise in the initial batch by walking through
    /// the encompassed graph and keeping track of how many times each exercise's lesson and course
    /// is encompassed by other lessons and courses in the graph.
    ///
    /// If reverse is true, the reverse graph is walked and the number of times the exercise's
    /// lesson and course encompasses other exercises in the initial batch is counted instead.
    fn compute_encompassing_map(
        initial_batch: &[Candidate],
        unit_graph: &dyn UnitGraph,
        reverse: bool,
    ) -> UstrMap<f32> {
        // Initialize the weight map and the set of set of lessons and courses to traverse.
        let mut unit_weight_map = UstrMap::default();
        let unit_set = initial_batch
            .iter()
            .flat_map(|candidate| vec![candidate.lesson_id, candidate.course_id])
            .collect::<UstrSet>();

        // For each, find all their encompassed lessons and courses.
        for unit_id in unit_set {
            // Initialize the stack and set of visited units.
            let mut stack: Vec<WeightQueueItem> = Vec::new();
            stack.push(WeightQueueItem {
                unit_id,
                reward: 1.0,
                weight: 1.0,
            });
            let mut visited = UstrSet::default();

            // Traverse the graph to propagate and accumulate the encompassing weight.
            while let Some(WeightQueueItem {
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

                // Get the units encompassed by the unit and update their weights.
                let next_units = if reverse {
                    unit_graph.get_encompassed_by(unit_id)
                } else {
                    unit_graph.get_encompasses(unit_id)
                };
                if let Some(next_units) = next_units {
                    for (next_unit_id, next_unit_weight) in next_units {
                        // Ignore edge if the weight is 0 or propagation should be stopped.
                        if next_unit_weight == 0.0
                            || RewardPropagator::stop_propagation(reward, weight)
                        {
                            continue;
                        }

                        // Update the weight and update the stack with the next units.
                        let entry = unit_weight_map.entry(next_unit_id).or_insert(0.0);
                        *entry += next_unit_weight;
                        stack.push(WeightQueueItem {
                            unit_id: next_unit_id,
                            reward: reward * REWARD_FACTOR,
                            weight: next_unit_weight * weight * WEIGHT_FACTOR,
                        });
                        if let Some(course_id) = unit_graph.get_lesson_course(next_unit_id) {
                            stack.push(WeightQueueItem {
                                unit_id: course_id,
                                reward: reward * REWARD_FACTOR,
                                weight: next_unit_weight * weight * WEIGHT_FACTOR,
                            });
                        }
                    }
                }
            }
        }

        // Convert the unit weight map to an exercise weight map by mapping each exercise to the
        // sum of the weights of its lesson and course.
        let mut exercise_weight_map = UstrMap::default();
        for candidate in initial_batch {
            let lesson_weight = unit_weight_map
                .get(&candidate.lesson_id)
                .copied()
                .unwrap_or(0.0);
            let course_weight = unit_weight_map
                .get(&candidate.course_id)
                .copied()
                .unwrap_or(0.0);
            exercise_weight_map.insert(candidate.exercise_id, lesson_weight + course_weight);
        }
        exercise_weight_map
    }

    /// Removes the very highly encompassed exercises from the initial batch.
    fn remove_very_highly_encompassed(
        candidates: Vec<Candidate>,
        weight_map: &UstrMap<f32>,
    ) -> Vec<Candidate> {
        candidates
            .into_iter()
            .filter(|candidate| {
                let weight = weight_map
                    .get(&candidate.exercise_id)
                    .copied()
                    .unwrap_or(0.0);
                !(weight >= VERY_HIGHLY_WEIGHT && candidate.exercise_score >= VERY_HIGHLY_SCORE)
            })
            .collect()
    }

    /// Returns a set containing the highly encompassed exercises.
    fn get_highly_encompassed(
        candidates: &[Candidate],
        weight_map: &UstrMap<f32>,
    ) -> Vec<Candidate> {
        let mut highly_encompassed = Vec::new();
        for candidate in candidates {
            if let Some(&weight) = weight_map.get(&candidate.exercise_id) {
                if weight >= VERY_HIGHLY_WEIGHT && candidate.exercise_score >= VERY_HIGHLY_SCORE {
                    continue;
                }
                if weight >= HIGHLY_WEIGHT && candidate.exercise_score >= HIGHLY_SCORE {
                    highly_encompassed.push(candidate.clone());
                }
            }
        }
        highly_encompassed
    }

    /// Performs the review knocking process on the initial batch of candidates and returns the
    /// result.
    pub(super) fn knock_out_reviews(&self, mut initial_batch: Vec<Candidate>) -> KnockoutResult {
        // Compute the encompassing weight maps in both directions. Update the candidates with the
        // encompasses weight so that the candidate filter can use it.
        let unit_graph = self.data.unit_graph.read();
        let encompassed_by_map =
            Self::compute_encompassing_map(&initial_batch, &*unit_graph, false);
        let encompasses_map = Self::compute_encompassing_map(&initial_batch, &*unit_graph, true);
        for candidate in &mut initial_batch {
            candidate.encompasses_weight = encompasses_map
                .get(&candidate.exercise_id)
                .copied()
                .unwrap_or(0.0);
            candidate.encompassed_weight = encompassed_by_map
                .get(&candidate.exercise_id)
                .copied()
                .unwrap_or(0.0);
        }

        // Remove the very highly encompassed exercises and identify the highly encompassed
        // exercises.
        let processed_batch =
            Self::remove_very_highly_encompassed(initial_batch, &encompassed_by_map);
        let highly_encompassed =
            Self::get_highly_encompassed(&processed_batch, &encompassed_by_map);
        KnockoutResult {
            candidates: processed_batch,
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

    /// Verifies that the weight map is computed correctly when there are several exercises that
    /// are encompassed by many other exercises in the initial batch.
    #[test]
    fn test_compute_weight_many_encompassed() -> Result<()> {
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

        // Compute the weight map for an initial batch containing one exercises from each lesson.
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
                    ..Default::default()
                })
            })
            .collect::<Vec<Candidate>>();

        // Compute the map and assert some heuristics are true. All exercise in the first five
        // courses should have a weight higher than the threshold
        let weight_map =
            ReviewKnocker::compute_encompassing_map(&initial_batch, &unit_graph, false);
        for course_id in 0..5 {
            for lesson_id in 0..num_lessons_per_course {
                let exercise_id = Ustr::from(&format!("ex{}_{}", course_id, lesson_id));
                let weight = weight_map.get(&exercise_id).copied().unwrap_or(0.0);
                assert!(weight >= VERY_HIGHLY_WEIGHT,);
            }
        }

        // Exercises in the very last course and lesson should have weight of 0, since they are not
        // encompassed by any other course or lesson.
        let last_exercise_id = Ustr::from(&format!(
            "ex{}_{}",
            num_courses - 1,
            num_lessons_per_course - 1
        ));
        let weight = weight_map.get(&last_exercise_id).copied().unwrap_or(0.0);
        assert_eq!(weight, 0.0);

        // Compute the reverse map. Exercises in the last few courses should have high weight
        // because they encompass many other exercises below them.
        let reverse_map =
            ReviewKnocker::compute_encompassing_map(&initial_batch, &unit_graph, true);
        for course_id in (num_courses - 5)..num_courses {
            for lesson_id in 0..num_lessons_per_course {
                let exercise_id = Ustr::from(&format!("ex{}_{}", course_id, lesson_id));
                let weight = reverse_map.get(&exercise_id).copied().unwrap_or(0.0);
                assert!(weight >= VERY_HIGHLY_WEIGHT,);
            }
        }

        // The first exercise in the first course should have weight 0 in the reverse map, since it
        // encompasses nothing.
        let first_exercise_id = Ustr::from("ex0_0");
        let weight = reverse_map.get(&first_exercise_id).copied().unwrap_or(0.0);
        assert_eq!(weight, 0.0);

        Ok(())
    }

    /// Verifies that the weight map is computed correctly when there are none or very few
    /// exercises that are encompassed by many other exercises in the initial batch.
    #[test]
    fn test_compute_weight_few_encompassed() -> Result<()> {
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

        // Compute the weight map for an initial batch containing one exercise from each lesson.
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
                    ..Default::default()
                })
            })
            .collect::<Vec<Candidate>>();

        // Compute the map and assert that all exercises have weight 0, since there are no
        // encompassing relationships.
        let weight_map =
            ReviewKnocker::compute_encompassing_map(&initial_batch, &unit_graph, false);
        for course_index in 0..num_courses {
            for lesson_index in 0..num_lessons_per_course {
                let exercise_id = Ustr::from(&format!("ex{}_{}", course_index, lesson_index));
                let weight = weight_map.get(&exercise_id).copied().unwrap_or(0.0);
                assert_eq!(weight, 0.0);
            }
        }

        // The reverse map should also be all zeros since there are no encompassing relationships.
        let reverse_map =
            ReviewKnocker::compute_encompassing_map(&initial_batch, &unit_graph, true);
        for course_index in 0..num_courses {
            for lesson_index in 0..num_lessons_per_course {
                let exercise_id = Ustr::from(&format!("ex{}_{}", course_index, lesson_index));
                let weight = reverse_map.get(&exercise_id).copied().unwrap_or(0.0);
                assert_eq!(weight, 0.0);
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
                ..Default::default()
            },
            Candidate {
                exercise_id: Ustr::from("ex2"),
                exercise_score: 3.5,
                lesson_id: Ustr::from("lesson2"),
                course_id: Ustr::from("course1"),
                ..Default::default()
            },
            Candidate {
                exercise_id: Ustr::from("ex3"),
                exercise_score: 2.0,
                lesson_id: Ustr::from("lesson3"),
                course_id: Ustr::from("course2"),
                ..Default::default()
            },
        ];

        let mut weight_map = UstrMap::default();
        weight_map.insert(Ustr::from("ex1"), 12.0);
        weight_map.insert(Ustr::from("ex2"), 8.0);
        weight_map.insert(Ustr::from("ex3"), 2.0);

        let result = ReviewKnocker::remove_very_highly_encompassed(initial_batch, &weight_map);

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
                ..Default::default()
            },
            Candidate {
                exercise_id: Ustr::from("ex2"),
                exercise_score: 3.9,
                lesson_id: Ustr::from("lesson2"),
                course_id: Ustr::from("course1"),
                ..Default::default()
            },
            Candidate {
                exercise_id: Ustr::from("ex3"),
                exercise_score: 2.0,
                lesson_id: Ustr::from("lesson3"),
                course_id: Ustr::from("course2"),
                ..Default::default()
            },
            Candidate {
                exercise_id: Ustr::from("ex4"),
                exercise_score: 3.8,
                lesson_id: Ustr::from("lesson2"),
                course_id: Ustr::from("course1"),
                ..Default::default()
            },
            Candidate {
                exercise_id: Ustr::from("ex5"),
                exercise_score: 4.0,
                lesson_id: Ustr::from("lesson4"),
                course_id: Ustr::from("course2"),
                ..Default::default()
            },
        ];

        let mut weight_map = UstrMap::default();
        weight_map.insert(Ustr::from("ex1"), 12.0);
        weight_map.insert(Ustr::from("ex2"), 8.0);
        weight_map.insert(Ustr::from("ex3"), 2.0);
        weight_map.insert(Ustr::from("ex4"), 3.0);
        weight_map.insert(Ustr::from("ex5"), 12.0);
        let result = ReviewKnocker::get_highly_encompassed(&initial_batch, &weight_map);
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|c| c.exercise_id == Ustr::from("ex2")));
        assert!(result.iter().any(|c| c.exercise_id == Ustr::from("ex5")));
        assert!(!result.iter().any(|c| c.exercise_id == Ustr::from("ex1")));
        assert!(!result.iter().any(|c| c.exercise_id == Ustr::from("ex3")));
        assert!(!result.iter().any(|c| c.exercise_id == Ustr::from("ex4")));
    }
}
