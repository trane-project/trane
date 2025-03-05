//! Contains the main logic for propagating rewards through the graph. When an exercise submits a
//! score, Trane uses the score and the unit graph to propagate a reward through the graph. Good
//! scores propagate a positive reward to the dependencies of the exercise, that is to say down the
//! graph. Bad scores propagate a negative reward to the dependents of the exercise, that is to say
//! up the graph. During scheduling of new exercises, previous rewards are used to adjust the score
//! of the exercises.
//!
//! The main goal of the propagation process is twofold. First, it tries to avoid repetition of
//! exercises that have been implicitly mastered by doing harder exercises. Second, it tries to
//! increase the repetition of exercises for which the user has not yet mastered the material that
//! depends on them.

use std::collections::VecDeque;
use ustr::{Ustr, UstrMap};

use crate::{
    data::{MasteryScore, UnitReward},
    scheduler::data::SchedulerData,
};

/// The initial weight of the rewards.
const INITIAL_WEIGHT: f32 = 1.0;

/// The minimum weight of the rewards. Once the propagation reaches this weight, it stops.
const MIN_WEIGHT: f32 = 0.01;

/// The factor by which the weight decreases with each traversal of the graph.
const WEIGHT_FACTOR: f32 = 0.8;

/// Contains the logic to rewards through the graph when submitting a score.
pub(super) struct RewardPropagator {
    /// The external data used by the scheduler. Contains pointers to the graph, blacklist, and
    /// course library and provides convenient functions.
    pub data: SchedulerData,
}

impl RewardPropagator {
    /// Sets the initial reward for the given score.
    fn initial_reward(score: &MasteryScore) -> f32 {
        match score {
            MasteryScore::Five => 2.0,
            MasteryScore::Four => 1.0,
            MasteryScore::Three => -0.5,
            MasteryScore::Two => -1.0,
            MasteryScore::One => -2.0,
        }
    }

    fn get_next_units(&self, unit_id: Ustr, reward: f32) -> Vec<Ustr> {
        if reward > 0.0 {
            // Get the dependencies of the unit.
            self.data
                .unit_graph
                .read()
                .get_dependencies(unit_id)
                .unwrap_or_default()
                .into_iter()
                .collect()
        } else {
            // Get the dependents of the unit.
            self.data
                .unit_graph
                .read()
                .get_dependents(unit_id)
                .unwrap_or_default()
                .into_iter()
                .collect()
        }
    }

    /// Propagates the given score through the graph.
    pub(super) fn propagate_rewards(
        &self,
        exercise_id: Ustr,
        score: &MasteryScore,
        timestamp: i64,
    ) -> Vec<(Ustr, UnitReward)> {
        // Get the lesson and course for this exercise.
        let lesson_id = self.data.get_course_id(exercise_id).unwrap_or_default();
        let course_id = self.data.get_course_id(lesson_id).unwrap_or_default();
        if lesson_id.is_empty() || course_id.is_empty() {
            return vec![];
        }

        // Populate the queue using the course and lesson with the initial reward and weight.
        let initial_reward = Self::initial_reward(score);
        let next_lessons = self.get_next_units(lesson_id, initial_reward);
        let next_courses = self.get_next_units(course_id, initial_reward);
        let mut queue: VecDeque<(Ustr, UnitReward)> = VecDeque::new();
        next_lessons
            .iter()
            .chain(next_courses.iter())
            .for_each(|id| {
                queue.push_back((
                    *id,
                    UnitReward {
                        reward: initial_reward,
                        weight: INITIAL_WEIGHT,
                        timestamp,
                    },
                ));
            });

        // While the queue is not empty, pop the first element, push it into the results, and
        // continue the search with updated rewards and weights.
        let mut results = UstrMap::default();
        while let Some((unit_id, unit_reward)) = queue.pop_front() {
            // If the weight is less than the minimum, stop propagating.
            if unit_reward.weight < MIN_WEIGHT {
                continue;
            }

            // If the unit is already in the results, continue. Otherwise, add it to the results.
            if results.contains_key(&unit_id) {
                continue;
            }
            results.insert(
                unit_id,
                UnitReward {
                    reward: unit_reward.reward,
                    weight: unit_reward.weight,
                    timestamp,
                },
            );

            // Get the next units and push them into the queue with updated rewards and weights.
            self.get_next_units(unit_id, unit_reward.reward)
                .iter()
                .for_each(|next_unit_id| {
                    queue.push_back((
                        *next_unit_id,
                        UnitReward {
                            reward: unit_reward.reward,
                            weight: unit_reward.weight * WEIGHT_FACTOR,
                            timestamp,
                        },
                    ));
                });
        }
        results.into_iter().collect()
    }
}
