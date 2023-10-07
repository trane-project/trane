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
use lazy_static::lazy_static;
use rand::{prelude::SliceRandom, thread_rng};
use ustr::{Ustr, UstrMap, UstrSet};

use crate::{
    data::{ExerciseManifest, MasteryWindow},
    scheduler::{Candidate, SchedulerData},
};

/// The initial weight of each candidate.
const INITIAL_WEIGHT: f32 = 1.0;

/// The part of the weight that depends on the score will be the product of the difference between
/// the score and the maximum score and this factor.
const SCORE_WEIGHT_FACTOR: f32 = 40.0;

/// The part of the weight that depends on the depth of the candidate will be the product of the
/// depth and this factor.
const DEPTH_WEIGHT_FACTOR: f32 = 5.0;

/// The part of the weight that depends on the depth of the candidate will be capped at this value.
const MAX_DEPTH_WEIGHT: f32 = 200.0;

/// The part of the weight that depends on the frequency of the candidate will be capped at this
/// value. Each time an exercise is scheduled, this portion of the weight is reduced by a factor.
const MAX_FREQUENCY_WEIGHT: f32 = 200.0;

/// The factor by which the weight is mulitiplied when the frequency is increased.
const FREQUENCY_FACTOR: f32 = 0.5;

/// The part of the weight that depends on the number of trials for that exercise will be capped at
/// this value. Each time an exercise is scheduled, this portion of the weight is reduced by a
/// factor.
const MAX_NUM_TRIALS_WEIGHT: f32 = 200.0;

/// The factor by which the weight is mulitiplied when the number of trials is increased.
const NUM_TRIALS_FACTOR: f32 = 0.5;

lazy_static! {
    /// A list of precomputed weights based on frequency to save on computation time. Candidates
    /// with higher frequencies than the capacity of this array are assigned a weight of zero.
    static ref PRECOMPUTED_FREQUENCY_WEIGHTS: [f32; 10] = [
        MAX_FREQUENCY_WEIGHT,
        MAX_FREQUENCY_WEIGHT * FREQUENCY_FACTOR,
        MAX_FREQUENCY_WEIGHT * FREQUENCY_FACTOR.powf(2.0),
        MAX_FREQUENCY_WEIGHT * FREQUENCY_FACTOR.powf(3.0),
        MAX_FREQUENCY_WEIGHT * FREQUENCY_FACTOR.powf(4.0),
        MAX_FREQUENCY_WEIGHT * FREQUENCY_FACTOR.powf(5.0),
        MAX_FREQUENCY_WEIGHT * FREQUENCY_FACTOR.powf(6.0),
        MAX_FREQUENCY_WEIGHT * FREQUENCY_FACTOR.powf(7.0),
        MAX_FREQUENCY_WEIGHT * FREQUENCY_FACTOR.powf(8.0),
        MAX_FREQUENCY_WEIGHT * FREQUENCY_FACTOR.powf(9.0),
    ];

    /// A list of precomputed weights based on the number of trials to save on computation time.
    /// Candidates with more trials than the capacity of this array are assigned a weight of zero.
    static ref PRECOMPUTED_NUM_TRIALS_WEIGHTS: [f32; 10] = [
        MAX_NUM_TRIALS_WEIGHT,
        MAX_NUM_TRIALS_WEIGHT * NUM_TRIALS_FACTOR,
        MAX_NUM_TRIALS_WEIGHT * NUM_TRIALS_FACTOR.powf(2.0),
        MAX_NUM_TRIALS_WEIGHT * NUM_TRIALS_FACTOR.powf(3.0),
        MAX_NUM_TRIALS_WEIGHT * NUM_TRIALS_FACTOR.powf(4.0),
        MAX_NUM_TRIALS_WEIGHT * NUM_TRIALS_FACTOR.powf(5.0),
        MAX_NUM_TRIALS_WEIGHT * NUM_TRIALS_FACTOR.powf(6.0),
        MAX_NUM_TRIALS_WEIGHT * NUM_TRIALS_FACTOR.powf(7.0),
        MAX_NUM_TRIALS_WEIGHT * NUM_TRIALS_FACTOR.powf(8.0),
        MAX_NUM_TRIALS_WEIGHT * NUM_TRIALS_FACTOR.powf(9.0),
    ];
}

/// The maximum weight that depends on the frequency of the lesson. The weight will be divided
/// equally among all the exercises from the same lesson.
const MAX_LESSON_FREQUENCY_WEIGHT: f32 = 200.0;

/// The batch size will be adjusted if there are not enough candidates (at least three times the
/// batch size) to create a batch of the size specified in the scheduler options. This value is the
/// minimum value for such an adjustment.
const MIN_DYNAMIC_BATCH_SIZE: usize = 10;

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

    /// Counts the number of candidates in each lesson.
    fn count_lesson_frequency(candidates: &[Candidate]) -> UstrMap<usize> {
        let mut lesson_frequency = UstrMap::default();
        for candidate in candidates {
            *lesson_frequency.entry(candidate.lesson_id).or_default() += 1;
        }
        lesson_frequency
    }

    /// Takes a list of candidates and randomly selects `num_to_select` candidates among them. The
    /// probabilities of selecting a candidate are weighted based on the following:
    ///
    /// 1. The candidate's score. A higher score is assigned less weight to present scores with
    ///    lower scores among those in the same mastery window.
    /// 2. The number of hops taken by the graph search to find the candidate. A higher number of
    ///    hops is assigned more weight to avoid only selecting exercises that are very close to the
    ///    start of the graph.
    /// 3. The frequency with which the candidate has been scheduled during the run of the
    ///    scheduler. A higher frequency is assigned less weight to avoid selecting the same
    ///    exercises too often.
    /// 4. The number of trials for that exercise. A higher number of trials is assigned less weight
    ///    to favor exercises that have been practiced fewer times.
    /// 5. The number of candidates in the same lesson. The more candidates there are in the same
    ///    lesson, the less weight each candidate is assigned to avoid selecting too many exercises
    ///    from the same lesson.
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

        // Count the number of candidates in each lesson.
        let lesson_frequency = Self::count_lesson_frequency(&candidates);

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
                weight += PRECOMPUTED_FREQUENCY_WEIGHTS
                    .get(c.frequency)
                    .unwrap_or(&0.0);

                // Increase the weight based on the number of trials for that exercise. Exercises
                // with more trials are assigned less weight.
                weight += PRECOMPUTED_NUM_TRIALS_WEIGHTS
                    .get(c.num_trials)
                    .unwrap_or(&0.0);

                // Increase the weight based on the number of candidates in the same lesson. The
                // more candidates there are in the same lesson, the less weight each candidate is
                // assigned.
                weight += MAX_LESSON_FREQUENCY_WEIGHT / lesson_frequency[&c.lesson_id] as f32;

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
        max_added: Option<usize>,
    ) {
        // Do not fill batches past 3/4 of the batch size to avoid creating unbalanced batches.
        if final_candidates.len() >= batch_size * 3 / 4 {
            return;
        }

        // If a maximum number of exercises to add has been specified, use that value. Otherwise,
        // fill up the remaining space in the batch.
        let num_remainder = batch_size - final_candidates.len();
        let num_added = match max_added {
            None => num_remainder,
            Some(max) => num_remainder.min(max),
        };
        let (remainder_candidates, _) = Self::select_candidates(remainder, num_added);
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
            .map(|c| -> Result<(Ustr, _)> {
                let manifest = self.data.get_exercise_manifest(&c.exercise_id)?;
                Ok((c.exercise_id, manifest))
            })
            .collect::<Result<Vec<(Ustr, _)>>>()?; // grcov-excl-line

        // Shuffle the list one more time to add more randomness to the final batch.
        exercises.shuffle(&mut thread_rng());

        Ok(exercises)
    }

    /// Computes the batch size to use based on the number of candidates and the batch size defined
    /// in the scheduler options.
    fn dynamic_batch_size(batch_size: usize, num_candidates: usize) -> usize {
        // Do not adjust the batch size if it's already small.
        if batch_size < MIN_DYNAMIC_BATCH_SIZE {
            return batch_size;
        }

        // If there are fewer candidates than three times the batch size, using the full batch size
        // would result in suboptimal filtering. Reduce the batch size to one third of the number
        // of candidates. Otherwise, keep the batch size as is.
        if num_candidates < batch_size * 3 {
            return (num_candidates / 3).max(MIN_DYNAMIC_BATCH_SIZE);
        }
        batch_size
    }

    /// Takes a list of exercises and filters them so that the end result is a list of exercise
    /// manifests which fit the mastery windows defined in the scheduler options.
    pub fn filter_candidates(
        &self,
        candidates: Vec<Candidate>,
    ) -> Result<Vec<(Ustr, ExerciseManifest)>> {
        // Find the batch size to use.
        let options = &self.data.options;
        let batch_size = Self::dynamic_batch_size(options.batch_size, candidates.len());
        let batch_size_float = batch_size as f32;

        // Find the candidates that fit in each window.
        let mastered_candidates =
            Self::candidates_in_window(&candidates, &options.mastered_window_opts);
        let easy_candidates = Self::candidates_in_window(&candidates, &options.easy_window_opts);
        let current_candidates =
            Self::candidates_in_window(&candidates, &options.current_window_opts);
        let target_candidates =
            Self::candidates_in_window(&candidates, &options.target_window_opts);
        let new_candidates = Self::candidates_in_window(&candidates, &options.new_window_opts);

        // Initialize the final list. For each window in descending order of mastery, add the
        // appropriate number of candidates to the final list.
        let mut final_candidates = Vec::with_capacity(batch_size);
        let num_mastered =
            (batch_size_float * options.mastered_window_opts.percentage).max(1.0) as usize;
        let (mastered_selected, mastered_remainder) =
            Self::select_candidates(mastered_candidates, num_mastered);
        final_candidates.extend(mastered_selected);

        // Add elements from the easy window.
        let num_easy = (batch_size_float * options.easy_window_opts.percentage).max(1.0) as usize;
        let (easy_selected, easy_remainder) = Self::select_candidates(easy_candidates, num_easy);
        final_candidates.extend(easy_selected);

        // Add elements from the current window.
        let num_current =
            (batch_size_float * options.current_window_opts.percentage).max(1.0) as usize;
        let (current_selected, current_remainder) =
            Self::select_candidates(current_candidates, num_current);
        final_candidates.extend(current_selected);

        // Add elements from the target window.
        let num_target =
            (batch_size_float * options.target_window_opts.percentage).max(1.0) as usize;
        let (target_selected, target_remainder) =
            Self::select_candidates(target_candidates, num_target);
        final_candidates.extend(target_selected);

        // Add elements from the new window.
        let num_new = (batch_size_float * options.new_window_opts.percentage).max(1.0) as usize;
        let (new_selected, new_remainder) = Self::select_candidates(new_candidates, num_new);
        final_candidates.extend(new_selected);

        // Go through the remainders and add them to the list of final candidates if there's still
        // space left in the batch. Add the remainder from the new, current, target, easy, and
        // mastered windows, in that order. Limit the number of too easy or too hard exercises to
        // avoid creating unbalanced batches.
        Self::add_remainder(batch_size, &mut final_candidates, new_remainder, None);
        Self::add_remainder(batch_size, &mut final_candidates, current_remainder, None);
        Self::add_remainder(
            batch_size,
            &mut final_candidates,
            target_remainder,
            Some(20),
        );
        Self::add_remainder(batch_size, &mut final_candidates, easy_remainder, Some(10));
        Self::add_remainder(
            batch_size,
            &mut final_candidates,
            mastered_remainder,
            Some(5),
        );

        // Convert the list of candidates into a list of tuples of exercise IDs and manifests.
        self.candidates_to_exercises(final_candidates)
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use ustr::Ustr;

    use crate::scheduler::{
        filter::{CandidateFilter, MIN_DYNAMIC_BATCH_SIZE},
        Candidate,
    };

    /// Verifies that the batch size is adjusted based on the number of candidates.
    #[test]
    fn dynamic_batch_size() -> Result<()> {
        // Small batch sizes are unaffected.
        assert_eq!(CandidateFilter::dynamic_batch_size(5, 10), 5);

        // The batch size is adjusted if there are not enough candidates.
        assert_eq!(CandidateFilter::dynamic_batch_size(50, 70), 70 / 3);
        assert_eq!(
            CandidateFilter::dynamic_batch_size(50, 10),
            MIN_DYNAMIC_BATCH_SIZE
        );

        // The batch size from the options is used if there are enough candidates.
        assert_eq!(CandidateFilter::dynamic_batch_size(50, 150), 50);
        assert_eq!(CandidateFilter::dynamic_batch_size(50, 200), 50);
        Ok(())
    }

    /// Verifies that the candidates per lesson are counted correctly.
    #[test]
    fn count_lesson_frequency() -> Result<()> {
        // Create a list of candidates with different lessons.
        let candidates = vec![
            Candidate {
                exercise_id: Ustr::from("exercise1"),
                lesson_id: Ustr::from("lesson1"),
                depth: 0.0,
                score: 0.0,
                num_trials: 0,
                frequency: 0,
            },
            Candidate {
                exercise_id: Ustr::from("exercise2"),
                lesson_id: Ustr::from("lesson1"),
                depth: 0.0,
                score: 0.0,
                num_trials: 0,
                frequency: 0,
            },
            Candidate {
                exercise_id: Ustr::from("exercise3"),
                lesson_id: Ustr::from("lesson2"),
                depth: 0.0,
                score: 0.0,
                num_trials: 0,
                frequency: 0,
            },
            Candidate {
                exercise_id: Ustr::from("exercise4"),
                lesson_id: Ustr::from(""),
                depth: 0.0,
                score: 0.0,
                num_trials: 0,
                frequency: 0,
            },
        ];

        // Count the number of candidates per lesson.
        let lesson_frequency = CandidateFilter::count_lesson_frequency(&candidates);
        assert_eq!(lesson_frequency.len(), 3);
        assert_eq!(lesson_frequency.get(&Ustr::from("lesson1")), Some(&2));
        assert_eq!(lesson_frequency.get(&Ustr::from("lesson2")), Some(&1));
        assert_eq!(lesson_frequency.get(&Ustr::from("")), Some(&1));

        Ok(())
    }
}
