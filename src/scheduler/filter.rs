//! Contains the logic for how candidate exercises found during the search part of the scheduling
//! are filtered down to the final batch of exercises.
//!
//! Once the search part of the scheduling algorithm selects an initial set of candidate, Trane must
//! find a good mix of exercises from different levels of difficulty. The aim is to have a batch of
//! exercises that is not too challenging, but also not too easy. The algorithm has two main parts:
//!
//! 1. Bucket all the candidates into the mastery windows defined in the scheduler options.
//! 2. Select a random subset of exercises from each bucket. The random selection is weighted by a
//!    number of factors, including the number of hops that were needed to reach a candidate, the
//!    score, and the frequency with which the exercise has been scheduled in the past.

use anyhow::Result;
use rand::{prelude::SliceRandom, thread_rng};
use ustr::{Ustr, UstrSet};

use crate::{
    data::{ExerciseManifest, MasteryWindow},
    scheduler::{Candidate, SchedulerData},
};

/// The initial weight of each candidate.
const INITIAL_WEIGHT: f32 = 1.0;

/// The part of the weight that depends on the score will be multiplied by this factor.
const SCORE_WEIGHT_FACTOR: f32 = 20.0;

/// The part of the weight that depends on the depth of the candidate will be multiplied by this
/// factor.
const DEPTH_WEIGHT_FACTOR: f32 = 5.0;

/// The part of the weight that depends on the weight of the candidate will be capped at this value.
const MAX_DEPTH_WEIGHT: f32 = 100.0;

/// The part of the weight that depends on the frequency of the candidate will be capped at this
/// value. Each time an exercise is scheduled, this portion of the weight is halved.
const MAX_FREQUENCY_WEIGHT: f32 = 200.0;

/// The filter used to reduce the candidates found during the search to a final batch of exercises.
pub(super) struct CandidateFilter {
    /// The data needed to run the candidate filter.
    data: SchedulerData,
}

impl CandidateFilter {
    /// Constructs a new candidate filter.
    pub fn new(data: SchedulerData) -> Self {
        Self { data }
    }

    /// Filters the candidates whose score fit in the given mastery window.
    fn candidates_in_window(
        candidates: &[Candidate],
        window_opts: &MasteryWindow,
    ) -> Vec<Candidate> {
        candidates
            .iter()
            .filter(|c| window_opts.in_window(c.score))
            .cloned()
            .collect()
    }

    /// Takes a list of candidates and randomly selects `num_selected` candidates among them. The
    /// probabilities of selecting a candidate are weighted based on the following:
    /// 1. The candidate's score. A higher score is assigned less weight to present scores with
    ///    lower scores among those in the same mastery window.
    /// 2. The number of hops taken by the graph search to find the candidate. A higher number of
    ///    hops is assigned more weight to avoid only selecting exercises that are very close to the
    ///    start of the graph.
    /// 3. The frequency with which the candidate has been scheduled during the run of the
    ///    scheduler. A higher frequency is assigned less weight to avoid selecting the same
    ///    exercises too often.
    ///
    /// The function returns a tuple of the selected candidates and the remainder exercises. The
    /// remainder will be used to fill the batch in case there is space left after the first round
    /// of filtering.
    fn select_candidates(
        candidates: Vec<Candidate>,
        num_to_select: usize,
    ) -> (Vec<Candidate>, Vec<Candidate>) {
        // Return the list if there are fewer candidates than the number to select.
        if candidates.len() <= num_to_select {
            return (candidates, vec![]);
        }

        // Otherwise, assign a weight to each candidate and perform a weighted random selection.
        // Safe to unwrap the result, as this function panics if `num_to_select` is greater than the
        // size of `candidates`, but that is checked above.
        let mut rng = thread_rng();
        let selected: Vec<Candidate> = candidates
            .choose_multiple_weighted(&mut rng, num_to_select, |c| {
                // Always assign an initial weight of to avoid assigning a zero weight.
                let mut weight = INITIAL_WEIGHT;

                // A portion of the score will depend on the score of the candidate. Lower scores
                // are given more weight.
                weight += SCORE_WEIGHT_FACTOR * (5.0 - c.score).max(0.0);

                // A part of the score will depend on the number of hops that were needed to reach
                // the candidate. It will be capped at a maximum.
                weight += (DEPTH_WEIGHT_FACTOR * c.depth).max(MAX_DEPTH_WEIGHT);

                // Increase the weight based on the frequency with which the exercise has been
                // scheduled. Exercises that have been scheduled more often are assigned less
                // weight.
                weight += MAX_FREQUENCY_WEIGHT / 2.0_f32.powf(c.frequency);
                weight
            })
            .unwrap()
            .cloned()
            .collect();
        let selected_ids: UstrSet = selected.iter().map(|c| c.exercise_id).collect();

        // Compute which exercises were not selected in the previous step.
        let remainder = candidates
            .iter()
            .filter(|c| !selected_ids.contains(&c.exercise_id))
            .cloned()
            .collect();

        (selected, remainder)
    }

    /// Fills up the lists of final candidates with the values from remainder if there are not
    /// enough candidates.
    fn add_remainder(
        batch_size: usize,
        final_candidates: &mut Vec<Candidate>,
        remainder: Vec<Candidate>,
    ) {
        // The batch is already full, so there's nothing to do.
        if final_candidates.len() >= batch_size {
            return;
        }

        // Otherwise, select as many candidates as possible from the remainder.
        let num_remainder = batch_size - final_candidates.len();
        let (remainder_candidates, _) = Self::select_candidates(remainder, num_remainder);
        final_candidates.extend(remainder_candidates);
    }

    /// Takes a list of candidates and returns a vector of tuples of exercises IDs and manifests.
    fn candidates_to_exercises(
        &self,
        candidates: Vec<Candidate>,
    ) -> Result<Vec<(Ustr, ExerciseManifest)>> {
        // Retrieve the manifests for each candidate.
        let mut exercises = candidates
            .into_iter()
            .map(|c| -> Result<(Ustr, ExerciseManifest)> {
                let manifest = self.data.get_exercise_manifest(&c.exercise_id)?;
                Ok((c.exercise_id, manifest))
            })
            .collect::<Result<Vec<(Ustr, ExerciseManifest)>>>()?; // grcov-excl-line

        // Shuffle the list one more time to add more randomness to the final batch.
        exercises.shuffle(&mut thread_rng());

        Ok(exercises)
    }

    /// Takes a list of exercises and filters them so that the end result is a list of exercise
    /// manifests which fit the mastery windows defined in the scheduler options.
    pub fn filter_candidates(
        &self,
        candidates: Vec<Candidate>,
    ) -> Result<Vec<(Ustr, ExerciseManifest)>> {
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

        // Initialize the final list. For each window in descending order of mastery, add the
        // appropriate number of candidates to the final list.
        let mut final_candidates = Vec::with_capacity(options.batch_size);
        let num_mastered = (batch_size_float * options.mastered_window_opts.percentage) as usize;
        let (mastered_selected, mastered_remainder) =
            Self::select_candidates(mastered_candidates, num_mastered);
        final_candidates.extend(mastered_selected);

        // Add elements from the easy window.
        let num_easy = (batch_size_float * options.easy_window_opts.percentage) as usize;
        let (easy_selected, easy_remainder) = Self::select_candidates(easy_candidates, num_easy);
        final_candidates.extend(easy_selected);

        // Add elements from the current window.
        let num_current = (batch_size_float * options.current_window_opts.percentage) as usize;
        let (current_selected, current_remainder) =
            Self::select_candidates(current_candidates, num_current);
        final_candidates.extend(current_selected);

        // For the target window, add as many candidates as possible to fill the batch.
        let remainder = options.batch_size - final_candidates.len();
        let (target_selected, _) = Self::select_candidates(target_candidates, remainder);
        final_candidates.extend(target_selected);

        // Go through the remainders in ascending order of difficulty and add them to the list of
        // final candidates if there's still space left in the batch.
        Self::add_remainder(
            options.batch_size,
            &mut final_candidates,
            mastered_remainder,
        );
        Self::add_remainder(options.batch_size, &mut final_candidates, easy_remainder);
        Self::add_remainder(options.batch_size, &mut final_candidates, current_remainder);

        // Convert the list of candidates into a list of tuples of exercise IDs and manifests.
        self.candidates_to_exercises(final_candidates)
    }
}
