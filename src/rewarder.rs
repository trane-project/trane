//! Contains the logic to compute the final reward of a unit based on its stored rewards.

use anyhow::Result;

use crate::data::UnitReward;

/// A trait exposing a function to combine the rewards of a unit into a single reward. The lesson
/// and course rewards are given separately to allow the implementation to treat them differently.
pub trait UnitRewarder {
    /// Computes the final reward for a unit based on its previous course and lesson rewards.
    fn reward(
        &self,
        previous_course_rewards: &[UnitReward],
        previous_lesson_rewards: &[UnitReward],
    ) -> Result<f32>;
}

/// A simple implementation of the [`UnitRewarder`] trait that computes a weighted average of the
/// rewards.
pub struct WeightedRewarder {}

impl UnitRewarder for WeightedRewarder {
    fn reward(
        &self,
        _previous_course_rewards: &[UnitReward],
        _previous_lesson_rewards: &[UnitReward],
    ) -> Result<f32> {
        // TODO: implement method. For now, return 0.0 to avoid adjusting any scores.
        Ok(0.0)
    }
}

/// An implementation of [Send] for [`WeightedRewarder`]. This implementation is safe because
/// [`WeightedRewarder`] stores no state.
unsafe impl Send for WeightedRewarder {}

/// An implementation of [Sync] for [`WeightedRewarder`]. This implementation is safe because
/// [`WeightedRewarder`] stores no state.
unsafe impl Sync for WeightedRewarder {}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {}
