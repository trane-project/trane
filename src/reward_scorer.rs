//! Rewards are propagated in the graph when a score is submitted to reflect performance of an
//! exercise on related ones. This module contains the logic that combines into a single value that
//! is then added to the score of an exercise that is computed from previous trials alone. The final
//! value can be positive or negative.

use anyhow::Result;
use chrono::{TimeZone, Utc};

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

/// The absolute value of the reward decreases by this amount each day to avoid old rewards from
/// affecting the score indefinitely.
const DAY_ADJUSTMENT: f32 = 0.025;

/// The weight of the course rewards in the final score.
const COURSE_REWARDS_WEIGHT: f32 = 0.3;

/// The weight of the lesson rewards in the final score. Lesson rewards are given more weight than
/// course rewards because lessons are more granular and related to the specific exercise.
const LESSON_REWARDS_WEIGHT: f32 = 0.7;

/// A simple implementation of the [`RewardScorer`] trait that computes a weighted average of the
/// rewards.
pub struct WeightedRewardScorer {}

impl WeightedRewardScorer {
    /// Returns the number of days since the reward.
    #[inline]
    fn days_since(rewards: &[UnitReward]) -> Vec<f32> {
        rewards
            .iter()
            .map(|reward| {
                let now = Utc::now();
                let timestamp = Utc
                    .timestamp_opt(reward.timestamp, 0)
                    .earliest()
                    .unwrap_or_default();
                (now - timestamp).num_days() as f32
            })
            .collect()
    }

    /// Returns the weights of the rewards.
    #[inline]
    fn reward_weights(rewards: &[UnitReward]) -> Vec<f32> {
        rewards.iter().map(|reward| reward.weight).collect()
    }

    /// Returns the adjusted rewards based on the number of days since the reward.
    #[inline]
    fn adjusted_rewards(rewards: &[UnitReward]) -> Vec<f32> {
        let days = Self::days_since(rewards);
        rewards
            .iter()
            .zip(days.iter())
            .map(|(reward, day)| {
                if reward.reward >= 0.0 {
                    (reward.reward - (day * DAY_ADJUSTMENT)).max(0.0)
                } else {
                    (reward.reward + (day * DAY_ADJUSTMENT)).min(0.0)
                }
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
        if weight_sum == 0.0 {
            0.0
        } else {
            cross_product / weight_sum
        }
    }
}

impl RewardScorer for WeightedRewardScorer {
    fn score_rewards(
        &self,
        previous_course_rewards: &[UnitReward],
        previous_lesson_rewards: &[UnitReward],
    ) -> Result<f32> {
        // Compute the lesson and course scores separately.
        let course_score = Self::weighted_average(
            &Self::adjusted_rewards(previous_course_rewards),
            &Self::reward_weights(previous_course_rewards),
        );
        let lesson_score = Self::weighted_average(
            &Self::adjusted_rewards(previous_lesson_rewards),
            &Self::reward_weights(previous_lesson_rewards),
        );

        // Calculate the final value, depending on which rewards are present.
        if previous_course_rewards.is_empty() && previous_lesson_rewards.is_empty() {
            Ok(0.0)
        } else if previous_course_rewards.is_empty() {
            Ok(lesson_score)
        } else if previous_lesson_rewards.is_empty() {
            Ok(course_score)
        } else {
            // If there are both course and lesson rewards, compute the lesson and course scores
            // separately and then combine them into a single score using another weighted average.
            Ok(Self::weighted_average(
                &[course_score, lesson_score],
                &[COURSE_REWARDS_WEIGHT, LESSON_REWARDS_WEIGHT],
            ))
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use chrono::Utc;

    use crate::{
        data::UnitReward,
        reward_scorer::{RewardScorer, WeightedRewardScorer},
    };

    const SECONDS_IN_DAY: i64 = 60 * 60 * 24;

    /// Generates a timestamp equal to the timestamp from `num_days` ago.
    fn generate_timestamp(num_days: i64) -> i64 {
        let now = Utc::now().timestamp();
        now - num_days * SECONDS_IN_DAY
    }

    /// Verifies adjusting the reward value based on the number of days since the reward.
    #[test]
    fn test_adjusted_rewards() {
        // Recent rewards still have some value.
        let rewards = vec![
            UnitReward {
                reward: 1.0,
                weight: 1.0,
                timestamp: generate_timestamp(1),
            },
            UnitReward {
                reward: -1.0,
                weight: 1.0,
                timestamp: generate_timestamp(1),
            },
        ];
        let adjusted_rewards = WeightedRewardScorer::adjusted_rewards(&rewards);
        assert_eq!(adjusted_rewards, vec![0.975, -0.975]);

        // The absolute value of older rewards trends to zero.
        let rewards = vec![
            UnitReward {
                reward: 1.0,
                weight: 1.0,
                timestamp: 1,
            },
            UnitReward {
                reward: -1.0,
                weight: 1.0,
                timestamp: 1,
            },
        ];
        let adjusted_rewards = WeightedRewardScorer::adjusted_rewards(&rewards);
        assert_eq!(adjusted_rewards, vec![0.0, 0.0]);
    }

    /// Verifies calculating the reward when no rewards are present.
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
                timestamp: generate_timestamp(1),
            },
            UnitReward {
                reward: 2.0,
                weight: 1.0,
                timestamp: generate_timestamp(2),
            },
        ];
        let result = scorer.score_rewards(&[], &lesson_rewards).unwrap();
        assert!((result - 1.462).abs() < 0.001);
    }

    /// Verifies calculating the reward when only course rewards are present.
    #[test]
    fn test_only_course_rewards() {
        let scorer = WeightedRewardScorer {};
        let course_rewards = vec![
            UnitReward {
                reward: 1.0,
                weight: 1.0,
                timestamp: generate_timestamp(1),
            },
            UnitReward {
                reward: 2.0,
                weight: 1.0,
                timestamp: generate_timestamp(2),
            },
        ];
        let result = scorer.score_rewards(&course_rewards, &[]).unwrap();
        assert!((result - 1.462).abs() < 0.001);
    }

    /// Verifies calculating the reward when both course and lesson rewards are present.
    #[test]
    fn test_both_rewards() {
        let scorer = WeightedRewardScorer {};
        let course_rewards = vec![
            UnitReward {
                reward: 1.0,
                weight: 1.0,
                timestamp: generate_timestamp(1),
            },
            UnitReward {
                reward: 2.0,
                weight: 1.0,
                timestamp: generate_timestamp(2),
            },
        ];
        let lesson_rewards = vec![
            UnitReward {
                reward: 2.0,
                weight: 1.0,
                timestamp: generate_timestamp(1),
            },
            UnitReward {
                reward: 4.0,
                weight: 2.0,
                timestamp: generate_timestamp(2),
            },
        ];
        let result = scorer
            .score_rewards(&course_rewards, &lesson_rewards)
            .unwrap();
        assert!((result - 2.742).abs() < 0.001);
    }

    /// Verifies calculating the reward when the weight is below the minimum weight.
    #[test]
    fn test_min_weight() {
        let scorer = WeightedRewardScorer {};
        let lesson_rewards = vec![
            UnitReward {
                reward: 2.0,
                weight: 1.0,
                timestamp: generate_timestamp(0),
            },
            UnitReward {
                reward: 1.0,
                weight: 0.0001,
                timestamp: generate_timestamp(0) - 1,
            },
        ];
        let result = scorer.score_rewards(&[], &lesson_rewards).unwrap();
        assert!((result - 1.999).abs() < 0.001);
    }
}
