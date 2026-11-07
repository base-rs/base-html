// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Workloads for in-process HTML rendering and pull-body streaming, not a network benchmark.
//!
//! Each process emits one [`base_benchmark::ProcessRecord`]. Collection, rounds, controls, and summaries come from `base-benchmark`.
mod benchmark_config;
mod workloads;

pub use benchmark_config::{BenchmarkConfig, Corpus, Variant, Workload};

use base_benchmark::ProcessRecord;
use std::io::{self, Write};
use std::process::ExitCode;

/// Run one configured workload. Setup, correctness preflight, and warm-up are excluded from measurement. A preflight mismatch is an `InvalidData` error, and a byte count that changes after preflight panics; neither produces a record.
pub fn run(config: &BenchmarkConfig) -> io::Result<ProcessRecord> {
    config.validate()?;
    workloads::run(config)
}

/// Execute the CLI, emitting exactly one record on success and none on failure.
pub fn command(
    arguments: impl IntoIterator<Item = String>,
    out: &mut impl Write,
    errors: &mut impl Write,
) -> ExitCode {
    let result = BenchmarkConfig::parse(arguments).and_then(|config| run(&config)?.emit(out));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // Reporting failure cannot turn a failed benchmark into a successful measurement.
            let _ = writeln!(errors, "benchmark failed: {error}");
            ExitCode::FAILURE
        }
    }
}
