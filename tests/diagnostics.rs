// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

#[test]
fn invalid_templates_report_actionable_errors() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}
