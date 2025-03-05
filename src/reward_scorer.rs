//! Rewards are propagated in the graph when a score is submitted to reflect performance of an
//! exercise on related ones. This module contains the logic that combines into a single value that
//! is then added to the score of an exercise that is computed from previous trials alone. The final
//! value can be positive or negative.

use anyhow::Result;

use crate::data::UnitReward;

/// A trait exposing a function to combine the rewards of a unit into a single value. The lesson
/// and course rewards are given separately to allow the implementation to treat them differently.
pub trait RewardScorer {
    /// Computes the final reward for a unit based on its previous course and lesson rewards.
    fn score_rewards(
        &self,
        previous_course_rewards: &[UnitReward],
        previous_lesson_rewards: &[UnitReward],
    ) -> Result<f32>;
}

/// The initial time weight for the most recent reward, which is the first one in the list.
const INITIAL_TIME_WEIGHT: f32 = 1.0;

/// The first reward has the initial time weight, and the weight decreases with each older reward by
/// this factor.
const TIME_WEIGHT_FACTOR: f32 = 0.8;

/// The minimum weight of a reward.
const MIN_WEIGHT: f32 = 0.01;

/// The weight of the course rewards in the final score.
const COURSE_REWARDS_WEIGHT: f32 = 0.3;

/// The weight of the lesson rewards in the final score. Lesson rewards are given more weight than
/// course rewards because lessons are more granular and related to the specific exercise.
const LESSON_REWARDS_WEIGHT: f32 = 0.7;

/// A simple implementation of the [`RewardScorer`] trait that computes a weighted average of the
/// rewards.
pub struct WeightedRewardScorer {}

impl WeightedRewardScorer {
    /// Returns the weights to used to compute the weighted average of the rewards. The final weight
    /// is computed as the product of the time and graph weight to compute a weight that reflects
    /// the recency of the reward as well as the graph distance from the origin of the reward.
    #[inline]
    fn score_weights(rewards: &[UnitReward]) -> Vec<f32> {
        rewards
            .iter()
            .enumerate()
            .map(|(i, reward)| {
                let weight =
                    reward.weight * INITIAL_TIME_WEIGHT * TIME_WEIGHT_FACTOR.powf(i as f32);
                weight.max(MIN_WEIGHT)
            })
            .collect()
    }

    /// Returns the weighted average of the scores.
    #[inline]
    fn weighted_average(rewards: &[f32], weights: &[f32]) -> f32 {
        // weighted average = (cross product of scores and their weights) / (sum of weights)
        let cross_product: f32 = rewards
            .iter()
            .zip(weights.iter())
            .map(|(s, w)| s * *w)
            .sum();
        let weight_sum = weights.iter().sum::<f32>();
        cross_product / weight_sum
    }
}

impl RewardScorer for WeightedRewardScorer {
    fn score_rewards(
        &self,
        previous_course_rewards: &[UnitReward],
        previous_lesson_rewards: &[UnitReward],
    ) -> Result<f32> {
        // Extract the reward values from the input structs.
        let lesson_rewards = previous_lesson_rewards
            .iter()
            .map(|r| r.reward)
            .collect::<Vec<_>>();
        let course_rewards = previous_course_rewards
            .iter()
            .map(|r| r.reward)
            .collect::<Vec<_>>();

        if previous_course_rewards.is_empty() && previous_lesson_rewards.is_empty() {
            // No rewards return a default value of 0.0.
            Ok(0.0)
        } else if previous_course_rewards.is_empty() {
            // If there are only lesson rewards, compute the weighted average of the lesson rewards.
            let lesson_weights = Self::score_weights(previous_lesson_rewards);
            Ok(Self::weighted_average(&lesson_rewards, &lesson_weights))
        } else if previous_lesson_rewards.is_empty() {
            // If there are only course rewards, compute the weighted average of the course rewards.
            let course_weights = Self::score_weights(previous_course_rewards);
            Ok(Self::weighted_average(&course_rewards, &course_weights))
        } else {
            // If there are both course and lesson rewards, compute the lesson and course scores
            // separately and then combine them into a single score using another weighted average.
            let course_weights = Self::score_weights(previous_course_rewards);
            let course_score = Self::weighted_average(&course_rewards, &course_weights);
            let lesson_weights = Self::score_weights(previous_lesson_rewards);
            let lesson_score = Self::weighted_average(&lesson_rewards, &lesson_weights);
            Ok(Self::weighted_average(
                &[course_score, lesson_score],
                &[COURSE_REWARDS_WEIGHT, LESSON_REWARDS_WEIGHT],
            ))
        }
    }
}

/// An implementation of [Send] for [`WeightedRewardScorer`]. This implementation is safe because
/// [`WeightedRewardScorer`] stores no state.
unsafe impl Send for WeightedRewardScorer {}

/// An implementation of [Sync] for [`WeightedRewardScorer`]. This implementation is safe because
/// [`WeightedRewardScorer`] stores no state.
unsafe impl Sync for WeightedRewardScorer {}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use crate::{
        data::UnitReward,
        reward_scorer::{RewardScorer, WeightedRewardScorer},
    };

    /// Verfies calculating the reward when no rewards are present.
    #[test]
    fn test_no_rewards() {
        let scorer = WeightedRewardScorer {};
        let result = scorer.score_rewards(&[], &[]).unwrap();
        assert_eq!(result, 0.0);
    }

    /// Verifies calculating the reward when only lesson rewards are present.
    #[test]
    fn test_only_lesson_rewards() {
        let scorer = WeightedRewardScorer {};
        let lesson_rewards = vec![
            UnitReward {
                reward: 1.0,
                weight: 1.0,
                timestamp: 3,
            },
            UnitReward {
                reward: 2.0,
                weight: 1.0,
                timestamp: 2,
            },
        ];
        let result = scorer.score_rewards(&[], &lesson_rewards).unwrap();
        assert!((result - 1.444).abs() < 0.001);
    }

    /// Verifies calculating the reward when only course rewards are present.
    #[test]
    fn test_only_course_rewards() {
        let scorer = WeightedRewardScorer {};
        let course_rewards = vec![
            UnitReward {
                reward: 1.0,
                weight: 1.0,
                timestamp: 3,
            },
            UnitReward {
                reward: 2.0,
                weight: 1.0,
                timestamp: 2,
            },
        ];
        let result = scorer.score_rewards(&course_rewards, &[]).unwrap();
        assert!((result - 1.444).abs() < 0.001);
    }

    /// Verifies calculating the reward when both course and lesson rewards are present.
    #[test]
    fn test_both_rewards() {
        let scorer = WeightedRewardScorer {};
        let course_rewards = vec![
            UnitReward {
                reward: 1.0,
                weight: 1.0,
                timestamp: 3,
            },
            UnitReward {
                reward: 2.0,
                weight: 1.0,
                timestamp: 2,
            },
        ];
        let lesson_rewards = vec![
            UnitReward {
                reward: 3.0,
                weight: 1.0,
                timestamp: 3,
            },
            UnitReward {
                reward: 4.0,
                weight: 1.0,
                timestamp: 2,
            },
        ];
        let result = scorer
            .score_rewards(&course_rewards, &lesson_rewards)
            .unwrap();
        assert!((result - 2.8444).abs() < 0.001);
    }

    /// Verifies calculating the reward when the weight is below the minimum weight.
    #[test]
    fn test_min_weight() {
        let scorer = WeightedRewardScorer {};
        let lesson_rewards = vec![
            UnitReward {
                reward: 2.0,
                weight: 1.0,
                timestamp: 3,
            },
            UnitReward {
                reward: 1.0,
                weight: 0.0001,
                timestamp: 2,
            },
        ];
        let result = scorer.score_rewards(&[], &lesson_rewards).unwrap();
        assert!((result - 1.99).abs() < 0.001);
    }
}
