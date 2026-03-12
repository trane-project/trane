use std::sync::Arc;

use parking_lot::RwLock;
use rand::seq::IteratorRandom;
use ustr::{Ustr, UstrSet};

use crate::{
    data::{MasteryScore, SchedulerOptions},
    scheduler::Candidate,
};

/// Stores recently failed exercises so that they can be re-scheduled soon. Research has shown that
/// re-scheduling these exercises soon after failure leads to improved retention.
pub(crate) struct RelearnPile {
    /// The scheduler options.
    options: SchedulerOptions,

    /// The pile of exercises, stored inside an `Arc<RwLock<>>` to allow concurrent access and
    /// mutation.
    pile: Arc<RwLock<UstrSet>>,
}

impl RelearnPile {
    /// Creates a new relearn pile.
    pub fn new(options: SchedulerOptions) -> Self {
        RelearnPile {
            options,
            pile: Arc::new(RwLock::new(UstrSet::default())),
        }
    }

    /// Updates the relearning pile based on the score of an exercise.
    pub fn update(&self, exercise_id: Ustr, score: &MasteryScore) {
        let mut relearning_pile = self.pile.write();
        match score {
            MasteryScore::One | MasteryScore::Two => relearning_pile.insert(exercise_id),
            MasteryScore::Three | MasteryScore::Four | MasteryScore::Five => {
                relearning_pile.remove(&exercise_id)
            }
        };
    }

    /// Removes an exercise from the relearn pile.
    pub fn remove(&self, exercise_id: Ustr) {
        self.pile.write().remove(&exercise_id);
    }

    /// Adds exercises from the relearn pile to the final batch.
    pub fn select_exercises(&self) -> Vec<Candidate> {
        // Select a random subset of exercises from the relearn pile.
        let num_to_add = (self.options.batch_size as f32 * self.options.relearn_fraction) as usize;
        let pile = self.pile.read();
        let relearn_exercises: Vec<_> = pile.iter().choose_multiple(&mut rand::rng(), num_to_add);

        // Convert them to candidates and add them to the batch. Fill the other fields with default
        // values as they are only needed for the filtering and sorting steps.
        relearn_exercises
            .into_iter()
            .map(|exercise_id| Candidate {
                exercise_id: *exercise_id,
                depth: 0.0,
                lesson_id: Ustr::default(),
                course_id: Ustr::default(),
                exercise_score: 0.0,
                lesson_score: 0.0,
                course_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
                frequency: 0,
                dead_end: false,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies updating the relearn pile adds and removes exercises correctly.
    #[test]
    fn test_update() {
        // Create a relearn pile and send some exercises with low scores to it.
        let relearn_pile = RelearnPile::new(SchedulerOptions::default());
        let exercise_id = Ustr::from("exercise_1");
        let exercise_id_2 = Ustr::from("exercise_2");
        relearn_pile.update(exercise_id, &MasteryScore::One);
        relearn_pile.update(exercise_id_2, &MasteryScore::Two);
        relearn_pile.update(exercise_id_2, &MasteryScore::One);
        assert!(relearn_pile.pile.read().contains(&exercise_id));
        assert!(relearn_pile.pile.read().contains(&exercise_id_2));

        // Send the exercises with high scores to the relearn pile and verify they are removed.
        relearn_pile.update(exercise_id, &MasteryScore::Four);
        relearn_pile.update(exercise_id_2, &MasteryScore::Five);
        assert!(!relearn_pile.pile.read().contains(&exercise_id));
        assert!(!relearn_pile.pile.read().contains(&exercise_id_2));
    }

    /// Verifies that removing an exercise from the relearn pile works correctly.
    #[test]
    fn test_remove() {
        let relearn_pile = RelearnPile::new(SchedulerOptions::default());
        let exercise_id = Ustr::from("exercise_1");
        relearn_pile.update(exercise_id, &MasteryScore::One);
        assert!(relearn_pile.pile.read().contains(&exercise_id));
        relearn_pile.remove(exercise_id);
        assert!(!relearn_pile.pile.read().contains(&exercise_id));
    }

    /// Verifies exercises from the relearn pile are added to the batch.
    #[test]
    fn test_add_to_batch() {
        // Create a relearn pile and add 20 exercises to it.
        let relearn_pile = RelearnPile::new(SchedulerOptions {
            batch_size: 10,
            relearn_fraction: 0.5,
            ..SchedulerOptions::default()
        });
        for i in 0..20 {
            let exercise_id = Ustr::from(&format!("exercise_{}", i));
            relearn_pile.update(exercise_id, &MasteryScore::One);
        }
        let pile = relearn_pile.select_exercises();
        assert_eq!(pile.len(), 5);
    }
}
