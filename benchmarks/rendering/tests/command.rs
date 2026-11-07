// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

#![cfg(unix)]

use base_web_html_benchmark_rendering::{BenchmarkConfig, command, run};
use std::io::{self, Write};
use std::process::{Command, ExitCode};

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).into()).collect()
}

#[test]
fn all_options_are_recorded_and_invalid_options_fail() {
    let values = args(&[
        "--workload",
        "http",
        "--variant",
        "reference",
        "--corpus",
        "escaped",
        "--iterations",
        "2",
        "--warmup",
        "0",
        "--items",
        "0",
        "--text-bytes",
        "0",
        "--buffer-bytes",
        "1",
        "--consumer-yields",
        "1",
    ]);
    let config = BenchmarkConfig::parse(values.clone()).unwrap();
    assert_eq!(config.iterations, 2);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    assert_eq!(command(values, &mut stdout, &mut stderr), ExitCode::SUCCESS);
    assert!(stderr.is_empty());
    let record: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(record["parameters"]["workload"], "http");
    assert_eq!(record["parameters"]["variant"], "reference");
    assert_eq!(record["parameters"]["consumer_yields"], 1);
    for input in [
        vec!["--workload", "wrong"],
        vec!["--variant", "wrong"],
        vec!["--corpus", "wrong"],
        vec!["--iterations", "0"],
        vec!["--buffer-bytes", "0"],
        vec!["--warmup", "-1"],
        vec!["--consumer-yields", "1"],
        vec!["--unknown", "1"],
        vec!["--items"],
        vec!["--items", "1", "--items", "2"],
    ] {
        stdout.clear();
        stderr.clear();
        assert_eq!(
            command(args(&input), &mut stdout, &mut stderr),
            ExitCode::FAILURE
        );
        assert!(stdout.is_empty());
        assert!(!stderr.is_empty());
    }
    for option in [
        "--iterations",
        "--items",
        "--text-bytes",
        "--buffer-bytes",
        "--consumer-yields",
    ] {
        assert!(BenchmarkConfig::parse(args(&[option, "not-an-integer"])).is_err());
    }
    assert!(
        BenchmarkConfig::parse(vec![
            "--items".into(),
            usize::MAX.to_string(),
            "--text-bytes".into(),
            "2".into()
        ])
        .is_err()
    );
    assert!(
        run(&BenchmarkConfig {
            iterations: 0,
            ..BenchmarkConfig::default()
        })
        .is_err()
    );
    let invalid_size = vec![
        "--workload".into(),
        "static".into(),
        "--text-bytes".into(),
        usize::MAX.to_string(),
        "--items".into(),
        "0".into(),
    ];
    stdout.clear();
    assert_eq!(
        command(invalid_size, &mut stdout, &mut stderr),
        ExitCode::FAILURE
    );
    assert!(stdout.is_empty());
}

struct RejectOutput;
impl Write for RejectOutput {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("output failed"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn output_errors_and_diagnostic_errors_do_not_change_failure_status() {
    let mut errors = Vec::new();
    assert_eq!(
        command(
            args(&["--iterations", "1", "--warmup", "0"]),
            &mut RejectOutput,
            &mut errors
        ),
        ExitCode::FAILURE
    );
    assert_eq!(
        command(
            args(&["--invalid", "1"]),
            &mut Vec::new(),
            &mut RejectOutput
        ),
        ExitCode::FAILURE
    );
}

#[test]
fn binary_emits_one_record_or_exits_unsuccessfully() {
    for workload in ["static", "list", "escape", "http"] {
        let output = Command::new(env!("CARGO_BIN_EXE_base-web-html-benchmark-rendering"))
            .args([
                "--workload",
                workload,
                "--iterations",
                "1",
                "--warmup",
                "0",
                "--items",
                "1",
                "--text-bytes",
                "8",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).lines().count(), 1);
        let _: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_base-web-html-benchmark-rendering"))
        .arg("--bad")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}

/// The shared runner accepts this workload's records for every configured scenario, and checks that both variants did identical work.
#[test]
fn shared_runner_smokes_every_scenario() {
    use base_benchmark::Workspace;
    let workspace = Workspace::new(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
    let package = Some("base-web-html-benchmark-rendering");
    let scenarios = workspace.suite(package).unwrap().scenarios;
    let directory = tempfile::tempdir().unwrap();
    let binary = std::path::Path::new(env!("CARGO_BIN_EXE_base-web-html-benchmark-rendering"));
    let campaigns = workspace
        .smoke_executable(
            package,
            binary,
            &directory.path().join("smoke"),
            Some(directory.path().join("lock")),
        )
        .unwrap();
    assert_eq!(campaigns.len(), scenarios.len());
    assert!(!campaigns.is_empty());
    for (campaign, scenario) in campaigns.iter().zip(&scenarios) {
        assert_eq!(campaign.summary.scenario, scenario.name);
        assert_eq!(campaign.summary.subjects.len(), 2, "{}", scenario.name);
        assert!(campaign.report.starts_with("smoke:"));
    }
}
