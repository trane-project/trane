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
use ustr::UstrSet;

use crate::{
    data::{MasteryWindow, SchedulerOptions},
    scheduler::{Candidate, SchedulerData, review_knocker::KnockoutResult},
};

/// The minimum candidate weight.
const MIN_CANDIDATE_WEIGHT: f32 = 0.05;

/// The minimum candidate cost.
const MIN_CANDIDATE_COST: f32 = 0.05;

/// The maximum candidate cost.
const MAX_CANDIDATE_COST: f32 = 100.0;

/// Coefficient applied to the depth term in the candidate cost.
const DEPTH_COST_COEFFICIENT: f32 = 0.7;

/// Coefficient applied to the coverage term in the candidate cost.
const ENCOMPASSES_COST_COEFFICIENT: f32 = 0.9;

/// Coefficient applied to the redundancy term in the candidate cost.
const ENCOMPASSED_COST_COEFFICIENT: f32 = 0.6;

/// Coefficient applied to the scheduled frequency term in the candidate cost.
const SCHEDULED_FREQUENCY_COST_COEFFICIENT: f32 = 2.0;

/// Reduction applied to the log-cost of dead-end candidates.
const DEAD_END_COST_BONUS: f32 = 1.0;

/// Coefficient applied to the signed velocity term in the candidate cost.
const VELOCITY_COST_COEFFICIENT: f32 = 0.5;

/// The velocity threshold under which a candidate is considered to be stagnant.
const STAGNANT_VELOCITY_THRESHOLD: f32 = 0.2;

/// The exercise score threshold above which a candidate is considered mastered for the purpose of
/// applying the stagnant velocity bonus or penalty.
const MASTERED_SCORE_THRESHOLD: f32 = 4.0;

/// Reduction applied to the log-cost of non-mastered candidates with stagnant velocity.
const STAGNANT_UNMASTERED_COST_BONUS: f32 = 0.7;

/// Penalty applied to the log-cost of mastered candidates with stagnant velocity.
const STAGNANT_MASTERED_COST_PENALTY: f32 = 1.0;

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

    /// Computes the cost assigned to a candidate that will be used to select it during the
    /// filtering phase. The cost is built in log-space so that individual factors can be summed,
    /// then converted back to linear space by exponentiation. Lower cost means a candidate should
    /// be selected more often.
    ///
    /// 1. Greater depth lowers the cost.
    /// 2. Higher coverage lowers the cost.
    /// 3. Being encompassed by other candidates raises the cost.
    /// 4. More repeated scheduling raises the cost.
    /// 5. Dead-end candidates get a cost reduction.
    /// 6. Higher positive velocity lowers the cost slightly, while negative velocity raises it.
    /// 7. Stagnant non-mastered candidates get a cost reduction.
    /// 8. Stagnant mastered candidates get a cost penalty.
    fn candidate_cost(c: &Candidate) -> f32 {
        let mut log_cost = 0.0;
        log_cost -= DEPTH_COST_COEFFICIENT * c.depth.ln_1p();
        log_cost -= ENCOMPASSES_COST_COEFFICIENT * c.encompasses_weight.ln_1p();
        log_cost += ENCOMPASSED_COST_COEFFICIENT * c.encompassed_weight.ln_1p();
        log_cost += SCHEDULED_FREQUENCY_COST_COEFFICIENT * (c.frequency as f32).ln_1p();

        if c.dead_end {
            log_cost -= DEAD_END_COST_BONUS;
        }

        if let Some(velocity) = c.velocity {
            log_cost += VELOCITY_COST_COEFFICIENT * -velocity.ln_1p();
            if velocity.abs() < STAGNANT_VELOCITY_THRESHOLD {
                if c.exercise_score >= MASTERED_SCORE_THRESHOLD {
                    log_cost += STAGNANT_MASTERED_COST_PENALTY;
                } else {
                    log_cost -= STAGNANT_UNMASTERED_COST_BONUS;
                }
            }
        }

        log_cost.exp().clamp(MIN_CANDIDATE_COST, MAX_CANDIDATE_COST)
    }

    /// Computes the weight assigned to a candidate that will be used to select it during the
    /// filtering phase. The weight is derived from the formula `urgency / sqrt(cost)`, where the
    /// urgency represents how important it is to schedule the exercise, and the cost represents how
    /// "expensive" it is to schedule the exercise.
    fn candidate_weight(c: &Candidate) -> f32 {
        let cost = Self::candidate_cost(c);
        (c.urgency / cost.sqrt()).max(MIN_CANDIDATE_WEIGHT)
    }

    /// Takes a list of candidates and randomly selects `num_to_select` candidates among them. Each
    /// candidate is given a weight based on a number of factors meant to favor candidates that are
    /// optimal for practice. The function returns a tuple of the selected candidates and the
    /// remainder exercises. The remainder will be used to fill the batch in case there is space
    /// left after the first round of filtering.
    fn select_candidates(
        candidates: &[Candidate],
        num_to_select: usize,
    ) -> (Vec<Candidate>, Vec<Candidate>) {
        // Return the list if there are fewer candidates than the number to select.
        if candidates.len() <= num_to_select {
            return (candidates.to_vec(), vec![]);
        }

        // Otherwise, assign a weight to each candidate and perform a weighted random selection.
        // Safe to unwrap the result, as this function panics if `num_to_select` is greater than the
        // size of `candidates`, but that is checked above.
        let mut rng = rng();
        let selected: Vec<Candidate> = candidates
            .sample_weighted(&mut rng, num_to_select, Self::candidate_weight)
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
        let (mastered_selected, mastered_remainder) =
            Self::select_candidates(&mastered_candidates, num_mastered);
        final_candidates.extend(mastered_selected);

        // Add elements from the easy window.
        let num_easy = (batch_size_float * options.easy_window_opts.percentage).max(1.0) as usize;
        let (easy_selected, easy_remainder) = Self::select_candidates(&easy_candidates, num_easy);
        final_candidates.extend(easy_selected);

        // Add elements from the current window.
        let num_current =
            (batch_size_float * options.current_window_opts.percentage).max(1.0) as usize;
        let (current_selected, current_remainder) =
            Self::select_candidates(&current_candidates, num_current);
        final_candidates.extend(current_selected);

        // Add elements from the target window.
        let num_target =
            (batch_size_float * options.target_window_opts.percentage).max(1.0) as usize;
        let (target_selected, target_remainder) =
            Self::select_candidates(&target_candidates, num_target);
        final_candidates.extend(target_selected);

        // Add elements from the new window.
        let num_new = (batch_size_float * options.new_window_opts.percentage).max(1.0) as usize;
        let (new_selected, new_remainder) = Self::select_candidates(&new_candidates, num_new);
        final_candidates.extend(new_selected);

        // Go through the remainders and add them to the list of final candidates if there's still
        // space left in the batch. Add the remainder from the current, new, target, easy, and
        // mastered windows, in that order. Limit the number hard exercises to avoid creating very
        // difficult batches.
        let base_remainder = (batch_size / 10).max(1);
        Self::add_remainder(batch_size, &mut final_candidates, &current_remainder, None);
        Self::add_remainder(
            batch_size,
            &mut final_candidates,
            &new_remainder,
            Some(5 * base_remainder),
        );
        Self::add_remainder(
            batch_size,
            &mut final_candidates,
            &target_remainder,
            Some(3 * base_remainder),
        );
        Self::add_remainder(batch_size, &mut final_candidates, &easy_remainder, None);
        Self::add_remainder(batch_size, &mut final_candidates, &mastered_remainder, None);
        final_candidates
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use ustr::Ustr;

    use super::*;
    use crate::scheduler::Candidate;

    /// Creates a candidate with default values and unit urgency.
    fn weighted_candidate() -> Candidate {
        Candidate {
            urgency: 1.0,
            ..Default::default()
        }
    }

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
                urgency: 1.0,
                ..Default::default()
            },
            Candidate {
                exercise_id: Ustr::from("exercise3"),
                urgency: 1.0,
                ..Default::default()
            },
            Candidate {
                exercise_id: Ustr::from("exercise4"),
                urgency: 1.0,
                ..Default::default()
            },
        ];

        // Verify that remainders are added when there are not enough candidates.
        let initial_len = final_candidates.len();
        CandidateFilter::add_remainder(batch_size, &mut final_candidates, &remainder.clone(), None);
        assert!(final_candidates.len() > initial_len);
        assert!(final_candidates.len() < batch_size);

        // Verify that remainders are not added when the batch is already full enough.
        let mut final_candidates_full = (0..=(batch_size * 3 / 4))
            .map(|i| Candidate {
                exercise_id: Ustr::from(&format!("exercise{i}")),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let initial_len_full = final_candidates_full.len();
        CandidateFilter::add_remainder(
            batch_size,
            &mut final_candidates_full,
            &remainder.clone(),
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
            Some(max_added),
        );
        assert_eq!(final_candidates_limited.len(), 2);
    }

    /// Verifies that candidates that took more hops to reach are given more weight.
    #[test]
    fn more_hops_more_weight() {
        let c1 = weighted_candidate();
        let c2 = Candidate {
            depth: 10.0,
            ..weighted_candidate()
        };
        assert!(CandidateFilter::candidate_weight(&c1) < CandidateFilter::candidate_weight(&c2));
    }

    /// Verifies that candidates with higher urgency are given more weight.
    #[test]
    fn higher_urgency_more_weight() {
        let c1 = Candidate {
            urgency: 1.0,
            ..Default::default()
        };
        let c2 = Candidate {
            urgency: 0.25,
            ..Default::default()
        };
        assert!(CandidateFilter::candidate_weight(&c1) > CandidateFilter::candidate_weight(&c2));
    }

    /// Verifies that candidates that have been scheduled more often are given less weight.
    #[test]
    fn more_scheduled_frequency_less_weight() {
        let c1 = Candidate {
            frequency: 5,
            ..weighted_candidate()
        };
        let c2 = Candidate {
            frequency: 1,
            ..weighted_candidate()
        };
        assert!(CandidateFilter::candidate_weight(&c1) < CandidateFilter::candidate_weight(&c2));
    }

    /// Verifies that candidates that are encompassed by more exercises in the initial batch are
    /// given less weight.
    #[test]
    fn higher_encompassed_weight_less_weight() {
        let c1 = Candidate {
            encompassed_weight: 10.0,
            ..weighted_candidate()
        };
        let c2 = Candidate {
            encompassed_weight: 3.0,
            ..weighted_candidate()
        };
        assert!(CandidateFilter::candidate_weight(&c1) < CandidateFilter::candidate_weight(&c2));
    }

    /// Verifies that candidates that encompass more units are given more weight.
    #[test]
    fn higher_encompasses_weight_more_weight() {
        let c1 = Candidate {
            encompasses_weight: 10.0,
            ..weighted_candidate()
        };
        let c2 = Candidate {
            encompasses_weight: 3.0,
            ..weighted_candidate()
        };
        assert!(CandidateFilter::candidate_weight(&c1) > CandidateFilter::candidate_weight(&c2));
    }

    /// Verifies that dead-end candidates have lower cost and therefore more weight.
    #[test]
    fn dead_end_more_weight() {
        let base = weighted_candidate();
        let dead_end = Candidate {
            dead_end: true,
            ..weighted_candidate()
        };

        assert!(
            CandidateFilter::candidate_cost(&dead_end) < CandidateFilter::candidate_cost(&base)
        );
        assert!(
            CandidateFilter::candidate_weight(&dead_end) > CandidateFilter::candidate_weight(&base)
        );
    }

    /// Verifies that candidate costs are clamped into the configured range.
    #[test]
    fn candidate_cost_clamped() {
        let favorable = Candidate {
            depth: 500.0,
            encompasses_weight: 500.0,
            dead_end: true,
            ..weighted_candidate()
        };
        let unfavorable = Candidate {
            frequency: 1000,
            encompassed_weight: 1000.0,
            velocity: Some(10.0),
            ..weighted_candidate()
        };
        assert_eq!(
            CandidateFilter::candidate_cost(&favorable),
            MIN_CANDIDATE_COST
        );
        assert_eq!(
            CandidateFilter::candidate_cost(&unfavorable),
            MAX_CANDIDATE_COST
        );
    }

    /// Verifies that the weight of very good candidates is clamped to the minimum weight to ensure
    /// they are still considered.
    #[test]
    fn candidate_weight_clamped() {
        let c = Candidate {
            depth: 500.0,
            encompasses_weight: 500.0,
            dead_end: true,
            urgency: 0.0001,
            ..weighted_candidate()
        };
        assert_eq!(CandidateFilter::candidate_weight(&c), MIN_CANDIDATE_WEIGHT);
    }

    /// Verifies that candidates with higher positive velocity get slightly more weight.
    #[test]
    fn positive_velocity_more_weight() {
        let base = Candidate {
            exercise_score: 2.0,
            velocity: Some(1.0),
            ..weighted_candidate()
        };
        let low_velocity = Candidate {
            velocity: Some(0.5),
            ..base.clone()
        };
        assert!(
            CandidateFilter::candidate_weight(&base)
                > CandidateFilter::candidate_weight(&low_velocity)
        );
    }

    /// Verifies that negative velocity raises cost and reduces weight.
    #[test]
    fn negative_velocity_less_weight() {
        let base = Candidate {
            exercise_score: 2.0,
            ..weighted_candidate()
        };
        let negative = Candidate {
            velocity: Some(-1.0),
            ..base.clone()
        };
        assert!(
            CandidateFilter::candidate_weight(&negative) < CandidateFilter::candidate_weight(&base)
        );
    }

    /// Verifies that stagnant non-mastered exercises get a cost reduction and more weight.
    #[test]
    fn stagnant_low_score_gets_bonus() {
        let base = Candidate {
            exercise_score: 2.0,
            ..weighted_candidate()
        };
        let stagnant = Candidate {
            velocity: Some(0.05),
            ..base.clone()
        };
        assert!(
            CandidateFilter::candidate_cost(&stagnant) < CandidateFilter::candidate_cost(&base)
        );
        assert!(
            CandidateFilter::candidate_weight(&stagnant) > CandidateFilter::candidate_weight(&base)
        );
    }

    /// Verifies that stagnant mastered exercises get a cost penalty and less weight.
    #[test]
    fn stagnant_high_score_gets_penalty() {
        let base = Candidate {
            exercise_score: 4.5,
            ..weighted_candidate()
        };
        let stagnant = Candidate {
            velocity: Some(0.05),
            ..base.clone()
        };
        assert!(
            CandidateFilter::candidate_cost(&stagnant) > CandidateFilter::candidate_cost(&base)
        );
        assert!(
            CandidateFilter::candidate_weight(&stagnant) < CandidateFilter::candidate_weight(&base)
        );
    }

    /// Verifies that velocity above the stagnation threshold only gets the directional velocity
    /// adjustment, without any stagnation bonus or penalty.
    #[test]
    fn positive_velocity_reduces_cost() {
        let base = Candidate {
            exercise_score: 2.0,
            ..weighted_candidate()
        };
        let active = Candidate {
            velocity: Some(0.5),
            ..base.clone()
        };
        assert!(CandidateFilter::candidate_cost(&active) < CandidateFilter::candidate_cost(&base));
        assert!(
            CandidateFilter::candidate_weight(&active) > CandidateFilter::candidate_weight(&base)
        );
    }

    /// Verifies that non-stagnant negative velocity increases cost and reduces weight.
    #[test]
    fn negative_velocity_increases_cost() {
        let base = Candidate {
            exercise_score: 2.0,
            ..weighted_candidate()
        };
        let active = Candidate {
            velocity: Some(-0.5),
            ..base.clone()
        };
        assert!(CandidateFilter::candidate_cost(&active) > CandidateFilter::candidate_cost(&base));
        assert!(
            CandidateFilter::candidate_weight(&active) < CandidateFilter::candidate_weight(&base)
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
}
