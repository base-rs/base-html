// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

#[test]
fn discovers_and_runs_project_tasks() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_base-html-bake"))
        .arg("--list")
        .output()
        .unwrap();
    assert!(output.status.success());
    let tasks = String::from_utf8(output.stdout).unwrap();
    assert!(tasks.contains("test:coverage"));
    assert!(tasks.contains("cargo:after_version_bump"));
    for task in ["list", "prepare", "smoke", "run", "summarize"] {
        assert!(
            tasks.contains(&format!("benchmark:{task}")),
            "benchmark:{task}"
        );
    }
}
