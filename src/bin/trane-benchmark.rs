//! CLI to run trane scheduler benchmarks with simulated student profiles.

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use trane::benchmark::{Benchmark, StudentResult};
use ustr::Ustr;

#[derive(Parser)]
#[command(about = "Run trane scheduler benchmarks with simulated student profiles")]
struct Args {
    #[arg(long, help = "Path to the trane library directory")]
    library_dir: PathBuf,

    #[arg(
        long,
        help = "ID of an advanced course used to decide when to check for full mastery"
    )]
    advanced_course: String,
}

/// Prints the result of a benchmark run for a given student profile.
fn print_result(label: &str, result: &StudentResult, max_days: u32) {
    match result.days_to_mastery {
        Some(days) => println!(
            "  {label}: mastery in {days} days, {sessions} sessions, {exercises} exercises",
            sessions = result.sessions_run,
            exercises = result.exercises_practiced,
        ),
        None => println!(
            "  {label}: no mastery in {max_days} days, {sessions} sessions, {exercises} exercises",
            sessions = result.sessions_run,
            exercises = result.exercises_practiced,
        ),
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    let benchmark = Benchmark {
        library_dir: args.library_dir,
        advanced_course: Ustr::from(&args.advanced_course),
        ..Benchmark::default()
    };
    benchmark.verify()?;

    println!("Running trane benchmarks...");
    let result = benchmark.run_benchmark()?;

    let max = benchmark.max_sessions;
    println!("Results:");
    print_result(
        "Remedial",
        &result.remedial_result,
        max * benchmark.remedial_profile.session_frequency,
    );
    print_result(
        "Below median",
        &result.below_median_result,
        max * benchmark.below_median_profile.session_frequency,
    );
    print_result(
        "Median",
        &result.median_result,
        max * benchmark.median_profile.session_frequency,
    );
    print_result(
        "Above median",
        &result.above_median_result,
        max * benchmark.above_median_profile.session_frequency,
    );
    print_result(
        "Excellent",
        &result.excellent_result,
        max * benchmark.excellent_profile.session_frequency,
    );

    Ok(())
}
