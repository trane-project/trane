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
    data::{ExerciseType, MasteryScore, UnitReward, UnitType},
    scheduler::data::SchedulerData,
};

/// The minimum absolute value of the reward. Propagation stops when this value is reached.
const MIN_ABS_REWARD: f32 = 0.2;

/// The minimum weight of the rewards. Once the propagation reaches this weight, it stops.
const MIN_WEIGHT: f32 = 0.2;

/// The factor by which the weight of the reward decreases with each traversal of the graph. Used
/// to localize the reward to the units closes to the exercise.
const WEIGHT_FACTOR: f32 = 0.8;

/// The factor by which the absolute value of the reward decreases with each traversal of the graph.
/// Used to localize the reward to the units closes to the exercise.
const REWARD_FACTOR: f32 = 0.9;

/// Contains the logic to rewards through the graph when submitting a score.
pub(super) struct RewardPropagator {
    /// The external data used by the scheduler. Contains pointers to the graph, blacklist, and
    /// course library and provides convenient functions.
    pub data: SchedulerData,
}

/// An item in the reward propagation queue.
struct RewardQueueItem {
    /// The unit ID.
    unit_id: Ustr,

    /// The reward associated with this unit.
    reward: UnitReward,

    /// The default exercise type for this unit.
    default_exercise_type: Option<ExerciseType>,
}

impl RewardPropagator {
    /// Sets the initial reward for the given score.
    fn initial_reward(score: &MasteryScore) -> f32 {
        match score {
            MasteryScore::Five => 0.8,
            MasteryScore::Four => 0.4,
            MasteryScore::Three => -0.3,
            MasteryScore::Two => -0.5,
            MasteryScore::One => -1.0,
        }
    }

    /// Gets the next units to visit, depending on the sign of the reward.
    fn get_next_units(&self, unit_id: Ustr, reward: f32) -> Vec<(Ustr, f32)> {
        if reward > 0.0 {
            self.data
                .unit_graph
                .read()
                .get_encompassed(unit_id)
                .unwrap_or_default()
                .into_iter()
                .collect()
        } else {
            self.data
                .unit_graph
                .read()
                .get_encompassed_by(unit_id)
                .unwrap_or_default()
                .into_iter()
                .collect()
        }
    }

    /// Returns whether propagation should stop.
    fn stop_propagation(item: &RewardQueueItem) -> bool {
        // Propagation stops when the default exercise type is Declarative (those centered around
        // memorization), because memorizing the material of one unit does not imply memorizing or
        // mastering the material of neighboring units.
        if let Some(ExerciseType::Declarative) = item.default_exercise_type {
            return true;
        }

        // Otherwise, propagation stops when the reward or weight is too small.
        item.reward.value.abs() < MIN_ABS_REWARD || item.reward.weight < MIN_WEIGHT
    }

    /// Returns the default exercise type for the given lesson and course.
    fn get_default_exercise_type(&self, unit_id: Ustr) -> Option<ExerciseType> {
        let unit_type = self.data.unit_graph.read().get_unit_type(unit_id);
        match unit_type {
            Some(UnitType::Course) => self
                .data
                .course_library
                .read()
                .get_course_manifest(unit_id)
                .and_then(|manifest| manifest.default_exercise_type),
            Some(UnitType::Lesson) => self
                .data
                .course_library
                .read()
                .get_lesson_manifest(unit_id)
                .and_then(|manifest| manifest.default_exercise_type),
            _ => None,
        }
    }

    /// Propagates the given score through the graph.
    pub(super) fn propagate_rewards(
        &self,
        exercise_id: Ustr,
        score: &MasteryScore,
        timestamp: i64,
    ) -> Vec<(Ustr, UnitReward)> {
        // Get the lesson and course for this exercise and the default exercise type.
        let lesson_id = self.data.get_lesson_id(exercise_id).unwrap_or_default();
        let course_id = self.data.get_course_id(lesson_id).unwrap_or_default();
        if lesson_id.is_empty() || course_id.is_empty() {
            return vec![];
        }

        // Populate the queue using the course and lesson with the initial reward and weight.
        let initial_reward = Self::initial_reward(score);
        let next_lessons = self.get_next_units(lesson_id, initial_reward);
        let next_courses = self.get_next_units(course_id, initial_reward);
        let mut queue: VecDeque<RewardQueueItem> = VecDeque::new();
        next_lessons
            .iter()
            .chain(next_courses.iter())
            .for_each(|(id, _)| {
                queue.push_back(RewardQueueItem {
                    unit_id: *id,
                    reward: UnitReward {
                        value: initial_reward,
                        weight: 1.0,
                        timestamp,
                    },
                    default_exercise_type: self.get_default_exercise_type(*id),
                });
            });

        // While the queue is not empty, pop the first element, push it into the results, and
        // continue the search with updated rewards and weights.
        let mut results = UstrMap::default();
        while let Some(item) = queue.pop_front() {
            // Check if propagation should continue and if the unit has already been visited. If
            // not, push the unit into the results and continue the search.
            if Self::stop_propagation(&item) {
                continue;
            }
            if results.contains_key(&item.unit_id) {
                continue;
            }
            results.insert(
                item.unit_id,
                UnitReward {
                    value: item.reward.value,
                    weight: item.reward.weight,
                    timestamp,
                },
            );

            // Get the next units and push them into the queue with updated rewards and weights.
            self.get_next_units(item.unit_id, item.reward.value)
                .iter()
                .for_each(|(next_unit_id, edge_weight)| {
                    queue.push_back(RewardQueueItem {
                        unit_id: *next_unit_id,
                        reward: UnitReward {
                            value: *edge_weight * REWARD_FACTOR * item.reward.value,
                            weight: WEIGHT_FACTOR * item.reward.weight,
                            timestamp,
                        },
                        default_exercise_type: self.get_default_exercise_type(*next_unit_id),
                    });
                });
        }
        results.into_iter().collect()
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use crate::{
        data::{MasteryScore, UnitReward},
        scheduler::reward_propagator::{
            MIN_ABS_REWARD, MIN_WEIGHT, RewardPropagator, RewardQueueItem,
        },
    };

    /// Verifies the initial reward for each score.
    #[test]
    fn initial_reward() {
        assert_eq!(RewardPropagator::initial_reward(&MasteryScore::Five), 0.8);
        assert_eq!(RewardPropagator::initial_reward(&MasteryScore::Four), 0.4);
        assert_eq!(RewardPropagator::initial_reward(&MasteryScore::Three), -0.3);
        assert_eq!(RewardPropagator::initial_reward(&MasteryScore::Two), -0.5);
        assert_eq!(RewardPropagator::initial_reward(&MasteryScore::One), -1.0);
    }

    /// Verifies stopping the propagation when the conditions are met.
    #[test]
    fn stop_propagation() {
        assert!(RewardPropagator::stop_propagation(&RewardQueueItem {
            unit_id: ustr::ustr("unit0"),
            reward: UnitReward {
                value: 1.0,
                weight: 1.0,
                timestamp: 0,
            },
            default_exercise_type: Some(crate::data::ExerciseType::Declarative),
        }));
        assert!(!RewardPropagator::stop_propagation(&RewardQueueItem {
            unit_id: ustr::ustr("unit1"),
            reward: UnitReward {
                value: MIN_ABS_REWARD,
                weight: MIN_WEIGHT,
                timestamp: 0,
            },
            default_exercise_type: None,
        }));
        assert!(RewardPropagator::stop_propagation(&RewardQueueItem {
            unit_id: ustr::ustr("unit2"),
            reward: UnitReward {
                value: MIN_ABS_REWARD - 0.001,
                weight: MIN_WEIGHT,
                timestamp: 0,
            },
            default_exercise_type: None,
        }));
        assert!(RewardPropagator::stop_propagation(&RewardQueueItem {
            unit_id: ustr::ustr("unit3"),
            reward: UnitReward {
                value: -MIN_ABS_REWARD + 0.001,
                weight: MIN_WEIGHT,
                timestamp: 0,
            },
            default_exercise_type: None,
        }));
        assert!(RewardPropagator::stop_propagation(&RewardQueueItem {
            unit_id: ustr::ustr("unit4"),
            reward: UnitReward {
                value: MIN_ABS_REWARD,
                weight: MIN_WEIGHT - 0.001,
                timestamp: 0,
            },
            default_exercise_type: None,
        }));
    }
}
