//! Contains the main logic for propagating rewards through the graph. When an exercise submits a
//! score, Trane uses the score and the unit graph to propagate a reward. Good scores propagate a
//! positive reward to the units encompassed by the exercise, that is to say down the graph. Bad
//! scores propagate a negative reward to the units that encompass the exercise, that is to say up
//! the graph. During scheduling of new exercises, previous rewards are used to adjust the score of
//! the exercises.
//!
//! The encompassing relationship describes when a unit's exercises implicitly practice the skills
//! of another unit. This differs from the dependency relationship, which only indicates that one
//! unit must be mastered before another can be attempted. While dependencies are prerequisites for
//! learning, encompassings represent partial or full implicit practice. For example, solving
//! advanced multiplication problems encompasses basic multiplication skills. The "fractional"
//! aspect allows specifying partial encompassings via weights in the range [0.0, 1.0], where 1.0
//! represents full encompassing and lower values indicate only some portion of the simpler skill is
//! practiced implicitly. This is crucial in hierarchical knowledge structures where advanced topics
//! only partially cover simpler component skills.
//!
//! The main goal of the propagation process is twofold. First, it tries to avoid showing exercises
//! that are fully or partially covered by performing other exercises. Second, it tries to increase
//! repetitions of exercises for which the student has not mastered the material encompassed by
//! them.
//!
//! To make it easy for course authors to specify the encompassing relationships, Trane assumes by
//! default that the encompassing relationship is the same as the dependency relationship, with a
//! weight of 1.0. This means that they only need to specify the encompassing relationships for the
//! units that are not part of the dependencies or to set a dependency with a weight of 0.0 to stop
//! propagation along that edge.
//!
//! This feature is heavily inspired by Fractional Implicit Repetition (FIRe), a method for
//! propagating rewards through hierarchical knowledge structures developed by Math Academy (see
//! <https://www.justinmath.com/individualized-spaced-repetition-in-hierarchical-knowledge-structures/>).

use ustr::{Ustr, UstrMap};

use crate::{
    data::{MasteryScore, UnitReward},
    graph::UnitGraph,
    scheduler::data::SchedulerData,
};

/// The minimum absolute value of the reward. Propagation stops when this value is reached.
pub(super) const MIN_ABS_REWARD: f32 = 0.2;

/// The minimum weight of the rewards. Once the propagation reaches this weight, it stops.
pub(super) const MIN_WEIGHT: f32 = 0.2;

/// The factor by which the weight of the reward decreases with each traversal of the graph. Used
/// to localize the reward to the units closes to the exercise.
pub(super) const WEIGHT_FACTOR: f32 = 0.8;

/// The factor by which the absolute value of the reward decreases with each traversal of the graph.
/// Used to localize the reward to the units closes to the exercise.
pub(super) const REWARD_FACTOR: f32 = 0.9;

/// Contains the logic to rewards through the graph when submitting a score.
pub(super) struct RewardPropagator {
    /// The external data used by the scheduler. Contains pointers to the graph, blacklist, and
    /// course library and provides convenient functions.
    pub data: SchedulerData,
}

/// An item in the reward propagation stack.
struct RewardStackItem {
    /// The unit ID.
    unit_id: Ustr,

    /// The reward associated with this unit.
    reward: UnitReward,
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
    fn get_next_units(unit_graph: &dyn UnitGraph, unit_id: Ustr, reward: f32) -> Vec<(Ustr, f32)> {
        if reward > 0.0 {
            unit_graph.get_encompasses(unit_id).unwrap_or_default()
        } else {
            unit_graph.get_encompassed_by(unit_id).unwrap_or_default()
        }
    }

    /// Returns whether propagation should stop.
    pub(super) fn stop_propagation(reward: f32, weight: f32) -> bool {
        reward.abs() < MIN_ABS_REWARD || weight < MIN_WEIGHT
    }

    /// Returns the lesson and course roots for the given exercise, if they exist.
    fn resolve_roots(unit_graph: &dyn UnitGraph, exercise_id: Ustr) -> Option<(Ustr, Ustr)> {
        let lesson_id = unit_graph.get_exercise_lesson(exercise_id)?;
        let course_id = unit_graph.get_lesson_course(lesson_id)?;
        Some((lesson_id, course_id))
    }

    /// Helper to propagate the rewards through the graph. It is written as a helper to make it
    /// easy to test the propagation logic in isolation.
    fn propagate_rewards_helper(
        unit_graph: &dyn UnitGraph,
        lesson_id: Ustr,
        course_id: Ustr,
        score: &MasteryScore,
        timestamp: i64,
    ) -> Vec<(Ustr, UnitReward)> {
        if lesson_id.is_empty() || course_id.is_empty() {
            return vec![]; // grcov-excl-line
        }

        // Populate the stack using the initial lessons and courses encompassed by this exercise.
        let initial_reward = Self::initial_reward(score);
        let next_lessons = Self::get_next_units(unit_graph, lesson_id, initial_reward);
        let next_courses = Self::get_next_units(unit_graph, course_id, initial_reward);
        let mut stack: Vec<RewardStackItem> = Vec::new();
        next_lessons
            .iter()
            .chain(next_courses.iter())
            .for_each(|(id, edge_weight)| {
                let value = edge_weight * initial_reward;
                let weight = *edge_weight;
                if Self::stop_propagation(value, weight) {
                    return;
                }
                stack.push(RewardStackItem {
                    unit_id: *id,
                    reward: UnitReward {
                        value,
                        weight,
                        timestamp,
                    },
                });
            });

        // While the stack is not empty, pop the last element, push it into the results, and
        // continue the search with updated rewards and weights.
        let mut results: UstrMap<UnitReward> = UstrMap::default();
        while let Some(item) = stack.pop() {
            // Skip paths that have become too weak, or that are weaker than an already known path.
            if Self::stop_propagation(item.reward.value, item.reward.weight) {
                continue;
            }
            if let Some(existing_reward) = results.get(&item.unit_id)
                && existing_reward.value.abs() >= item.reward.value.abs()
            {
                continue;
            }
            results.insert(item.unit_id, item.reward.clone());

            // Get the next units and push them onto the stack with updated rewards and weights.
            for (next_unit_id, edge_weight) in
                &Self::get_next_units(unit_graph, item.unit_id, item.reward.value)
            {
                let next_value = *edge_weight * REWARD_FACTOR * item.reward.value;
                let next_weight = *edge_weight * WEIGHT_FACTOR * item.reward.weight;
                if Self::stop_propagation(next_value, next_weight) {
                    continue;
                }
                stack.push(RewardStackItem {
                    unit_id: *next_unit_id,
                    reward: UnitReward {
                        value: next_value,
                        weight: next_weight,
                        timestamp,
                    },
                });
            }
        }
        results.into_iter().collect()
    }

    /// Propagates the given score through the graph.
    pub(super) fn propagate_rewards(
        &self,
        exercise_id: Ustr,
        score: &MasteryScore,
        timestamp: i64,
    ) -> Vec<(Ustr, UnitReward)> {
        let unit_graph = self.data.unit_graph.read();
        let roots = Self::resolve_roots(&*unit_graph, exercise_id);
        let Some((lesson_id, course_id)) = roots else {
            return vec![]; // grcov-excl-line
        };
        Self::propagate_rewards_helper(&*unit_graph, lesson_id, course_id, score, timestamp)
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use anyhow::Result;
    use ustr::{Ustr, UstrMap};

    use crate::{
        data::{MasteryScore, UnitReward},
        graph::{InMemoryUnitGraph, UnitGraph},
        scheduler::reward_propagator::{MIN_ABS_REWARD, MIN_WEIGHT, RewardPropagator},
    };

    fn build_path_graph(source_encompassed: &[(Ustr, f32)]) -> Result<InMemoryUnitGraph> {
        let mut graph = InMemoryUnitGraph::default();
        graph.add_course(Ustr::from("0"))?;
        graph.add_lesson(Ustr::from("0::0"), Ustr::from("0"))?;
        graph.add_lesson(Ustr::from("0::1"), Ustr::from("0"))?;
        graph.add_lesson(Ustr::from("0::2"), Ustr::from("0"))?;
        graph.add_lesson(Ustr::from("0::3"), Ustr::from("0"))?;
        graph.add_exercise(Ustr::from("0::0::0"), Ustr::from("0::0"))?;

        graph.add_encompassed(Ustr::from("0::0"), &[], source_encompassed)?;
        graph.add_encompassed(Ustr::from("0::1"), &[], &[(Ustr::from("0::3"), 1.0)])?;
        graph.add_encompassed(Ustr::from("0::2"), &[], &[(Ustr::from("0::3"), 1.0)])?;
        Ok(graph)
    }

    fn propagate_five_rewards(unit_graph: &dyn UnitGraph) -> Result<UstrMap<UnitReward>> {
        let rewards = RewardPropagator::propagate_rewards_helper(
            unit_graph,
            Ustr::from("0::0"),
            Ustr::from("0"),
            &MasteryScore::Five,
            0,
        );
        Ok(rewards.into_iter().collect())
    }

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
        assert!(!RewardPropagator::stop_propagation(
            MIN_ABS_REWARD,
            MIN_WEIGHT,
        ));
        assert!(RewardPropagator::stop_propagation(
            MIN_ABS_REWARD - 0.001,
            MIN_WEIGHT,
        ));
        assert!(RewardPropagator::stop_propagation(
            -MIN_ABS_REWARD + 0.001,
            MIN_WEIGHT,
        ));
        assert!(RewardPropagator::stop_propagation(
            MIN_ABS_REWARD,
            MIN_WEIGHT - 0.001,
        ));
    }

    /// Verifies that when multiple paths reach the same unit, the strongest path is used.
    #[test]
    fn strongest_path_wins() -> Result<()> {
        let graph = build_path_graph(&[(Ustr::from("0::1"), 1.0), (Ustr::from("0::2"), 0.5)])?;
        let reward_map = propagate_five_rewards(&graph)?;
        let reward = reward_map.get(&Ustr::from("0::3")).unwrap();
        assert!((reward.value - 0.72).abs() < f32::EPSILON);
        assert!((reward.weight - 0.8).abs() < f32::EPSILON);
        Ok(())
    }

    /// Verifies that the strongest-path result is independent of path insertion order.
    #[test]
    fn strongest_path_is_order_independent() -> Result<()> {
        let first_order =
            build_path_graph(&[(Ustr::from("0::1"), 1.0), (Ustr::from("0::2"), 0.5)])?;
        let second_order =
            build_path_graph(&[(Ustr::from("0::2"), 0.5), (Ustr::from("0::1"), 1.0)])?;
        let first_reward = propagate_five_rewards(&first_order)?
            .get(&Ustr::from("0::3"))
            .cloned()
            .unwrap();
        let second_reward = propagate_five_rewards(&second_order)?
            .get(&Ustr::from("0::3"))
            .cloned()
            .unwrap();
        assert!((first_reward.value - second_reward.value).abs() < f32::EPSILON);
        assert!((first_reward.weight - second_reward.weight).abs() < f32::EPSILON);
        Ok(())
    }

    /// Verifies that edge weights attenuate both reward value and reward weight.
    #[test]
    fn edge_weights_attenuate_reward_weight() -> Result<()> {
        let mut graph = InMemoryUnitGraph::default();
        graph.add_course(Ustr::from("0"))?;
        graph.add_lesson(Ustr::from("0::0"), Ustr::from("0"))?;
        graph.add_lesson(Ustr::from("0::1"), Ustr::from("0"))?;
        graph.add_lesson(Ustr::from("0::2"), Ustr::from("0"))?;
        graph.add_encompassed(Ustr::from("0::0"), &[], &[(Ustr::from("0::1"), 0.8)])?;
        graph.add_encompassed(Ustr::from("0::1"), &[], &[(Ustr::from("0::2"), 0.8)])?;

        let reward_map = propagate_five_rewards(&graph)?;
        let first_hop = reward_map.get(&Ustr::from("0::1")).unwrap();
        assert!((first_hop.value - 0.64).abs() < f32::EPSILON);
        assert!((first_hop.weight - 0.8).abs() < f32::EPSILON);

        let second_hop = reward_map.get(&Ustr::from("0::2")).unwrap();
        assert!((second_hop.value - 0.4608).abs() < f32::EPSILON);
        assert!((second_hop.weight - 0.512).abs() < f32::EPSILON);
        Ok(())
    }

    /// Verifies that very weak initial edges are pruned before being recorded.
    #[test]
    fn weak_initial_edges_are_pruned() -> Result<()> {
        let mut graph = InMemoryUnitGraph::default();
        graph.add_course(Ustr::from("0"))?;
        graph.add_lesson(Ustr::from("0::0"), Ustr::from("0"))?;
        graph.add_lesson(Ustr::from("0::1"), Ustr::from("0"))?;
        graph.add_encompassed(Ustr::from("0::0"), &[], &[(Ustr::from("0::1"), 0.1)])?;

        let reward_map = propagate_five_rewards(&graph)?;
        assert!(reward_map.is_empty());
        Ok(())
    }

    /// Verifies resolving roots from the graph directly.
    #[test]
    fn resolve_roots() -> Result<()> {
        let mut graph = InMemoryUnitGraph::default();
        graph.add_course(Ustr::from("course"))?;
        graph.add_lesson(Ustr::from("lesson"), Ustr::from("course"))?;
        graph.add_exercise(Ustr::from("exercise"), Ustr::from("lesson"))?;

        assert_eq!(
            RewardPropagator::resolve_roots(&graph, Ustr::from("exercise")),
            Some((Ustr::from("lesson"), Ustr::from("course")))
        );
        assert_eq!(
            RewardPropagator::resolve_roots(&graph, Ustr::from("missing")),
            None
        );
        Ok(())
    }
}
