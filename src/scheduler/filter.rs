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
use rand::{prelude::SliceRandom, rng, seq::IndexedRandom};
use ustr::{UstrMap, UstrSet};

use crate::{
    data::{ExerciseManifest, MasteryWindow},
    scheduler::{Candidate, SchedulerData, review_knocker::KnockoutResult},
};

/// The minimum weight for each candidate. This is used to prevent any candidate from becoming too
/// unlikely to be selected.
const MIN_WEIGHT: f32 = 100.0;

/// The part of the weight that depends on the exercise's score will (5.0 - score) times this
/// factor.
const EXERCISE_SCORE_WEIGHT_FACTOR: f32 = 200.0;

/// The part of the weight that depends on the lesson's score will be (5.0 - score) times this
/// factor.
const LESSON_SCORE_WEIGHT_FACTOR: f32 = 100.0;

/// The part of the weight that depends on the course's score will be (5.0 - score) times this
/// factor.
const COURSE_SCORE_WEIGHT_FACTOR: f32 = 50.0;

/// The part of the weight that depends on the frequency with which the exercise is encompassed by
/// other exercises in the initial batch will be this value divided by that frequency.
const MAX_ENCOMPASSED_WEIGHT: f32 = 1000.0;

/// The part of the weight that depends on the depth of the candidate will be the product of the
/// depth and this factor.
const DEPTH_WEIGHT_FACTOR: f32 = 25.0;

/// The part of the weight that depends on the depth of the candidate will be capped at this value.
const MAX_DEPTH_WEIGHT: f32 = 1000.0;

/// The part of the weight that depends on the number of times this exercise is scheduled during the
/// run of the program will be capped at this value. Each time an exercise is scheduled, this
/// portion of the weight is reduced by a factor.
const MAX_SCHEDULED_WEIGHT: f32 = 1000.0;

/// The factor by which the weight is mulitiplied every time the same exercise is scheduled during a
/// single run of the program.
const SCHEDULED_FACTOR: f32 = 0.5;

/// The part of the weight that depends on the number of trials for that exercise will be capped at
/// this value. Each time an exercise is scheduled, this portion of the weight is reduced by a
/// factor.
const MAX_NUM_TRIALS_WEIGHT: f32 = 1000.0;

/// The factor by which the weight is mulitiplied when the number of trials is increased.
const NUM_TRIALS_FACTOR: f32 = 0.75;

/// The part of the weight that depends on the number of days since the exercise was last seen.
const LAST_SEEN_WEIGHT_PER_DAY: f32 = 10.0;

/// The maximum amount of weight this component can add.
const MAX_LAST_SEEN_WEIGHT: f32 = 1000.0;

/// The maximum weight that depends on the frequency of exercises from the same lesson. The weight
/// will be divided equally among all the exercises from the same lesson.
const MAX_LESSON_FREQUENCY_WEIGHT: f32 = 1000.0;

/// The maximum weight that depends on the frequency of exercises from the same course. The weight
/// will be divided equally among all the exercises from the same course.
const MAX_COURSE_FREQUENCY_WEIGHT: f32 = 1000.0;

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
        encompassed_set: &UstrSet,
        window_opts: &MasteryWindow,
    ) -> Vec<Candidate> {
        candidates
            .iter()
            .filter(|c| window_opts.in_window(c.exercise_score))
            .filter(|c| !encompassed_set.contains(&c.exercise_id))
            .cloned()
            .collect()
    }

    /// Counts the number of candidates from each lesson.
    fn count_lesson_frequency(candidates: &[Candidate]) -> UstrMap<u32> {
        let mut lesson_frequency = UstrMap::default();
        for candidate in candidates {
            *lesson_frequency.entry(candidate.lesson_id).or_default() += 1;
        }
        lesson_frequency
    }

    /// Counts the number of candidates from each course.
    fn count_course_frequency(candidates: &[Candidate]) -> UstrMap<u32> {
        let mut course_frequency = UstrMap::default();
        for candidate in candidates {
            *course_frequency.entry(candidate.course_id).or_default() += 1;
        }
        course_frequency
    }

    /// Computes the weight assigned to a candidate that will be used to select it during the
    /// filtering phase. The weight is based on the following factors:
    ///
    /// 1. The candidate's exercise score. A higher score is assigned less weight to give them
    ///    precedence over candidates with lower scores.
    /// 2. The candidate's lesson score. Exercises from lessons with a higher score will be shown
    ///    less often.
    /// 3. The candidate's course score. Exercises from courses with a higher score will be shown
    ///    less often.
    /// 4. The frequency with which the candidate is encompassed by other exercises in the initial
    ///    batch. This means that reviewing those other exercises will implicitly review this one. A
    ///    higher frequency is assigned less weight.
    /// 5. The number of hops taken by the graph search to find the candidate. A higher number of
    ///    hops is assigned more weight to give precedence to candidates from more advanced
    ///    material.
    /// 6. The frequency with which the candidate has been scheduled during the run of the
    ///    scheduler. A higher frequency is assigned less weight to avoid selecting the same
    ///    exercises too often during the same session.
    /// 7. The number of trials for that candidate. A higher number of trials is assigned less
    ///    weight to favor exercises that have been practiced fewer times.
    /// 8. The number of days since this candidate was last seen. More days since last seen gets
    ///    more weight.
    /// 9. The number of candidates in the same lesson. The more candidates there are in the same
    ///    lesson, the less weight each candidate is assigned to avoid selecting too many exercises
    ///    from the same lesson.
    /// 10. The number of candidates in the same course. The same logic applies as for the lesson
    ///     frequency.
    fn candidate_weight(
        c: &Candidate,
        encompassed_freq: u32,
        lesson_freq: u32,
        course_freq: u32,
    ) -> f32 {
        // A part of the score will depend on the score of the exercise.
        let mut weight = EXERCISE_SCORE_WEIGHT_FACTOR * (5.0 - c.exercise_score).max(0.0);

        // A part of the score will depend on the score of the lesson.
        weight += LESSON_SCORE_WEIGHT_FACTOR * (5.0 - c.lesson_score).max(0.0);

        // A part of the score will depend on the score of the course.
        weight += COURSE_SCORE_WEIGHT_FACTOR * (5.0 - c.course_score).max(0.0);

        // A part of the score will depend on the frequency with which the exercise is encompassed by other
        // exercises in the initial batch.
        weight += MAX_ENCOMPASSED_WEIGHT / (encompassed_freq.max(1) as f32);

        // A part of the score will depend on the number of hops that were needed to reach
        // the candidate.
        weight += (DEPTH_WEIGHT_FACTOR * c.depth).clamp(0.0, MAX_DEPTH_WEIGHT);

        // A part of the weight is based on the frequency with which the exercise has been
        // scheduled.
        weight += MAX_SCHEDULED_WEIGHT * SCHEDULED_FACTOR.powf(c.frequency as f32);

        // A part of the weight is based on the number of trials for that exercise.
        weight += MAX_NUM_TRIALS_WEIGHT * NUM_TRIALS_FACTOR.powf(c.num_trials as f32);

        // A part of the weight is based on the number of days since this exercise was last seen.
        weight += (LAST_SEEN_WEIGHT_PER_DAY * c.last_seen).clamp(0.0, MAX_LAST_SEEN_WEIGHT);

        // A part of the weight is based on the number of candidates in the same lesson.
        weight += MAX_LESSON_FREQUENCY_WEIGHT / lesson_freq.max(1) as f32;

        // A part of the weight is based on the number of candidates in the same course.
        weight += MAX_COURSE_FREQUENCY_WEIGHT / course_freq.max(1) as f32;

        // Give each candidates a minimum weight.
        weight.max(MIN_WEIGHT)
    }

    /// Takes a list of candidates and randomly selects `num_to_select` candidates among them. Each
    /// candidate is given a weight based on a number of factors meant to favor candidates that are
    /// optimal for practice. The function returns a tuple of the selected candidates and the
    /// remainder exercises. The remainder will be used to fill the batch in case there is space
    /// left after the first round of filtering.
    fn select_candidates(
        candidates: &[Candidate],
        frequency_map: &UstrMap<u32>,
        num_to_select: usize,
    ) -> (Vec<Candidate>, Vec<Candidate>) {
        // Return the list if there are fewer candidates than the number to select.
        if candidates.len() <= num_to_select {
            return (candidates.to_vec(), vec![]);
        }

        // Count the number of candidates in each lesson and course.
        let lesson_freq = Self::count_lesson_frequency(candidates);
        let course_freq = Self::count_course_frequency(candidates);

        // Otherwise, assign a weight to each candidate and perform a weighted random selection.
        // Safe to unwrap the result, as this function panics if `num_to_select` is greater than the
        // size of `candidates`, but that is checked above.
        let mut rng = rng();
        let selected: Vec<Candidate> = candidates
            .choose_multiple_weighted(&mut rng, num_to_select, |c| {
                let encompassed_frequency = frequency_map.get(&c.exercise_id).copied().unwrap_or(0);
                Self::candidate_weight(
                    c,
                    encompassed_frequency,
                    lesson_freq.get(&c.lesson_id).copied().unwrap_or(0),
                    course_freq.get(&c.course_id).copied().unwrap_or(0),
                )
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
        remainder: &[Candidate],
        frequency_map: &UstrMap<u32>,
        max_added: Option<usize>,
    ) {
        // Do not fill batches past 2/3 of the batch size to avoid creating unbalanced batches.
        if final_candidates.len() >= batch_size * 2 / 3 {
            return;
        }

        // If a maximum number of exercises to add has been specified, use that value. Otherwise,
        // fill up the remaining space in the batch.
        let num_remainder = batch_size - final_candidates.len();
        let num_added = match max_added {
            None => num_remainder,
            Some(max) => num_remainder.min(max),
        };
        let (remainder_candidates, _) =
            Self::select_candidates(remainder, frequency_map, num_added);
        final_candidates.extend(remainder_candidates);
    }

    /// Takes a list of candidates and returns a vector of tuples of exercises IDs and manifests.
    fn candidates_to_exercises(&self, candidates: Vec<Candidate>) -> Result<Vec<ExerciseManifest>> {
        // Retrieve the manifests for each candidate.
        let mut exercises = candidates
            .into_iter()
            .map(|c| -> Result<_> {
                let manifest = self.data.get_exercise_manifest(c.exercise_id)?;
                Ok(manifest)
            })
            .collect::<Result<Vec<_>>>()?;

        // Shuffle the list one more time to add more randomness to the final batch.
        exercises.shuffle(&mut rng());
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
    pub fn filter_candidates(&self, result: KnockoutResult) -> Result<Vec<ExerciseManifest>> {
        // Find the batch size to use.
        let candidates = &result.candidates;
        let options = &self.data.options;
        let batch_size = Self::dynamic_batch_size(options.batch_size, candidates.len());
        let batch_size_float = batch_size as f32;

        // Find the candidates that fit in each window. Then combine the mastered and highly
        // encompassed candidates into a single window to ensure that they are not overrepresented
        // in the final batch.
        let encompassed_set: UstrSet = result
            .highly_encompassed
            .iter()
            .map(|c| c.exercise_id)
            .collect();
        let mastered_candidates =
            Self::candidates_in_window(candidates, &encompassed_set, &options.mastered_window_opts);
        let easy_candidates =
            Self::candidates_in_window(candidates, &encompassed_set, &options.easy_window_opts);
        let current_candidates =
            Self::candidates_in_window(candidates, &encompassed_set, &options.current_window_opts);
        let target_candidates =
            Self::candidates_in_window(candidates, &encompassed_set, &options.target_window_opts);
        let new_candidates =
            Self::candidates_in_window(candidates, &encompassed_set, &options.new_window_opts);
        let mastered_candidates = [mastered_candidates, result.highly_encompassed].concat();

        // Initialize the final list. For each window in descending order of mastery, add the
        // appropriate number of candidates to the final list.
        let mut final_candidates = Vec::with_capacity(batch_size);
        let num_mastered =
            (batch_size_float * options.mastered_window_opts.percentage).max(1.0) as usize;
        let frequency_map = &result.frequency_map;
        let (mastered_selected, mastered_remainder) =
            Self::select_candidates(&mastered_candidates, frequency_map, num_mastered);
        final_candidates.extend(mastered_selected);

        // Add elements from the easy window.
        let num_easy = (batch_size_float * options.easy_window_opts.percentage).max(1.0) as usize;
        let (easy_selected, easy_remainder) =
            Self::select_candidates(&easy_candidates, frequency_map, num_easy);
        final_candidates.extend(easy_selected);

        // Add elements from the current window.
        let num_current =
            (batch_size_float * options.current_window_opts.percentage).max(1.0) as usize;
        let (current_selected, current_remainder) =
            Self::select_candidates(&current_candidates, frequency_map, num_current);
        final_candidates.extend(current_selected);

        // Add elements from the target window.
        let num_target =
            (batch_size_float * options.target_window_opts.percentage).max(1.0) as usize;
        let (target_selected, target_remainder) =
            Self::select_candidates(&target_candidates, frequency_map, num_target);
        final_candidates.extend(target_selected);

        // Add elements from the new window.
        let num_new = (batch_size_float * options.new_window_opts.percentage).max(1.0) as usize;
        let (new_selected, new_remainder) =
            Self::select_candidates(&new_candidates, frequency_map, num_new);
        final_candidates.extend(new_selected);

        // Go through the remainders and add them to the list of final candidates if there's still
        // space left in the batch. Add the remainder from the current, new, target, easy, and
        // mastered windows, in that order. Limit the number of too easy or too hard exercises to
        // avoid creating unbalanced batches.
        //
        // The number of exercises added is a multiple of 1/20th of the batch size to make the
        // values proportional to it.
        let base_remainder = (batch_size / 20).max(1);
        Self::add_remainder(
            batch_size,
            &mut final_candidates,
            &current_remainder,
            frequency_map,
            None,
        );
        Self::add_remainder(
            batch_size,
            &mut final_candidates,
            &new_remainder,
            frequency_map,
            Some(5 * base_remainder),
        );
        Self::add_remainder(
            batch_size,
            &mut final_candidates,
            &target_remainder,
            frequency_map,
            Some(3 * base_remainder),
        );
        Self::add_remainder(
            batch_size,
            &mut final_candidates,
            &easy_remainder,
            frequency_map,
            Some(2 * base_remainder),
        );
        Self::add_remainder(
            batch_size,
            &mut final_candidates,
            &mastered_remainder,
            frequency_map,
            Some(base_remainder),
        );

        // Convert the list of candidates into a list of tuples of exercise IDs and manifests.
        self.candidates_to_exercises(final_candidates)
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use ustr::Ustr;

    use super::*;
    use crate::scheduler::Candidate;

    /// Verifies that the batch size is adjusted based on the number of candidates.
    #[test]
    fn dynamic_batch_size() {
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
    }

    /// Verifies that the candidates per lesson are counted correctly.
    #[test]
    fn count_lesson_frequency() {
        // Create a list of candidates with different lessons.
        let candidates = vec![
            Candidate {
                exercise_id: Ustr::from("exercise1"),
                lesson_id: Ustr::from("lesson1"),
                course_id: Ustr::from("course1"),
                depth: 0.0,
                exercise_score: 0.0,
                lesson_score: 0.0,
                course_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
                frequency: 0,
            },
            Candidate {
                exercise_id: Ustr::from("exercise2"),
                lesson_id: Ustr::from("lesson1"),
                course_id: Ustr::from("course1"),
                depth: 0.0,
                exercise_score: 0.0,
                lesson_score: 0.0,
                course_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
                frequency: 0,
            },
            Candidate {
                exercise_id: Ustr::from("exercise3"),
                lesson_id: Ustr::from("lesson2"),
                course_id: Ustr::from("course1"),
                depth: 0.0,
                exercise_score: 0.0,
                lesson_score: 0.0,
                course_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
                frequency: 0,
            },
            Candidate {
                exercise_id: Ustr::from("exercise4"),
                lesson_id: Ustr::from(""),
                course_id: Ustr::from("course1"),
                depth: 0.0,
                exercise_score: 0.0,
                lesson_score: 0.0,
                course_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
                frequency: 0,
            },
        ];

        // Count the number of candidates per lesson.
        let lesson_frequency = CandidateFilter::count_lesson_frequency(&candidates);
        assert_eq!(lesson_frequency.len(), 3);
        assert_eq!(lesson_frequency.get(&Ustr::from("lesson1")), Some(&2));
        assert_eq!(lesson_frequency.get(&Ustr::from("lesson2")), Some(&1));
        assert_eq!(lesson_frequency.get(&Ustr::from("")), Some(&1));
    }

    /// Verifies the logic to select candidates in the right candidate window.
    #[test]
    fn candidates_in_window() {
        let candidates = vec![
            Candidate {
                exercise_id: Ustr::from("exercise1"),
                lesson_id: Ustr::from("lesson1"),
                course_id: Ustr::from("course1"),
                depth: 0.0,
                exercise_score: 2.1,
                lesson_score: 0.0,
                course_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
                frequency: 0,
            },
            Candidate {
                exercise_id: Ustr::from("exercise2"),
                lesson_id: Ustr::from("lesson1"),
                course_id: Ustr::from("course1"),
                depth: 0.0,
                exercise_score: 3.0,
                lesson_score: 0.0,
                course_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
                frequency: 0,
            },
            Candidate {
                exercise_id: Ustr::from("exercise3"),
                lesson_id: Ustr::from("lesson2"),
                course_id: Ustr::from("course1"),
                depth: 0.0,
                exercise_score: 3.7,
                lesson_score: 0.0,
                course_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
                frequency: 0,
            },
            Candidate {
                exercise_id: Ustr::from("exercise4"),
                lesson_id: Ustr::from(""),
                course_id: Ustr::from("course1"),
                depth: 0.0,
                exercise_score: 1.0,
                lesson_score: 0.0,
                course_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
                frequency: 0,
            },
            Candidate {
                exercise_id: Ustr::from("exercise5"),
                lesson_id: Ustr::from(""),
                course_id: Ustr::from("course1"),
                depth: 0.0,
                exercise_score: 3.5,
                lesson_score: 0.0,
                course_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
                frequency: 0,
            },
        ];
        let window_opts = MasteryWindow {
            percentage: 1.0,
            range: (2.0, 4.0),
        };
        let encompassed_set =
            UstrSet::from_iter([Ustr::from("exercise1"), Ustr::from("exercise5")]);
        let candidates_in_window =
            CandidateFilter::candidates_in_window(&candidates, &encompassed_set, &window_opts);
        assert_eq!(candidates_in_window.len(), 2);
        assert!(
            candidates_in_window
                .iter()
                .any(|c| c.exercise_id == Ustr::from("exercise2"))
        );
        assert!(
            candidates_in_window
                .iter()
                .any(|c| c.exercise_id == Ustr::from("exercise3"))
        );
    }

    /// Verifies that remainders are added to the final list of candidates when there are not enough
    /// candidates in the initial batch.
    #[test]
    fn add_remainder() {
        // Build initial data for the test.
        let batch_size = 10;
        let mut final_candidates = vec![Candidate {
            exercise_id: Ustr::from("exercise1"),
            lesson_id: Ustr::from("lesson1"),
            course_id: Ustr::from("course1"),
            depth: 0.0,
            exercise_score: 0.0,
            lesson_score: 0.0,
            course_score: 0.0,
            num_trials: 0,
            last_seen: 0.0,
            frequency: 0,
        }];
        let remainder = vec![
            Candidate {
                exercise_id: Ustr::from("exercise2"),
                lesson_id: Ustr::from("lesson2"),
                course_id: Ustr::from("course2"),
                depth: 0.0,
                exercise_score: 0.0,
                lesson_score: 0.0,
                course_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
                frequency: 0,
            },
            Candidate {
                exercise_id: Ustr::from("exercise3"),
                lesson_id: Ustr::from("lesson3"),
                course_id: Ustr::from("course3"),
                depth: 0.0,
                exercise_score: 0.0,
                lesson_score: 0.0,
                course_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
                frequency: 0,
            },
            Candidate {
                exercise_id: Ustr::from("exercise4"),
                lesson_id: Ustr::from("lesson4"),
                course_id: Ustr::from("course4"),
                depth: 0.0,
                exercise_score: 0.0,
                lesson_score: 0.0,
                course_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
                frequency: 0,
            },
        ];
        let frequency_map = UstrMap::default();

        // Verify that remainders are added when there are not enough candidates.
        let initial_len = final_candidates.len();
        CandidateFilter::add_remainder(
            batch_size,
            &mut final_candidates,
            &remainder.clone(),
            &frequency_map,
            None,
        );
        assert!(final_candidates.len() > initial_len);
        assert!(final_candidates.len() < batch_size);

        // Verify that remainders are not added when the batch is already full enough.
        let mut final_candidates_full = (0..batch_size * 2 / 3 + 1)
            .map(|i| Candidate {
                exercise_id: Ustr::from(&format!("exercise{}", i)),
                lesson_id: Ustr::from(&format!("lesson{}", i)),
                course_id: Ustr::from(&format!("course{}", i)),
                depth: 0.0,
                exercise_score: 0.0,
                lesson_score: 0.0,
                course_score: 0.0,
                num_trials: 0,
                last_seen: 0.0,
                frequency: 0,
            })
            .collect::<Vec<_>>();
        let initial_len_full = final_candidates_full.len();
        CandidateFilter::add_remainder(
            batch_size,
            &mut final_candidates_full,
            &remainder.clone(),
            &frequency_map,
            None,
        );
        assert_eq!(final_candidates_full.len(), initial_len_full);

        // Verify that max_added limits the number of remainders added.
        let mut final_candidates_limited = vec![Candidate {
            exercise_id: Ustr::from("exercise1"),
            lesson_id: Ustr::from("lesson1"),
            course_id: Ustr::from("course1"),
            depth: 0.0,
            exercise_score: 0.0,
            lesson_score: 0.0,
            course_score: 0.0,
            num_trials: 0,
            last_seen: 0.0,
            frequency: 0,
        }];
        let max_added = 1;
        CandidateFilter::add_remainder(
            batch_size,
            &mut final_candidates_limited,
            &remainder,
            &frequency_map,
            Some(max_added),
        );
        assert_eq!(final_candidates_limited.len(), 2);
    }

    /// Verifies that candidates that took more hopes to reach are given more weight.
    #[test]
    fn more_hops_more_weight() {
        let c1 = Candidate {
            exercise_id: Ustr::from("exercise1"),
            lesson_id: Ustr::from("lesson1"),
            course_id: Ustr::from("course1"),
            depth: 0.0,
            exercise_score: 0.0,
            lesson_score: 0.0,
            course_score: 0.0,
            num_trials: 0,
            last_seen: 0.0,
            frequency: 0,
        };
        let c2 = Candidate {
            exercise_id: Ustr::from("exercise2"),
            lesson_id: Ustr::from("lesson1"),
            course_id: Ustr::from("course1"),
            depth: 10.0,
            exercise_score: 0.0,
            lesson_score: 0.0,
            course_score: 0.0,
            num_trials: 0,
            last_seen: 0.0,
            frequency: 0,
        };
        assert!(
            CandidateFilter::candidate_weight(&c1, 0, 1, 1)
                < CandidateFilter::candidate_weight(&c2, 0, 1, 1)
        );
    }

    /// Verifies that candidates with a higher score are given less weight.
    #[test]
    fn higher_exercise_score_less_weight() {
        let c1 = Candidate {
            exercise_id: Ustr::from("exercise1"),
            lesson_id: Ustr::from("lesson1"),
            course_id: Ustr::from("course1"),
            depth: 0.0,
            exercise_score: 5.0,
            lesson_score: 5.0,
            course_score: 5.0,
            num_trials: 0,
            last_seen: 0.0,
            frequency: 0,
        };
        let c2 = Candidate {
            exercise_id: Ustr::from("exercise2"),
            lesson_id: Ustr::from("lesson1"),
            course_id: Ustr::from("course1"),
            depth: 0.0,
            exercise_score: 1.0,
            lesson_score: 1.0,
            course_score: 1.0,
            num_trials: 0,
            last_seen: 0.0,
            frequency: 0,
        };
        assert!(
            CandidateFilter::candidate_weight(&c1, 0, 1, 1)
                < CandidateFilter::candidate_weight(&c2, 0, 1, 1)
        );
    }

    /// Verifies that candidates with a higher lesson score are given less weight.
    #[test]
    fn higher_lesson_score_less_weight() {
        let c1 = Candidate {
            exercise_id: Ustr::from("exercise1"),
            lesson_id: Ustr::from("lesson1"),
            course_id: Ustr::from("course1"),
            depth: 0.0,
            exercise_score: 0.0,
            lesson_score: 5.0,
            course_score: 0.0,
            num_trials: 0,
            last_seen: 0.0,
            frequency: 0,
        };
        let c2 = Candidate {
            exercise_id: Ustr::from("exercise2"),
            lesson_id: Ustr::from("lesson1"),
            course_id: Ustr::from("course1"),
            depth: 0.0,
            exercise_score: 0.0,
            lesson_score: 1.0,
            course_score: 0.0,
            num_trials: 0,
            last_seen: 0.0,
            frequency: 0,
        };
        assert!(
            CandidateFilter::candidate_weight(&c1, 0, 1, 1)
                < CandidateFilter::candidate_weight(&c2, 0, 1, 1)
        );
    }

    /// Verifies that candidates with a higher course score are given less weight.
    #[test]
    fn higher_course_score_less_weight() {
        let c1 = Candidate {
            exercise_id: Ustr::from("exercise1"),
            lesson_id: Ustr::from("lesson1"),
            course_id: Ustr::from("course1"),
            depth: 0.0,
            exercise_score: 0.0,
            lesson_score: 0.0,
            course_score: 5.0,
            num_trials: 0,
            last_seen: 0.0,
            frequency: 0,
        };
        let c2 = Candidate {
            exercise_id: Ustr::from("exercise2"),
            lesson_id: Ustr::from("lesson1"),
            course_id: Ustr::from("course1"),
            depth: 0.0,
            exercise_score: 0.0,
            lesson_score: 0.0,
            course_score: 1.0,
            num_trials: 0,
            last_seen: 0.0,
            frequency: 0,
        };
        assert!(
            CandidateFilter::candidate_weight(&c1, 0, 1, 1)
                < CandidateFilter::candidate_weight(&c2, 0, 1, 1)
        );
    }

    /// Verifies that candidates that have been scheduled more often are given less weight.
    #[test]
    fn more_scheduled_frequency_less_weight() {
        let c1 = Candidate {
            exercise_id: Ustr::from("exercise1"),
            lesson_id: Ustr::from("lesson1"),
            course_id: Ustr::from("course1"),
            depth: 0.0,
            exercise_score: 0.0,
            lesson_score: 0.0,
            course_score: 0.0,
            num_trials: 0,
            last_seen: 0.0,
            frequency: 5,
        };
        let c2 = Candidate {
            exercise_id: Ustr::from("exercise2"),
            lesson_id: Ustr::from("lesson1"),
            course_id: Ustr::from("course1"),
            depth: 0.0,
            exercise_score: 0.0,
            lesson_score: 0.0,
            course_score: 0.0,
            num_trials: 0,
            last_seen: 0.0,
            frequency: 1,
        };
        assert!(
            CandidateFilter::candidate_weight(&c1, 0, 1, 1)
                < CandidateFilter::candidate_weight(&c2, 0, 1, 1)
        );
    }

    /// Verifies that candidates with fewer trials are given more weight.
    #[test]
    fn fewer_trials_more_weight() {
        let c1 = Candidate {
            exercise_id: Ustr::from("exercise1"),
            lesson_id: Ustr::from("lesson1"),
            course_id: Ustr::from("course1"),
            depth: 0.0,
            exercise_score: 0.0,
            lesson_score: 0.0,
            course_score: 0.0,
            num_trials: 5,
            last_seen: 0.0,
            frequency: 0,
        };
        let c2 = Candidate {
            exercise_id: Ustr::from("exercise2"),
            lesson_id: Ustr::from("lesson1"),
            course_id: Ustr::from("course1"),
            depth: 0.0,
            exercise_score: 0.0,
            lesson_score: 0.0,
            course_score: 0.0,
            num_trials: 1,
            last_seen: 0.0,
            frequency: 0,
        };
        assert!(
            CandidateFilter::candidate_weight(&c1, 0, 1, 1)
                < CandidateFilter::candidate_weight(&c2, 0, 1, 1)
        );
    }

    /// Verifies that candidates seen less recently are given more weight.
    #[test]
    fn more_days_since_last_seen_more_weight() {
        let c1 = Candidate {
            exercise_id: Ustr::from("exercise1"),
            lesson_id: Ustr::from("lesson1"),
            course_id: Ustr::from("course1"),
            depth: 0.0,
            exercise_score: 0.0,
            lesson_score: 0.0,
            course_score: 0.0,
            num_trials: 0,
            last_seen: 1.0,
            frequency: 0,
        };
        let c2 = Candidate {
            exercise_id: Ustr::from("exercise2"),
            lesson_id: Ustr::from("lesson2"),
            course_id: Ustr::from("course2"),
            depth: 0.0,
            exercise_score: 0.0,
            lesson_score: 0.0,
            course_score: 0.0,
            num_trials: 0,
            last_seen: 20.0,
            frequency: 0,
        };
        assert!(
            CandidateFilter::candidate_weight(&c1, 0, 1, 1)
                < CandidateFilter::candidate_weight(&c2, 0, 1, 1)
        );
    }

    /// Verifies that candidates from lessons with more candidates are given less weight.
    #[test]
    fn higher_lesson_frequency_less_weight() {
        let c1 = Candidate {
            exercise_id: Ustr::from("exercise1"),
            lesson_id: Ustr::from("lesson1"),
            course_id: Ustr::from("course1"),
            depth: 0.0,
            exercise_score: 0.0,
            lesson_score: 0.0,
            course_score: 0.0,
            num_trials: 0,
            last_seen: 0.0,
            frequency: 0,
        };
        let c2 = Candidate {
            exercise_id: Ustr::from("exercise2"),
            lesson_id: Ustr::from("lesson2"),
            course_id: Ustr::from("course1"),
            depth: 0.0,
            exercise_score: 0.0,
            lesson_score: 0.0,
            course_score: 0.0,
            num_trials: 0,
            last_seen: 0.0,
            frequency: 0,
        };
        assert!(
            CandidateFilter::candidate_weight(&c1, 0, 10, 1)
                < CandidateFilter::candidate_weight(&c2, 0, 3, 1)
        );
    }

    /// Verifies that candidates from courses with more candidates are given less weight.
    #[test]
    fn higher_course_frequency_less_weight() {
        let c1 = Candidate {
            exercise_id: Ustr::from("exercise1"),
            lesson_id: Ustr::from("lesson1"),
            course_id: Ustr::from("course1"),
            depth: 0.0,
            exercise_score: 0.0,
            lesson_score: 0.0,
            course_score: 0.0,
            num_trials: 0,
            last_seen: 0.0,
            frequency: 0,
        };
        let c2 = Candidate {
            exercise_id: Ustr::from("exercise2"),
            lesson_id: Ustr::from("lesson2"),
            course_id: Ustr::from("course2"),
            depth: 0.0,
            exercise_score: 0.0,
            lesson_score: 0.0,
            course_score: 0.0,
            num_trials: 0,
            last_seen: 0.0,
            frequency: 0,
        };
        assert!(
            CandidateFilter::candidate_weight(&c1, 0, 1, 10)
                < CandidateFilter::candidate_weight(&c2, 0, 1, 3)
        );
    }

    /// Verifies that candidates that are encompassed by more exercises in the initial batch are given
    /// less weight.
    #[test]
    fn higher_encompassed_frequency_less_weight() {
        let c1 = Candidate {
            exercise_id: Ustr::from("exercise1"),
            lesson_id: Ustr::from("lesson1"),
            course_id: Ustr::from("course1"),
            depth: 0.0,
            exercise_score: 0.0,
            lesson_score: 0.0,
            course_score: 0.0,
            num_trials: 0,
            last_seen: 0.0,
            frequency: 0,
        };
        let c2 = Candidate {
            exercise_id: Ustr::from("exercise2"),
            lesson_id: Ustr::from("lesson1"),
            course_id: Ustr::from("course1"),
            depth: 0.0,
            exercise_score: 0.0,
            lesson_score: 0.0,
            course_score: 0.0,
            num_trials: 0,
            last_seen: 0.0,
            frequency: 0,
        };
        assert!(
            CandidateFilter::candidate_weight(&c1, 10, 1, 1)
                < CandidateFilter::candidate_weight(&c2, 3, 1, 1)
        );
    }

    /// Verifies that the minimum weight is applied to candidates.
    #[test]
    fn minimum_weight() {
        // Create a candidate that should have a very low weight.
        let c = Candidate {
            exercise_id: Ustr::from("exercise1"),
            lesson_id: Ustr::from("lesson1"),
            course_id: Ustr::from("course1"),
            depth: 0.0,
            exercise_score: 5.0,
            lesson_score: 5.0,
            course_score: 5.0,
            num_trials: 1000,
            last_seen: 0.0,
            frequency: 1000,
        };
        assert_eq!(
            CandidateFilter::candidate_weight(&c, 100, 1000, 1000),
            MIN_WEIGHT
        );
    }
}
