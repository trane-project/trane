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

use rand::{rng, seq::IndexedRandom};
use ustr::{UstrMap, UstrSet};

use crate::{
    data::{MasteryWindow, SchedulerOptions},
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
const DEPTH_WEIGHT_FACTOR: f32 = 500.0;

/// The part of the weight that depends on the number of dependents of the lesson and course of the
/// candidate will be the product of the number of dependents and this factor.
const NUM_DEPENDENTS_WEIGHT_FACTOR: f32 = 250.0;

/// The part of the weight that depends on whether the candidate was found at a dead-end in the
/// graph.
const DEAD_WEIGHT_FACTOR: f32 = 1000.0;

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

/// The factor used to compute the part of the weight that depends on the number of days since this
/// exercise was last seen.
const LAST_SEEN_WEIGHT_PER_DAY: f32 = 5.0;

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

/// The factor used to multiply the absolute value of the velocity to compute its contribution to
/// the weight.
const VELOCITY_WEIGHT_FACTOR: f32 = 250.0;

/// The part of the weight added to non-mastered candidates with a stagnant velocity.
const STAGNANT_VELOCITY_WEIGHT: f32 = 2000.0;

/// The part of the weight substracted to mastered candidates with a stagnant velocity.
const STAGNANT_VELOCITY_PENALTY: f32 = -2000.0;

/// The velocity threshold under which a candidate is considered to be stagnant.
const STAGNANT_VELOCITY_THRESHOLD: f32 = 0.2;

/// The exercise score threshold above which a candidate is considered mastered for the purpose of
/// applying the stagnant velocity bonus or penalty.
const MASTERED_SCORE_THRESHOLD: f32 = 4.0;

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
    /// 6. The number of dependents of the lesson and course of the candidate. Exercises from
    ///    lessons and courses that unlock more units are given higher weight.
    /// 7. The frequency with which the candidate has been scheduled during the run of the
    ///    scheduler. A higher frequency is assigned less weight to avoid selecting the same
    ///    exercises too often during the same session.
    /// 8. The number of trials for that candidate. A higher number of trials is assigned less
    ///    weight to favor exercises that have been practiced fewer times.
    /// 9. The number of days since this candidate was last seen. More days since last seen gets
    ///    more weight.
    /// 10. The number of candidates in the same lesson. The more candidates there are in the same
    ///     lesson, the less weight each candidate is assigned to avoid selecting too many exercises
    ///     from the same lesson.
    /// 11. The number of candidates in the same course. The same logic applies as for the lesson
    ///     frequency.
    /// 12. Whether the candidate comes from a dead-end in the traversal. Dead-end candidates get a
    ///     fixed bonus to prioritize the learner's frontier.
    /// 13. The candidate's score velocity. The absolute value of the velocity is multiplied by a
    ///     factor.
    /// 14. Whether the candidate has a stagnant velocity. Non-mastered candidates with a stagnant
    ///     velocity get a weight bonus, while mastered candidates with a stagnant velocity get a
    ///     penalty.
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

        // A part of the score will depend on the frequency with which the exercise is encompassed
        // by other exercises in the initial batch.
        weight += MAX_ENCOMPASSED_WEIGHT / (encompassed_freq.max(1) as f32);

        // A part of the score will depend on the number of hops that were needed to reach
        // the candidate.
        weight += DEPTH_WEIGHT_FACTOR * c.depth.ln_1p();

        // A part of the weight is based on the number of dependents of the lesson and course of the
        // candidate.
        weight += NUM_DEPENDENTS_WEIGHT_FACTOR * (c.num_dependents as f32).ln_1p();

        // A part of the weight is based on the frequency with which the exercise has been
        // scheduled.
        weight += MAX_SCHEDULED_WEIGHT * SCHEDULED_FACTOR.powf(c.frequency as f32);

        // A part of the weight is based on the number of trials for that exercise.
        weight += MAX_NUM_TRIALS_WEIGHT * NUM_TRIALS_FACTOR.powf(c.num_trials as f32);

        // A part of the weight is based on the number of days since this exercise was last seen.
        // The computation includes a factor based on the score so that exercises with lower score
        // are given higher weight.
        weight += (LAST_SEEN_WEIGHT_PER_DAY * c.last_seen * (5.0 - c.exercise_score))
            .clamp(0.0, MAX_LAST_SEEN_WEIGHT);

        // A part of the weight is based on the number of candidates in the same lesson.
        weight += MAX_LESSON_FREQUENCY_WEIGHT / lesson_freq.max(1) as f32;

        // A part of the weight is based on the number of candidates in the same course.
        weight += MAX_COURSE_FREQUENCY_WEIGHT / course_freq.max(1) as f32;

        // A fixed part of the score depends on whether the candidate is at a dead-end.
        if c.dead_end {
            weight += DEAD_WEIGHT_FACTOR;
        }

        // A part of the weight is based on the candidate's score velocity. All exercises get a
        // boost based on the absolute value of the velocity. Stagnant exercises get a boost or a
        // penalty depending on whether they are mastered.
        if let Some(velocity) = c.score_velocity {
            weight += VELOCITY_WEIGHT_FACTOR * velocity.abs();
            if velocity.abs() < STAGNANT_VELOCITY_THRESHOLD {
                if c.exercise_score >= MASTERED_SCORE_THRESHOLD {
                    weight += STAGNANT_VELOCITY_PENALTY;
                } else {
                    weight += STAGNANT_VELOCITY_WEIGHT;
                }
            }
        }

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
            .sample_weighted(&mut rng, num_to_select, |c| {
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
        let (remainder_candidates, _) =
            Self::select_candidates(remainder, frequency_map, num_added);
        final_candidates.extend(remainder_candidates);
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

    /// Takes the base scheduler options and updates the mastery windows percentages based on the
    /// success rate of the session.
    fn adjusted_mastery_windows(options: &SchedulerOptions, success_rate: f32) -> SchedulerOptions {
        let mut adjusted_options = options.clone();

        // The optimal zone is a success rate between 75% and 90%. No adjustment is needed if
        // the success rate is in this range.
        let shift = if success_rate > 0.90 {
            0.05_f32
        } else if (0.75..=0.90).contains(&success_rate) {
            return adjusted_options;
        } else if (0.50..0.75).contains(&success_rate) {
            -0.05_f32
        } else {
            // success_rate < 0.50
            -0.10_f32
        };

        // Shift harder and easier window percentages in opposite directions. Clamp each percentage
        // to [0.05, 0.50] to keep all windows represented.
        let clamp = |p: f32| p.clamp(0.05, 0.50);
        adjusted_options.new_window_opts.percentage =
            clamp(options.new_window_opts.percentage + shift);
        adjusted_options.target_window_opts.percentage =
            clamp(options.target_window_opts.percentage + shift);
        adjusted_options.easy_window_opts.percentage =
            clamp(options.easy_window_opts.percentage - shift);
        adjusted_options.mastered_window_opts.percentage =
            clamp(options.mastered_window_opts.percentage - shift);

        // Normalize so all five windows still sum to 1.0. The current window absorbs the rounding
        // difference since it represents the mid-difficulty sweet spot.
        let sum = adjusted_options.new_window_opts.percentage
            + adjusted_options.target_window_opts.percentage
            + adjusted_options.easy_window_opts.percentage
            + adjusted_options.mastered_window_opts.percentage;
        adjusted_options.current_window_opts.percentage = (1.0_f32 - sum).max(0.05);

        adjusted_options
    }

    /// Takes a list of exercises and filters them so that the end result is a list of exercise
    /// manifests which fit the mastery windows defined in the scheduler options.
    pub fn filter_candidates(&self, result: KnockoutResult) -> Vec<Candidate> {
        // Find the batch size to use.
        let candidates = &result.candidates;
        let options =
            Self::adjusted_mastery_windows(&self.data.options, self.data.get_success_rate());
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
        let mut mastered_candidates =
            Self::candidates_in_window(candidates, &encompassed_set, &options.mastered_window_opts);
        let easy_candidates =
            Self::candidates_in_window(candidates, &encompassed_set, &options.easy_window_opts);
        let current_candidates =
            Self::candidates_in_window(candidates, &encompassed_set, &options.current_window_opts);
        let target_candidates =
            Self::candidates_in_window(candidates, &encompassed_set, &options.target_window_opts);
        let new_candidates =
            Self::candidates_in_window(candidates, &encompassed_set, &options.new_window_opts);
        mastered_candidates.extend(result.highly_encompassed);

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
        // mastered windows, in that order. Limit the number hard exercises to avoid creating very
        // difficult batches.
        let base_remainder = (batch_size / 10).max(1);
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
            None,
        );
        Self::add_remainder(
            batch_size,
            &mut final_candidates,
            &mastered_remainder,
            frequency_map,
            None,
        );
        final_candidates
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
                lesson_id: Ustr::from("lesson1"),
                ..Default::default()
            },
            Candidate {
                lesson_id: Ustr::from("lesson1"),
                ..Default::default()
            },
            Candidate {
                lesson_id: Ustr::from("lesson2"),
                ..Default::default()
            },
            Candidate::default(),
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
                exercise_score: 2.1,
                ..Default::default()
            },
            Candidate {
                exercise_id: Ustr::from("exercise2"),
                exercise_score: 3.0,
                ..Default::default()
            },
            Candidate {
                exercise_id: Ustr::from("exercise3"),
                exercise_score: 3.7,
                ..Default::default()
            },
            Candidate {
                exercise_id: Ustr::from("exercise4"),
                exercise_score: 1.0,
                ..Default::default()
            },
            Candidate {
                exercise_id: Ustr::from("exercise5"),
                exercise_score: 3.5,
                ..Default::default()
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
            ..Default::default()
        }];
        let remainder = vec![
            Candidate {
                exercise_id: Ustr::from("exercise2"),
                ..Default::default()
            },
            Candidate {
                exercise_id: Ustr::from("exercise3"),
                ..Default::default()
            },
            Candidate {
                exercise_id: Ustr::from("exercise4"),
                ..Default::default()
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
                ..Default::default()
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
            ..Default::default()
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

    /// Verifies that candidates that took more hops to reach are given more weight.
    #[test]
    fn more_hops_more_weight() {
        let c1 = Candidate::default();
        let c2 = Candidate {
            depth: 10.0,
            ..Default::default()
        };
        assert!(
            CandidateFilter::candidate_weight(&c1, 0, 1, 1)
                < CandidateFilter::candidate_weight(&c2, 0, 1, 1)
        );
    }

    /// Verifies that candidates with more dependents are given more weight.
    #[test]
    fn more_dependents_more_weight() {
        let c1 = Candidate::default();
        let c2 = Candidate {
            num_dependents: 50,
            ..Default::default()
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
            exercise_score: 5.0,
            lesson_score: 5.0,
            course_score: 5.0,
            ..Default::default()
        };
        let c2 = Candidate {
            exercise_score: 1.0,
            lesson_score: 1.0,
            course_score: 1.0,
            ..Default::default()
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
            lesson_score: 5.0,
            ..Default::default()
        };
        let c2 = Candidate {
            lesson_score: 1.0,
            ..Default::default()
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
            course_score: 5.0,
            ..Default::default()
        };
        let c2 = Candidate {
            course_score: 1.0,
            ..Default::default()
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
            frequency: 5,
            ..Default::default()
        };
        let c2 = Candidate {
            frequency: 1,
            ..Default::default()
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
            num_trials: 5,
            ..Default::default()
        };
        let c2 = Candidate {
            num_trials: 1,
            ..Default::default()
        };
        assert!(
            CandidateFilter::candidate_weight(&c1, 0, 1, 1)
                < CandidateFilter::candidate_weight(&c2, 0, 1, 1)
        );
    }

    /// Verifies that candidates seen less recently are given more weight.
    #[test]
    fn more_days_since_last_seen_more_weight() {
        // Candidates with the same exercise score but different last seen values.
        let c1 = Candidate {
            last_seen: 1.0,
            exercise_score: 4.0,
            ..Default::default()
        };
        let c2 = Candidate {
            last_seen: 20.0,
            exercise_score: 2.0,
            ..Default::default()
        };
        assert!(
            CandidateFilter::candidate_weight(&c1, 0, 1, 1)
                < CandidateFilter::candidate_weight(&c2, 0, 1, 1)
        );

        // Candidates with different exercise scores but the same last seen values.
        let c3 = Candidate {
            last_seen: 10.0,
            exercise_score: 4.0,
            ..Default::default()
        };
        let c4 = Candidate {
            last_seen: 10.0,
            exercise_score: 2.0,
            ..Default::default()
        };
        assert!(
            CandidateFilter::candidate_weight(&c3, 0, 1, 1)
                < CandidateFilter::candidate_weight(&c4, 0, 1, 1)
        );
    }

    /// Verifies that candidates from lessons with more candidates are given less weight.
    #[test]
    fn higher_lesson_frequency_less_weight() {
        let c = Candidate::default();
        assert!(
            CandidateFilter::candidate_weight(&c, 0, 10, 1)
                < CandidateFilter::candidate_weight(&c, 0, 3, 1)
        );
    }

    /// Verifies that the mastery windows are adjusted based on the success rate.
    #[test]
    fn adjusted_mastery_windows() {
        // In the optimal zone (75%-90%), windows are unchanged.
        let options = SchedulerOptions::default();
        let adjusted = CandidateFilter::adjusted_mastery_windows(&options, 0.85);
        assert_eq!(
            adjusted.new_window_opts.percentage,
            options.new_window_opts.percentage
        );
        assert_eq!(
            adjusted.target_window_opts.percentage,
            options.target_window_opts.percentage
        );
        assert_eq!(
            adjusted.current_window_opts.percentage,
            options.current_window_opts.percentage
        );
        assert_eq!(
            adjusted.easy_window_opts.percentage,
            options.easy_window_opts.percentage
        );
        assert_eq!(
            adjusted.mastered_window_opts.percentage,
            options.mastered_window_opts.percentage
        );

        // At the boundaries of the optimal zone, windows are also unchanged.
        let adjusted_low = CandidateFilter::adjusted_mastery_windows(&options, 0.75);
        assert_eq!(
            adjusted_low.new_window_opts.percentage,
            options.new_window_opts.percentage
        );
        let adjusted_high = CandidateFilter::adjusted_mastery_windows(&options, 0.90);
        assert_eq!(
            adjusted_high.new_window_opts.percentage,
            options.new_window_opts.percentage
        );

        // Success rate > 90%: too easy, shift toward harder windows.
        let adjusted = CandidateFilter::adjusted_mastery_windows(&options, 0.95);
        assert!(adjusted.new_window_opts.percentage > options.new_window_opts.percentage);
        assert!(adjusted.target_window_opts.percentage > options.target_window_opts.percentage);
        assert!(adjusted.easy_window_opts.percentage < options.easy_window_opts.percentage);
        assert!(adjusted.mastered_window_opts.percentage < options.mastered_window_opts.percentage);

        // Success rate 50%-75%: too hard, shift toward easier windows.
        let adjusted = CandidateFilter::adjusted_mastery_windows(&options, 0.60);
        assert!(adjusted.new_window_opts.percentage < options.new_window_opts.percentage);
        assert!(adjusted.target_window_opts.percentage < options.target_window_opts.percentage);
        assert!(adjusted.easy_window_opts.percentage > options.easy_window_opts.percentage);
        assert!(adjusted.mastered_window_opts.percentage > options.mastered_window_opts.percentage);

        // Success rate < 50%: very hard, shift even more toward easier windows.
        let adjusted_very_hard = CandidateFilter::adjusted_mastery_windows(&options, 0.30);
        let adjusted_hard = CandidateFilter::adjusted_mastery_windows(&options, 0.60);
        assert!(
            adjusted_very_hard.easy_window_opts.percentage
                > adjusted_hard.easy_window_opts.percentage
        );
        assert!(
            adjusted_very_hard.mastered_window_opts.percentage
                > adjusted_hard.mastered_window_opts.percentage
        );
        assert!(
            adjusted_very_hard.new_window_opts.percentage
                < adjusted_hard.new_window_opts.percentage
        );
        assert!(
            adjusted_very_hard.target_window_opts.percentage
                < adjusted_hard.target_window_opts.percentage
        );

        // All five windows always sum to 1.0.
        for rate in [0.0, 0.30, 0.60, 0.80, 0.95, 1.0] {
            let adj = CandidateFilter::adjusted_mastery_windows(&options, rate);
            let sum = adj.new_window_opts.percentage
                + adj.target_window_opts.percentage
                + adj.current_window_opts.percentage
                + adj.easy_window_opts.percentage
                + adj.mastered_window_opts.percentage;
            assert!((sum - 1.0).abs() < 1e-6);
        }
    }

    /// Verifies that candidates from courses with more candidates are given less weight.
    #[test]
    fn higher_course_frequency_less_weight() {
        let c = Candidate::default();
        assert!(
            CandidateFilter::candidate_weight(&c, 0, 1, 10)
                < CandidateFilter::candidate_weight(&c, 0, 1, 3)
        );
    }

    /// Verifies that candidates that are encompassed by more exercises in the initial batch are given
    /// less weight.
    #[test]
    fn higher_encompassed_frequency_less_weight() {
        let c = Candidate::default();
        assert!(
            CandidateFilter::candidate_weight(&c, 10, 1, 1)
                < CandidateFilter::candidate_weight(&c, 3, 1, 1)
        );
    }

    /// Verifies that dead-end candidates get a fixed additional weight.
    #[test]
    fn dead_end_fixed_weight_bonus() {
        let base = Candidate::default();
        let dead_end = Candidate {
            dead_end: true,
            ..Default::default()
        };

        let base_weight = CandidateFilter::candidate_weight(&base, 0, 1, 1);
        let dead_end_weight = CandidateFilter::candidate_weight(&dead_end, 0, 1, 1);
        assert_eq!(dead_end_weight - base_weight, DEAD_WEIGHT_FACTOR);
    }

    /// Verifies that the minimum weight is applied to candidates.
    #[test]
    fn minimum_weight() {
        // Create a candidate that should have a very low weight.
        let c = Candidate {
            exercise_score: 5.0,
            lesson_score: 5.0,
            course_score: 5.0,
            num_trials: 1000,
            frequency: 1000,
            ..Default::default()
        };
        assert_eq!(
            CandidateFilter::candidate_weight(&c, 100, 1000, 1000),
            MIN_WEIGHT
        );
    }

    /// Verifies that candidates with higher absolute velocity get more weight.
    #[test]
    fn higher_velocity_more_weight() {
        let base = Candidate {
            exercise_score: 2.0,
            score_velocity: Some(1.0),
            ..Default::default()
        };
        let low_velocity = Candidate {
            score_velocity: Some(0.5),
            ..base.clone()
        };
        assert!(
            CandidateFilter::candidate_weight(&base, 0, 1, 1)
                > CandidateFilter::candidate_weight(&low_velocity, 0, 1, 1)
        );
    }

    /// Verifies that negative velocity also boosts weight via the absolute value.
    #[test]
    fn negative_velocity_boosts_weight() {
        let base = Candidate {
            exercise_score: 2.0,
            ..Default::default()
        };
        let negative = Candidate {
            score_velocity: Some(-1.0),
            ..base.clone()
        };
        assert!(
            CandidateFilter::candidate_weight(&negative, 0, 1, 1)
                > CandidateFilter::candidate_weight(&base, 0, 1, 1)
        );
    }

    /// Verifies that stagnant non-mastered exercises get a weight bonus.
    #[test]
    fn stagnant_low_score_gets_bonus() {
        let base = Candidate {
            exercise_score: 2.0,
            ..Default::default()
        };
        let stagnant = Candidate {
            score_velocity: Some(0.05),
            ..base.clone()
        };
        let base_weight = CandidateFilter::candidate_weight(&base, 0, 1, 1);
        let stagnant_weight = CandidateFilter::candidate_weight(&stagnant, 0, 1, 1);
        assert!(stagnant_weight > base_weight + STAGNANT_VELOCITY_WEIGHT - 100.0);
    }

    /// Verifies that stagnant mastered exercises get a weight penalty.
    #[test]
    fn stagnant_high_score_gets_penalty() {
        let base = Candidate {
            exercise_score: 4.5,
            ..Default::default()
        };
        let stagnant = Candidate {
            score_velocity: Some(0.05),
            ..base.clone()
        };
        assert!(
            CandidateFilter::candidate_weight(&stagnant, 0, 1, 1)
                < CandidateFilter::candidate_weight(&base, 0, 1, 1)
        );
    }

    /// Verifies that velocity above the stagnation threshold does not trigger the stagnation
    /// bonus or penalty.
    #[test]
    fn non_stagnant_velocity_no_bonus_or_penalty() {
        let base = Candidate {
            exercise_score: 2.0,
            ..Default::default()
        };
        let active = Candidate {
            score_velocity: Some(0.5),
            ..base.clone()
        };
        let base_weight = CandidateFilter::candidate_weight(&base, 0, 1, 1);
        let active_weight = CandidateFilter::candidate_weight(&active, 0, 1, 1);
        let expected_diff = VELOCITY_WEIGHT_FACTOR * 0.5;
        assert!((active_weight - base_weight - expected_diff).abs() < 1.0);
    }
}
