use anyhow::Result;
use rand::{prelude::SliceRandom, thread_rng};
use std::collections::HashSet;

use crate::{
    data::{ExerciseManifest, MasteryWindowOpts},
    scheduler::{Candidate, SchedulerData},
};

/// Filters the candidates based on the scheduler options.
pub(super) struct CandidateFilter {
    /// The data needed to run the candidate filter.
    data: SchedulerData,
}

impl CandidateFilter {
    /// Constructs a new candidate filter.
    pub fn new(data: SchedulerData) -> Self {
        Self { data }
    }

    /// Filters the candidates whose score fit in the given window.
    fn candidates_in_window(
        candidates: &[Candidate],
        window_opts: &MasteryWindowOpts,
    ) -> Vec<Candidate> {
        candidates
            .iter()
            .filter(|c| window_opts.in_window(c.score))
            .cloned()
            .collect()
    }

    /// Takes a list of candidates and randomly selectes num_selected candidates among them. The
    /// probabilities of selecting a candidate are weighted based on their score and the number of
    /// hops taken by the graph search to find them. Lower scores and higher number of hops give the
    /// candidate a higher chance of being selected. The function returns a tuple of the selected
    /// candidates and the remainder.
    fn select_candidates(
        candidates: Vec<Candidate>,
        num_selected: usize,
    ) -> Result<(Vec<Candidate>, Vec<Candidate>)> {
        if candidates.len() <= num_selected {
            return Ok((candidates, vec![]));
        }

        let mut rng = thread_rng();
        let selected: Vec<Candidate> = candidates
            .choose_multiple_weighted(&mut rng, num_selected, |c| {
                1.0 + (5.0 - c.score) + (c.num_hops as f32)
            })?
            .cloned()
            .collect();
        let selected_uids: HashSet<u64> = selected.iter().map(|c| c.exercise_uid).collect();
        let remainder = candidates
            .iter()
            .filter(|c| !selected_uids.contains(&c.exercise_uid))
            .cloned()
            .collect();
        Ok((selected, remainder))
    }

    /// Fills up the candidates with the values from remainder if there are not enough candidates.
    fn add_remainder(
        batch_size: usize,
        final_candidates: &mut Vec<Candidate>,
        remainder_candidates: &[Candidate],
    ) {
        if final_candidates.len() < batch_size {
            let remainder = batch_size - final_candidates.len();
            final_candidates.extend(
                remainder_candidates[..remainder.min(remainder_candidates.len())]
                    .iter()
                    .cloned(),
            );
        }
    }

    /// Takes a list of candidates and returns a vector of tuples of exercises IDs and manifests.
    fn candidates_to_exercises(
        &self,
        candidates: Vec<Candidate>,
    ) -> Result<Vec<(String, ExerciseManifest)>> {
        let mut exercises = candidates
            .into_iter()
            .map(|c| -> Result<(String, ExerciseManifest)> {
                let id = self.data.get_id(c.exercise_uid)?;
                let manifest = self.data.get_exercise_manifest(c.exercise_uid)?;
                Ok((id, manifest))
            })
            .collect::<Result<Vec<(String, ExerciseManifest)>>>()?;
        exercises.shuffle(&mut thread_rng());
        Ok(exercises)
    }

    /// Takes a list of exercises and filters them so that the end result is a list of exercise
    /// manifests which fit the options given to the scheduler.
    pub fn filter_candidates(
        &self,
        candidates: Vec<Candidate>,
    ) -> Result<Vec<(String, ExerciseManifest)>> {
        let options = &self.data.options;
        let batch_size_float = options.batch_size as f32;

        // Find the candidates that fit in each window.
        let mastered_candidates =
            Self::candidates_in_window(&candidates, &options.mastered_window_opts);
        let easy_candidates = Self::candidates_in_window(&candidates, &options.easy_window_opts);
        let current_candidates =
            Self::candidates_in_window(&candidates, &options.current_window_opts);
        let target_candidates =
            Self::candidates_in_window(&candidates, &options.target_window_opts);
        let mut final_candidates = Vec::with_capacity(options.batch_size);

        // For each window, add the appropriate number of candidates to the final list.
        let num_mastered = (batch_size_float * options.mastered_window_opts.percentage) as usize;
        let (mastered_selected, mastered_remainder) =
            Self::select_candidates(mastered_candidates, num_mastered)?;
        final_candidates.extend(mastered_selected);

        let num_easy = (batch_size_float * options.easy_window_opts.percentage) as usize;
        let (easy_selected, easy_remainder) = Self::select_candidates(easy_candidates, num_easy)?;
        final_candidates.extend(easy_selected);

        let num_current = (batch_size_float * options.current_window_opts.percentage) as usize;
        let (current_selected, current_remainder) =
            Self::select_candidates(current_candidates, num_current)?;
        final_candidates.extend(current_selected);

        // For the target window, add as many candidates as possible to fill the batch.
        let remainder = options.batch_size - final_candidates.len();
        let (target_selected, _) = Self::select_candidates(target_candidates, remainder)?;
        final_candidates.extend(target_selected);

        // Go through the remainders in descending order of difficulty and add them to the list of
        // final candidates if there's still space left in the batch.
        Self::add_remainder(
            options.batch_size,
            &mut final_candidates,
            &current_remainder,
        );
        Self::add_remainder(options.batch_size, &mut final_candidates, &easy_remainder);
        Self::add_remainder(
            options.batch_size,
            &mut final_candidates,
            &mastered_remainder,
        );

        self.candidates_to_exercises(final_candidates)
    }
}
