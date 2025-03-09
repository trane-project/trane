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

/// The minimum absolute value of the reward. Propagation stops when this value is reached.
const MIN_ABS_REWARD: f32 = 0.1;

/// The initial weight of the rewards.
const INITIAL_WEIGHT: f32 = 1.0;

/// The minimum weight of the rewards. Once the propagation reaches this weight, it stops.
const MIN_WEIGHT: f32 = 0.2;

/// The factor by which the weight decreases with each traversal of the graph.
const WEIGHT_FACTOR: f32 = 0.7;

/// The factor by which the absolute value of the reward decreases with each traversal of the graph.
const DEPTH_FACTOR: f32 = 0.9;

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
            MasteryScore::Five => 1.0,
            MasteryScore::Four => 0.5,
            MasteryScore::Three => -0.5,
            MasteryScore::Two => -1.0,
            MasteryScore::One => -1.5,
        }
    }

    /// Gets the next units to visit, depending on the sign of the reward.
    fn get_next_units(&self, unit_id: Ustr, reward: f32) -> Vec<Ustr> {
        if reward > 0.0 {
            self.data
                .unit_graph
                .read()
                .get_dependencies(unit_id)
                .unwrap_or_default()
                .into_iter()
                .collect()
        } else {
            self.data
                .unit_graph
                .read()
                .get_dependents(unit_id)
                .unwrap_or_default()
                .into_iter()
                .collect()
        }
    }

    /// Returns whether propagation should stop along the path with the given reward and weight.
    fn stop_propagation(reward: f32, weigh: f32) -> bool {
        reward.abs() < MIN_ABS_REWARD || weigh < MIN_WEIGHT
    }

    /// Propagates the given score through the graph.
    pub(super) fn propagate_rewards(
        &self,
        exercise_id: Ustr,
        score: &MasteryScore,
        timestamp: i64,
    ) -> Vec<(Ustr, UnitReward)> {
        // Get the lesson and course for this exercise.
        let lesson_id = self.data.get_lesson_id(exercise_id).unwrap_or_default();
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
            // Check if propagation should continue and if the unit has already been visited. If
            // not, push the unit into the results and continue the search.
            if Self::stop_propagation(unit_reward.reward, unit_reward.weight) {
                continue;
            }
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
                            reward: unit_reward.reward * DEPTH_FACTOR,
                            weight: unit_reward.weight * WEIGHT_FACTOR,
                            timestamp,
                        },
                    ));
                });
        }
        results.into_iter().collect()
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use crate::{
        data::MasteryScore,
        scheduler::reward_propagator::{RewardPropagator, MIN_ABS_REWARD, MIN_WEIGHT},
    };

    /// Verifies the initial reward for each score.
    #[test]
    fn initial_reward() {
        assert_eq!(RewardPropagator::initial_reward(&MasteryScore::Five), 1.0);
        assert_eq!(RewardPropagator::initial_reward(&MasteryScore::Four), 0.5);
        assert_eq!(RewardPropagator::initial_reward(&MasteryScore::Three), -0.5);
        assert_eq!(RewardPropagator::initial_reward(&MasteryScore::Two), -1.0);
        assert_eq!(RewardPropagator::initial_reward(&MasteryScore::One), -1.5);
    }

    /// Verifies stopping the propagation if the reward or weight is too small.
    #[test]
    fn stop_propagation() {
        assert!(!RewardPropagator::stop_propagation(
            MIN_ABS_REWARD,
            MIN_WEIGHT
        ));
        assert!(RewardPropagator::stop_propagation(
            MIN_ABS_REWARD - 0.001,
            MIN_WEIGHT
        ));
        assert!(RewardPropagator::stop_propagation(
            -MIN_ABS_REWARD + 0.001,
            MIN_WEIGHT
        ));
        assert!(RewardPropagator::stop_propagation(
            MIN_ABS_REWARD,
            MIN_WEIGHT - 0.001
        ));
    }
}
