//! Rewards are propagated in the graph when a score is submitted to reflect performance of an
//! exercise on related ones. This module contains the logic that combines into a single value that
//! is then added to the score of an exercise that is computed from previous trials alone. The final
//! value can be positive or negative.

use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};

use crate::data::{ExerciseTrial, UnitReward};

/// A trait exposing a function to combine the rewards of a unit into a single value. The lesson
/// and course rewards are given separately to allow the implementation to treat them differently.
pub trait RewardScorer {
    /// Computes the final reward for a unit based on its previous course and lesson rewards.
    fn score_rewards(
        &self,
        previous_course_rewards: &[UnitReward],
        previous_lesson_rewards: &[UnitReward],
    ) -> Result<f32>;

    /// Determines whether the reward should be applied to an exercise with the given trials. The
    /// trials are assumed to be ordered in descending order by timestamp.
    fn apply_reward(&self, reward: f32, previous_trials: &[ExerciseTrial]) -> bool;
}

/// The reward half-life, in days, used to decay both reward values and reward weights.
const REWARD_HALF_LIFE_DAYS: f32 = 14.0;

/// Rewards with effective weights below this threshold are ignored.
const MIN_EFFECTIVE_WEIGHT: f32 = 0.05;

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
    fn days_since(reward: &UnitReward, now: DateTime<Utc>) -> f32 {
        let timestamp = Utc
            .timestamp_opt(reward.timestamp, 0)
            .earliest()
            .unwrap_or_default();
        (now - timestamp).num_days().max(0) as f32
    }

    /// Returns the reward-decay factor for the given number of elapsed days.
    fn decay_factor(days: f32) -> f32 {
        0.5_f32.powf(days / REWARD_HALF_LIFE_DAYS)
    }

    /// Returns the reward value and weight after applying time decay.
    fn decayed_reward(reward: &UnitReward, now: DateTime<Utc>) -> (f32, f32) {
        let days = Self::days_since(reward, now);
        let decay = Self::decay_factor(days);
        (reward.value * decay, reward.weight * decay)
    }

    /// Returns the weighted average of the scores.
    fn weighted_average(rewards: &[UnitReward], now: DateTime<Utc>) -> f32 {
        let mut numerator = 0.0;
        let mut denominator = 0.0;

        // weighted average = (cross product of scores and their weights) / (sum of weights)
        for reward in rewards {
            let (effective_value, effective_weight) = Self::decayed_reward(reward, now);
            if effective_weight < MIN_EFFECTIVE_WEIGHT {
                continue;
            }
            numerator += effective_value * effective_weight;
            denominator += effective_weight;
        }

        if denominator == 0.0 {
            0.0
        } else {
            numerator / denominator
        }
    }
}

impl RewardScorer for WeightedRewardScorer {
    fn score_rewards(
        &self,
        previous_course_rewards: &[UnitReward],
        previous_lesson_rewards: &[UnitReward],
    ) -> Result<f32> {
        let now = Utc::now();

        // Compute the lesson and course scores separately.
        let course_score = Self::weighted_average(previous_course_rewards, now);
        let lesson_score = Self::weighted_average(previous_lesson_rewards, now);

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
            let numerator =
                course_score * COURSE_REWARDS_WEIGHT + lesson_score * LESSON_REWARDS_WEIGHT;
            let denominator = COURSE_REWARDS_WEIGHT + LESSON_REWARDS_WEIGHT;
            Ok(numerator / denominator)
        }
    }

    fn apply_reward(&self, reward: f32, previous_trials: &[ExerciseTrial]) -> bool {
        // Do not apply rewards to exercises with very few trials
        if previous_trials.len() <= 2 {
            return false;
        }

        // Do not apply positive rewards to exercises where the average of the last 3 trials is less
        // than 3 and the last trials was less than a week ago. This is to avoid rewarding exercises
        // that are not being performed well.
        let recent_trials = previous_trials.iter().take(3);
        let last_trial = previous_trials.first().unwrap();
        let num_days = (Utc::now().timestamp() - last_trial.timestamp) as f32 / (86_400.0);
        let average_score = recent_trials.map(|trial| trial.score).sum::<f32>() / 3.0;
        if reward > 0.0 && average_score < 3.0 && num_days < 7.0 {
            return false;
        }

        // Do not apply negative rewards to exercises where the average of the last 3 trials is
        // greater than 3.5 and the last trial was less than a week ago. This is to avoid penalizing
        // exercises that are being performed well.
        if reward < 0.0 && average_score > 3.5 && num_days < 7.0 {
            return false;
        }

        // Apply rewards in all other cases.
        true
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use chrono::Utc;

    use crate::{
        data::{ExerciseTrial, UnitReward},
        reward_scorer::{RewardScorer, WeightedRewardScorer},
    };

    const SECONDS_IN_DAY: i64 = 60 * 60 * 24;

    /// Generates a timestamp equal to the timestamp from `num_days` ago.
    fn generate_timestamp(num_days: i64) -> i64 {
        let now = Utc::now().timestamp();
        now - num_days * SECONDS_IN_DAY
    }

    /// Generates a timestamp equal to the timestamp from `num_days` in the future.
    fn generate_future_timestamp(num_days: i64) -> i64 {
        let now = Utc::now().timestamp();
        now + num_days * SECONDS_IN_DAY
    }

    /// Verifies computing the decay factor from elapsed days.
    #[test]
    fn test_decay_factor() {
        assert!((WeightedRewardScorer::decay_factor(0.0) - 1.0).abs() < 0.000_001);
        assert!((WeightedRewardScorer::decay_factor(14.0) - 0.5).abs() < 0.001);
        assert!((WeightedRewardScorer::decay_factor(28.0) - 0.25).abs() < 0.001);
    }

    /// Verifies decaying reward values and weights with elapsed time.
    #[test]
    fn test_decayed_reward() {
        let now = Utc::now();

        let reward = UnitReward {
            value: 1.0,
            weight: 2.0,
            timestamp: generate_timestamp(14),
        };
        let (value, weight) = WeightedRewardScorer::decayed_reward(&reward, now);
        assert!((value - 0.5).abs() < 0.001);
        assert!((weight - 1.0).abs() < 0.001);

        let reward = UnitReward {
            value: -1.0,
            weight: 1.0,
            timestamp: generate_timestamp(14),
        };
        let (value, weight) = WeightedRewardScorer::decayed_reward(&reward, now);
        assert!((value + 0.5).abs() < 0.001);
        assert!((weight - 0.5).abs() < 0.001);
    }

    /// Verifies clamping elapsed days to avoid amplifying rewards with future timestamps.
    #[test]
    fn test_future_timestamp_is_clamped() {
        let now = Utc::now();
        let reward = UnitReward {
            value: 1.0,
            weight: 2.0,
            timestamp: generate_future_timestamp(3),
        };
        let (value, weight) = WeightedRewardScorer::decayed_reward(&reward, now);
        assert!((value - 1.0).abs() < 0.001);
        assert!((weight - 2.0).abs() < 0.001);
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
                value: 1.0,
                weight: 1.0,
                timestamp: generate_timestamp(1),
            },
            UnitReward {
                value: 2.0,
                weight: 1.0,
                timestamp: generate_timestamp(2),
            },
        ];
        let result = scorer.score_rewards(&[], &lesson_rewards).unwrap();
        assert!((result - 1.371).abs() < 0.001);
    }

    /// Verifies calculating the reward when only course rewards are present.
    #[test]
    fn test_only_course_rewards() {
        let scorer = WeightedRewardScorer {};
        let course_rewards = vec![
            UnitReward {
                value: 1.0,
                weight: 1.0,
                timestamp: generate_timestamp(1),
            },
            UnitReward {
                value: 2.0,
                weight: 1.0,
                timestamp: generate_timestamp(2),
            },
        ];
        let result = scorer.score_rewards(&course_rewards, &[]).unwrap();
        assert!((result - 1.371).abs() < 0.001);
    }

    /// Verifies calculating the reward when both course and lesson rewards are present.
    #[test]
    fn test_both_rewards() {
        let scorer = WeightedRewardScorer {};
        let course_rewards = vec![
            UnitReward {
                value: 1.0,
                weight: 1.0,
                timestamp: generate_timestamp(1),
            },
            UnitReward {
                value: 2.0,
                weight: 1.0,
                timestamp: generate_timestamp(2),
            },
        ];
        let lesson_rewards = vec![
            UnitReward {
                value: 2.0,
                weight: 1.0,
                timestamp: generate_timestamp(1),
            },
            UnitReward {
                value: 4.0,
                weight: 2.0,
                timestamp: generate_timestamp(2),
            },
        ];
        let result = scorer
            .score_rewards(&course_rewards, &lesson_rewards)
            .unwrap();
        assert!((result - 2.533).abs() < 0.001);
    }

    /// Verifies calculating the reward when the weight is below the minimum weight.
    #[test]
    fn test_min_weight() {
        let scorer = WeightedRewardScorer {};
        let lesson_rewards = vec![
            UnitReward {
                value: 2.0,
                weight: 1.0,
                timestamp: generate_timestamp(0),
            },
            UnitReward {
                value: 1.0,
                weight: 0.0001,
                timestamp: generate_timestamp(0) - 1,
            },
        ];
        let result = scorer.score_rewards(&[], &lesson_rewards).unwrap();
        assert!((result - 2.0).abs() < 0.001);
    }

    /// Verifies stale high-weight rewards do not overly dilute fresh reward signal.
    #[test]
    fn test_stale_rewards_do_not_drag_denominator() {
        let scorer = WeightedRewardScorer {};
        let lesson_rewards = vec![
            UnitReward {
                value: 1.0,
                weight: 10.0,
                timestamp: generate_timestamp(70),
            },
            UnitReward {
                value: 1.0,
                weight: 1.0,
                timestamp: generate_timestamp(0),
            },
        ];
        let result = scorer.score_rewards(&[], &lesson_rewards).unwrap();
        assert!(result > 0.7);
    }

    /// Verifies that the rewards are applied only when the correct criteria are met.
    #[test]
    fn test_apply_rewards() {
        let scorer = WeightedRewardScorer {};

        // Do not apply rewards to exercises with very few trials.
        let trials = vec![ExerciseTrial {
            score: 2.0,
            timestamp: generate_timestamp(1),
        }];
        assert!(!scorer.apply_reward(0.5, &trials));
        assert!(!scorer.apply_reward(-1.0, &trials));

        // Do not apply positive rewards to exercises where the average of the last 3 trials is less
        // than 3 and the last trial was less than a week ago.
        let trials = vec![
            ExerciseTrial {
                score: 2.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 2.0,
                timestamp: generate_timestamp(8),
            },
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(10),
            },
        ];
        assert!(!scorer.apply_reward(0.5, &trials));
        assert!(scorer.apply_reward(-1.0, &trials));

        // Do not apply negative rewards to exercises where the average of the last 3 trials is
        // greater than 3.5 and the last trial was less than a week ago.
        let trials = vec![
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 5.0,
                timestamp: generate_timestamp(8),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(10),
            },
        ];
        assert!(!scorer.apply_reward(-0.5, &trials));
        assert!(scorer.apply_reward(1.0, &trials));

        // Apply rewards in other cases.
        let trials = vec![
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(8),
            },
            ExerciseTrial {
                score: 4.0,
                timestamp: generate_timestamp(10),
            },
        ];
        assert!(scorer.apply_reward(0.5, &trials));
        let trials = vec![
            ExerciseTrial {
                score: 2.0,
                timestamp: generate_timestamp(1),
            },
            ExerciseTrial {
                score: 3.0,
                timestamp: generate_timestamp(8),
            },
            ExerciseTrial {
                score: 2.0,
                timestamp: generate_timestamp(10),
            },
        ];
        assert!(scorer.apply_reward(-0.5, &trials));
    }
}
