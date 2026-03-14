//! Defines the logic for shuffling the final batch of candidates before they are returned.

use rand::seq::SliceRandom;

use crate::{data::SchedulerOptions, scheduler::Candidate};

/// The maximum number of low-scoring candidates from the same course in a single group.
const MAX_GROUP_SIZE: usize = 5;

pub(crate) struct Shuffler;

impl Shuffler {
    /// Shuffles the final batch of candidates before they are returned, making sure to group new
    /// and low-scoring exercises from the same course together. Blocking works better than
    /// interleaving for these exercises.
    pub(crate) fn shuffle_candidates(
        candidates: Vec<Candidate>,
        options: &SchedulerOptions,
    ) -> Vec<Candidate> {
        // Partition the candidates based on the threshold score.
        let threshold_score = options.target_window_opts.range.1;
        let (mut low_candidates, high_candidates): (Vec<Candidate>, Vec<Candidate>) = candidates
            .into_iter()
            .partition(|candidate| candidate.exercise_score <= threshold_score);

        // Group the low candidates by course and turn each high candidates into its own group.
        let rng = &mut rand::rng();
        low_candidates.sort_by_key(|candidate| candidate.course_id);
        let grouped_low_candidates: Vec<Vec<Candidate>> = low_candidates
            .chunk_by(|a, b| a.course_id == b.course_id)
            .flat_map(|chunk| {
                let mut chunk = chunk.to_vec();
                chunk.shuffle(rng);
                chunk
                    .chunks(MAX_GROUP_SIZE)
                    .map(|c| c.to_vec())
                    .collect::<Vec<_>>()
            })
            .collect();
        let grouped_high_candidates: Vec<Vec<Candidate>> = high_candidates
            .into_iter()
            .map(|candidate| vec![candidate])
            .collect();

        // Chain, shuffle, and flatten the groups.
        let mut all_groups = grouped_low_candidates;
        all_groups.extend(grouped_high_candidates);
        all_groups.shuffle(rng);
        all_groups.into_iter().flatten().collect()
    }
}

#[cfg(test)]
mod tests {
    use ustr::Ustr;

    use crate::{data::SchedulerOptions, scheduler::Candidate};

    use super::{MAX_GROUP_SIZE, Shuffler};

    /// Creates a candidate with the given course ID, exercise ID, and exercise score.
    fn candidate(course_id: &str, exercise_id: &str, exercise_score: f32) -> Candidate {
        Candidate {
            exercise_id: Ustr::from(exercise_id),
            lesson_id: Ustr::from("lesson_1"),
            course_id: Ustr::from(course_id),
            depth: 0.0,
            exercise_score,
            lesson_score: 0.0,
            course_score: 0.0,
            num_trials: 0,
            last_seen: 0.0,
            frequency: 0,
            dead_end: false,
        }
    }

    /// Returns whether all candidates matching the predicate are contiguous in the result.
    fn is_contiguous(result: &[Candidate], predicate: impl Fn(&Candidate) -> bool) -> bool {
        let positions: Vec<usize> = result
            .iter()
            .enumerate()
            .filter(|(_, c)| predicate(c))
            .map(|(i, _)| i)
            .collect();
        if positions.is_empty() {
            return true;
        }
        positions.last().unwrap() - positions.first().unwrap() == positions.len() - 1
    }

    /// Verifies that an empty input returns an empty output.
    #[test]
    fn empty_candidates() {
        let options = SchedulerOptions::default();
        let result = Shuffler::shuffle_candidates(vec![], &options);
        assert!(result.is_empty());
    }

    /// Verifies that all candidates are preserved after shuffling.
    #[test]
    fn preserves_all_candidates() {
        let options = SchedulerOptions::default();
        let candidates = vec![
            candidate("c1", "e1", 1.0),
            candidate("c1", "e2", 1.5),
            candidate("c2", "e3", 0.5),
            candidate("c3", "e4", 4.0),
            candidate("c3", "e5", 3.5),
        ];
        let result = Shuffler::shuffle_candidates(candidates, &options);
        assert_eq!(result.len(), 5);

        let mut ids: Vec<String> = result.iter().map(|c| c.exercise_id.to_string()).collect();
        ids.sort();
        assert_eq!(ids, vec!["e1", "e2", "e3", "e4", "e5"]);
    }

    /// Verifies that low-scoring candidates from the same course appear contiguously.
    #[test]
    fn low_candidates_grouped_by_course() {
        let options = SchedulerOptions::default();
        let candidates = vec![
            candidate("c1", "e1", 1.0),
            candidate("c2", "e2", 0.5),
            candidate("c1", "e3", 2.0),
            candidate("c2", "e4", 1.5),
            candidate("c1", "e5", 0.0),
        ];

        for _ in 0..20 {
            let result = Shuffler::shuffle_candidates(candidates.clone(), &options);
            assert!(is_contiguous(&result, |c| c.course_id == "c1"));
            assert!(is_contiguous(&result, |c| c.course_id == "c2"));
        }
    }

    /// Verifies that low candidates are grouped by course even when mixed with high candidates.
    #[test]
    fn mixed_low_and_high_candidates() {
        let options = SchedulerOptions::default();
        let candidates = vec![
            candidate("c1", "e1", 1.0),
            candidate("c1", "e2", 2.0),
            candidate("c2", "e3", 0.5),
            candidate("c1", "e4", 4.0),
            candidate("c2", "e5", 3.0),
        ];

        let threshold = options.target_window_opts.range.1;
        for _ in 0..20 {
            let result = Shuffler::shuffle_candidates(candidates.clone(), &options);
            assert_eq!(result.len(), 5);
            assert!(is_contiguous(&result, |c| c.course_id == "c1"
                && c.exercise_score <= threshold));
            assert!(is_contiguous(&result, |c| c.course_id == "c2"
                && c.exercise_score <= threshold));
        }
    }

    /// Verifies that candidates at exactly the threshold score are treated as low candidates.
    #[test]
    fn threshold_boundary() {
        let options = SchedulerOptions::default();
        let threshold = options.target_window_opts.range.1;
        let candidates = vec![
            candidate("c1", "e1", threshold),
            candidate("c1", "e2", threshold),
            candidate("c2", "e3", threshold),
        ];

        for _ in 0..20 {
            let result = Shuffler::shuffle_candidates(candidates.clone(), &options);
            assert!(is_contiguous(&result, |c| c.course_id == "c1"));
            assert!(is_contiguous(&result, |c| c.course_id == "c2"));
        }
    }

    /// Verifies that a course with more than MAX_GROUP_SIZE low-scoring exercises is split into
    /// multiple groups of at most MAX_GROUP_SIZE.
    #[test]
    fn large_course_split_into_chunks() {
        let options = SchedulerOptions::default();
        let mut candidates: Vec<Candidate> = (0..8)
            .map(|i| candidate("c1", &format!("e_c1_{i}"), 1.0))
            .collect();
        // Add enough other groups so the c1 chunks are likely to be separated.
        for i in 0..5 {
            candidates.push(candidate(
                &format!("c{}", i + 2),
                &format!("e_other_{i}"),
                1.0,
            ));
        }

        let mut saw_split = false;
        for _ in 0..20 {
            let result = Shuffler::shuffle_candidates(candidates.clone(), &options);
            assert_eq!(result.len(), 13);

            // Find contiguous runs of c1 exercises. If splitting works, the maximum run should be
            // at most MAX_GROUP_SIZE in at least some iterations.
            let mut run_length = 0;
            let mut max_run = 0;
            for c in &result {
                if c.course_id == "c1" {
                    run_length += 1;
                    max_run = max_run.max(run_length);
                } else {
                    run_length = 0;
                }
            }
            if max_run <= MAX_GROUP_SIZE {
                saw_split = true;
                break;
            }
        }
        assert!(saw_split);
    }
}
