//! Contains a benchmark that simulates the performance of different types of students using a given
//! set of courses and scheduler options. The goal is to evaluate the performance of the scheduler
//! in terms of the time to mastery for different student profiles.

use anyhow::{Result, anyhow};
use chrono::Utc;
use rand::distr::{Distribution, weighted::WeightedIndex};
use std::path::{Path, PathBuf};
use ustr::{Ustr, UstrMap};
use walkdir::WalkDir;

use crate::{
    Trane,
    course_library::CourseLibrary,
    data::{MasteryScore, SchedulerOptions},
    scheduler::ExerciseScheduler,
};

/// Probabilities for each mastery score (1 through 5) given by a student for an individual
/// exercise. Indices 0..4 correspond to scores 1..5. Probabilities must sum up to 1.0.
pub type PerformanceProbs = [f32; 5];

/// Validates that the probabilities sum up to 1.0.
pub fn verify_probs(probs: &PerformanceProbs) -> Result<()> {
    let sum: f32 = probs.iter().sum();
    if (sum - 1.0).abs() < 1e-4 {
        Ok(())
    } else {
        Err(anyhow!("Probabilities must sum up to 1.0 instead of {sum}"))
    }
}

/// Describes the information that is used to simulate the performance of different types of
/// students.
#[derive(Clone, Debug)]
pub struct StudentProfile {
    /// The frequency at which the student practices expressed as how many days there are in between
    /// practice sessions. For example, a frequency of 2 means that the student practices every 2
    /// days.
    pub session_frequency: u32,

    /// The number of exercises that the student practices in each session.
    pub exercises_per_session: u32,

    /// The initial performance of the student when they see an exercise for the first time.
    pub initial_performance: PerformanceProbs,

    /// The number of trials the student needs to reach stable performance.
    pub trials_before_stable: u32,

    /// The stable performance of the student after they have practiced an exercise a large number
    /// of trials. The probabilities of trials in between the initial and the stable trials are
    /// interpolated.
    pub stable_performance: PerformanceProbs,
}

/// The result of running a benchmark for a student profile.
#[derive(Clone, Debug)]
pub struct StudentResult {
    /// The number of sessions it took for the student to master the advanced course. None if the
    /// student did not reach mastery within the maximum number of sessions.
    pub days_to_mastery: Option<u32>,

    /// The number of sessions run during the benchmark.
    pub sessions_run: u32,

    /// The number of exercises practiced by the student.
    pub exercises_practiced: u32,
}

/// The result of running the entire benchmark. See the definitions of the individual students in
/// the `Benchmark` struct.
#[derive(Clone, Debug)]
#[allow(missing_docs)]
pub struct BenchmarkResult {
    pub remedial_result: StudentResult,
    pub below_median_result: StudentResult,
    pub median_result: StudentResult,
    pub above_median_result: StudentResult,
    pub excellent_result: StudentResult,
}

/// Runs several simulations of different student profiles to benchmark the performance of the
/// scheduler given the provided library and options.
pub struct Benchmark {
    /// The directory where the trane library used in the benchmark are located.
    pub library_dir: PathBuf,

    /// The scheduler options to benchmark.
    pub scheduler_opts: SchedulerOptions,

    /// Profile for a student in the bottom 10% of the performance distribution.
    pub remedial_profile: StudentProfile,

    /// Profile for a student in the 25% of the performance distribution.
    pub below_median_profile: StudentProfile,

    /// Profile for an average student in the 50% of the performance distribution.
    pub median_profile: StudentProfile,

    /// Profile for a student in the 75% of the performance distribution.
    pub above_median_profile: StudentProfile,

    /// Profile for a student in the top 90% of the performance distribution.
    pub excellent_profile: StudentProfile,

    /// The ID of an advanced course that is used to decide whether the entirety of the curriculum
    /// should be checked. It does not have to be the final course as long as it is sufficiently
    /// advanced. It is used to avoid having to prematurely check.
    pub advanced_course: Ustr,

    /// The score threshold at which a course is considered mastered.
    pub mastery_threshold: f32,

    /// The maximum number of sessions to simulate for each student to avoid the simulation from
    /// running indefinitely.
    pub max_sessions: u32,
}

impl Default for Benchmark {
    /// Creates defaults for the benchmark. The library directory and advanced course ID are
    /// placeholders and should be replaced.
    fn default() -> Self {
        Benchmark {
            library_dir: PathBuf::from("placeholder_library_dir"),
            scheduler_opts: SchedulerOptions::default(),
            remedial_profile: StudentProfile {
                session_frequency: 4,
                exercises_per_session: 20,
                initial_performance: [0.3, 0.2, 0.25, 0.15, 0.1],
                trials_before_stable: 10,
                stable_performance: [0.02, 0.08, 0.1, 0.3, 0.5],
            },
            below_median_profile: StudentProfile {
                session_frequency: 3,
                exercises_per_session: 25,
                initial_performance: [0.2, 0.25, 0.3, 0.15, 0.1],
                trials_before_stable: 10,
                stable_performance: [0.02, 0.03, 0.1, 0.3, 0.55],
            },
            median_profile: StudentProfile {
                session_frequency: 2,
                exercises_per_session: 30,
                initial_performance: [0.15, 0.25, 0.3, 0.18, 0.12],
                trials_before_stable: 8,
                stable_performance: [0.02, 0.03, 0.1, 0.25, 0.6],
            },
            above_median_profile: StudentProfile {
                session_frequency: 1,
                exercises_per_session: 40,
                initial_performance: [0.1, 0.15, 0.4, 0.2, 0.15],
                trials_before_stable: 7,
                stable_performance: [0.01, 0.02, 0.07, 0.25, 0.65],
            },
            excellent_profile: StudentProfile {
                session_frequency: 1,
                exercises_per_session: 60,
                initial_performance: [0.08, 0.12, 0.4, 0.2, 0.2],
                trials_before_stable: 5,
                stable_performance: [0.01, 0.02, 0.07, 0.2, 0.7],
            },
            advanced_course: Ustr::from("placeholder_advanced_course"),
            mastery_threshold: 4.3,
            max_sessions: 2000,
        }
    }
}

impl Benchmark {
    /// Returns the timestamp for the start of a session. The anchor is `Utc::now()` at the
    /// start of the simulation (session 0). Sessions advance forward from the anchor.
    fn session_timestamp(anchor: i64, session: u32, session_frequency: u32) -> i64 {
        anchor + i64::from(session) * i64::from(session_frequency) * 86400
    }

    /// Returns the timestamp for an exercise within a session.
    fn exercise_timestamp(session_start: i64, exercise_index: u32) -> i64 {
        session_start + i64::from(exercise_index)
    }

    /// Copies the library from the source directory to a temporary directory.
    fn copy_library_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dst)?;
        for entry in WalkDir::new(src).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            let relative = path.strip_prefix(src).unwrap();
            let dst_path = dst.join(relative);

            if path.is_dir() {
                std::fs::create_dir_all(&dst_path)?;
            } else {
                std::fs::copy(path, &dst_path)?;
            }
        }
        Ok(())
    }

    /// Interpolates the performance probabilities of a student at a given trial number.
    fn interpolate_performance(profile: &StudentProfile, trial_num: u32) -> PerformanceProbs {
        let weight = (trial_num as f32 / profile.trials_before_stable as f32).min(1.0);
        std::array::from_fn(|i| {
            profile.initial_performance[i] * (1.0 - weight) + profile.stable_performance[i] * weight
        })
    }

    /// Gets the score for an exercise given the number of trials and the student profile.
    fn get_score(profile: &StudentProfile, trial_num: u32) -> MasteryScore {
        let performance = Self::interpolate_performance(profile, trial_num);
        let choice = WeightedIndex::new(performance).unwrap();
        match choice.sample(&mut rand::rng()) {
            0 => MasteryScore::One,
            1 => MasteryScore::Two,
            2 => MasteryScore::Three,
            3 => MasteryScore::Four,
            4 => MasteryScore::Five,
            _ => unreachable!(), // grcov-excl-line
        }
    }

    /// Checks if all courses have reached mastery level (triggered when advanced course reaches
    /// mastery).
    fn check_mastery(
        trane: &Trane,
        advanced_course: Ustr,
        mastery_threshold: f32,
        all_courses: &[Ustr],
    ) -> bool {
        if let Ok(Some(adv_score)) = trane.get_unit_score(advanced_course)
            && adv_score >= mastery_threshold
        {
            return all_courses.iter().all(|course_id| {
                trane
                    .get_unit_score(*course_id)
                    .ok()
                    .flatten()
                    .is_some_and(|s| s >= mastery_threshold)
            });
        }
        false
    }

    /// Runs a simulation for the given profile.
    fn simulate_student(&self, profile: &StudentProfile) -> Result<StudentResult> {
        // Create a temporary directory and copy the library there.
        let temp_dir = tempfile::TempDir::new()?;
        let temp_path = temp_dir.path().to_path_buf();
        Self::copy_library_dir(&self.library_dir, &temp_path)?;

        // Create trane instance and set the scheduler options.
        let mut trane = Trane::new_local(&temp_path, &temp_path)?;
        trane.set_scheduler_options(self.scheduler_opts.clone());

        // Run sessions until mastery is reached or the maximum number of sessions is reached.
        let anchor = Utc::now().timestamp();
        let all_courses = trane.get_course_ids();
        let mut trial_counts: UstrMap<u32> = UstrMap::default();
        let mut days_to_mastery = None;
        let mut sessions_run = 0;
        let mut exercises_practiced = 0;

        'session_loop: for session in 0..self.max_sessions {
            // Score exercises in the session, fetching new batches as needed.
            let session_start = Self::session_timestamp(anchor, session, profile.session_frequency);
            let mut exercises_in_session = 0u32;
            while exercises_in_session < profile.exercises_per_session {
                // Get a new batch, setting the simulated timestamp beforehand.
                let batch_ts = Self::exercise_timestamp(session_start, exercises_in_session);
                trane.override_current_timestamp(Some(batch_ts));
                let batch = trane.get_exercise_batch(None)?;
                if batch.is_empty() {
                    break 'session_loop; // grcov-excl-line
                }

                // Submit each exercise in the batch.
                for exercise in batch {
                    if exercises_in_session >= profile.exercises_per_session {
                        break;
                    }
                    let trial_count = trial_counts.entry(exercise.id).or_insert(0);
                    let score = Self::get_score(profile, *trial_count);
                    let timestamp = Self::exercise_timestamp(session_start, exercises_in_session);
                    trane.score_exercise(exercise.id, score, timestamp)?;
                    *trial_count += 1;
                    exercises_practiced += 1;
                    exercises_in_session += 1;
                }
            }
            sessions_run = session + 1;

            // Check if all courses have reached mastery and stop if they have. Set the correct
            // timestamp beforehand.
            let end_of_session_ts = Self::exercise_timestamp(session_start, exercises_in_session);
            trane.override_current_timestamp(Some(end_of_session_ts));
            if Self::check_mastery(
                &trane,
                self.advanced_course,
                self.mastery_threshold,
                &all_courses,
            ) {
                days_to_mastery = Some(session * profile.session_frequency);
            }
            if days_to_mastery.is_some() {
                break;
            }
        }

        Ok(StudentResult {
            days_to_mastery,
            sessions_run,
            exercises_practiced,
        })
    }

    /// Verifies that the benchmark configuration is valid.
    pub fn verify(&self) -> Result<()> {
        self.scheduler_opts.verify()?;
        for profile in [
            &self.remedial_profile,
            &self.below_median_profile,
            &self.median_profile,
            &self.above_median_profile,
            &self.excellent_profile,
        ] {
            verify_probs(&profile.initial_performance)?;
            verify_probs(&profile.stable_performance)?;
        }
        Ok(())
    }

    /// Runs the benchmark across all student profiles.
    pub fn run_benchmark(&self) -> Result<BenchmarkResult> {
        // Run each student profile in a separate thread and collect the results.
        let results = std::thread::scope(|s| {
            let h1 = s.spawn(|| self.simulate_student(&self.remedial_profile));
            let h2 = s.spawn(|| self.simulate_student(&self.below_median_profile));
            let h3 = s.spawn(|| self.simulate_student(&self.median_profile));
            let h4 = s.spawn(|| self.simulate_student(&self.above_median_profile));
            let h5 = s.spawn(|| self.simulate_student(&self.excellent_profile));
            (
                h1.join()
                    .map_err(|_| anyhow::anyhow!("remedial thread panicked"))
                    .and_then(|r| r),
                h2.join()
                    .map_err(|_| anyhow::anyhow!("below_median thread panicked"))
                    .and_then(|r| r),
                h3.join()
                    .map_err(|_| anyhow::anyhow!("median thread panicked"))
                    .and_then(|r| r),
                h4.join()
                    .map_err(|_| anyhow::anyhow!("above_median thread panicked"))
                    .and_then(|r| r),
                h5.join()
                    .map_err(|_| anyhow::anyhow!("excellent thread panicked"))
                    .and_then(|r| r),
            )
        });
        Ok(BenchmarkResult {
            remedial_result: results.0?,
            below_median_result: results.1?,
            median_result: results.2?,
            above_median_result: results.3?,
            excellent_result: results.4?,
        })
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests {
    use super::*;

    /// Verifies that the probabilities are validated to sum up to 1.0.
    #[test]
    fn performance_probs_validate_valid() {
        assert!(verify_probs(&[0.2, 0.2, 0.2, 0.2, 0.2]).is_ok());
    }

    /// Verifies that invalid probabilities that don't sum to 1.0 are rejected.
    #[test]
    fn performance_probs_validate_invalid() {
        assert!(verify_probs(&[0.5, 0.4, 0.0, 0.0, 0.0]).is_err());
    }

    fn test_profile() -> StudentProfile {
        StudentProfile {
            session_frequency: 1,
            exercises_per_session: 5,
            initial_performance: [0.5, 0.3, 0.1, 0.05, 0.05],
            trials_before_stable: 10,
            stable_performance: [0.0, 0.1, 0.2, 0.3, 0.4],
        }
    }

    /// Verifies that performance interpolation returns initial performance at trial 0.
    #[test]
    fn interpolate_performance_initial() {
        let perf = Benchmark::interpolate_performance(&test_profile(), 0);
        let expected = [0.5, 0.3, 0.1, 0.05, 0.05];
        for (a, b) in perf.iter().zip(expected.iter()) {
            assert!((a - b).abs() < f32::EPSILON);
        }
    }

    /// Verifies that performance interpolation reaches stable performance at the threshold.
    #[test]
    fn interpolate_performance_stable() {
        let perf = Benchmark::interpolate_performance(&test_profile(), 10);
        let expected = [0.0, 0.1, 0.2, 0.3, 0.4];
        for (a, b) in perf.iter().zip(expected.iter()) {
            assert!((a - b).abs() < f32::EPSILON);
        }
    }

    /// Verifies that performance interpolation blends initial and stable performance correctly.
    #[test]
    fn interpolate_performance_blend() {
        let perf = Benchmark::interpolate_performance(&test_profile(), 5);
        let expected = [0.25, 0.2, 0.15, 0.175, 0.225];
        for (a, b) in perf.iter().zip(expected.iter()) {
            assert!((a - b).abs() < f32::EPSILON);
        }
    }

    /// Verifies that session timestamps are calculated correctly based on anchor, session number,
    /// and frequency.
    #[test]
    fn session_timestamp() {
        let anchor = 1_000_000;
        assert_eq!(Benchmark::session_timestamp(anchor, 0, 1), anchor);
        assert_eq!(Benchmark::session_timestamp(anchor, 1, 1), anchor + 86400);
        assert_eq!(Benchmark::session_timestamp(anchor, 1, 2), anchor + 172800);
        assert_eq!(Benchmark::session_timestamp(anchor, 7, 1), anchor + 604800);
    }

    /// Verifies that exercise timestamps are calculated correctly within a session.
    #[test]
    fn exercise_timestamp() {
        assert_eq!(Benchmark::exercise_timestamp(0, 0), 0);
        assert_eq!(Benchmark::exercise_timestamp(0, 5), 5);
        assert_eq!(Benchmark::exercise_timestamp(86400, 0), 86400);
        assert_eq!(Benchmark::exercise_timestamp(86400, 3), 86403);
    }

    /// Verifies that get_score returns a deterministic score when probabilities are 100% for one
    /// rating.
    #[test]
    fn get_score_deterministic() {
        let benchmark = Benchmark {
            remedial_profile: StudentProfile {
                session_frequency: 1,
                exercises_per_session: 5,
                initial_performance: [0.0, 0.0, 0.0, 0.0, 1.0],
                trials_before_stable: 1,
                stable_performance: [0.0, 0.0, 0.0, 0.0, 1.0],
            },
            ..Default::default()
        };

        let profile = &benchmark.remedial_profile;
        for _ in 0..10 {
            let score = Benchmark::get_score(profile, 0);
            assert_eq!(score, MasteryScore::Five);
        }
    }

    /// Verifies that the default benchmark is valid.
    #[test]
    fn verify_default_benchmark() {
        let benchmark = Benchmark::default();
        assert!(benchmark.verify().is_ok());
    }

    /// Verifies that the benchmark completes with a valid configuration.
    #[test]
    fn run_benchmark() {
        let benchmark = Benchmark {
            library_dir: PathBuf::from("tests/small_test_library"),
            advanced_course: Ustr::from("trane::music::improvise_for_real::sing_the_numbers::3"),
            max_sessions: 60,
            ..Benchmark::default()
        };
        let result = benchmark.run_benchmark();
        assert!(result.is_ok());

        // Verify that sessions and exercises were practiced for all student profiles.
        let benchmark_result = result.unwrap();
        assert!(benchmark_result.remedial_result.exercises_practiced > 0);
        assert!(benchmark_result.remedial_result.sessions_run > 0);
        assert!(benchmark_result.below_median_result.exercises_practiced > 0);
        assert!(benchmark_result.below_median_result.sessions_run > 0);
        assert!(benchmark_result.median_result.exercises_practiced > 0);
        assert!(benchmark_result.median_result.sessions_run > 0);
        assert!(benchmark_result.above_median_result.exercises_practiced > 0);
        assert!(benchmark_result.above_median_result.sessions_run > 0);
        assert!(benchmark_result.excellent_result.exercises_practiced > 0);
        assert!(benchmark_result.excellent_result.sessions_run > 0);

        // Verify that at least the above median and excellent profiles reached mastery.
        assert!(
            benchmark_result
                .above_median_result
                .days_to_mastery
                .is_some()
        );
        assert!(benchmark_result.excellent_result.days_to_mastery.is_some());
    }
}
