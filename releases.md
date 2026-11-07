# Releases

## Unreleased

- Rename the crates to `base-web-html` and `base-web-html-macros`, joining the `base-web-*` family of Base web crates; `base-html` and `base-html-macros` 0.1.x remain available but receive no further releases.

## v0.1.1

- Refuse further writes to an `HtmlWriter` after a template using it is cancelled or fails part-way, so later escaped values cannot complete its open tag or attribute, and end an HTTP body with an error when its renderer abandoned a template; `HtmlWriter::is_abandoned` reports this state.
- Document that responses must declare UTF-8, and that URL and framework-evaluated attributes need caller validation.

## v0.1.0

- Refresh workspace dependencies and shared Base project guidance, including the benchmark and performance skills.
- Add a private, parameterized benchmark suite with static, component-list, escaping, and HTTP-body workloads, and independent correctness preflights. Collection, controls, and summaries use the shared `base-benchmark` tooling through `benchmark:*` Bake tasks, with no Python.
- Make `html!` and `HtmlWriter` async-first, with one template/component interface and an explicit adapter for synchronous destinations.
- Add bounded buffering, incremental string escaping, native awaited expressions, async components, and `@flush` boundaries.
- Add an optional demand-driven HTTP body with bounded render-ahead, cancellation, and late-error propagation, without spawning a renderer task.
- Restrict arbitrary `Display` formatting to an explicit 512-byte fallback; custom values can render incrementally through `HtmlText`.
- Add a compile-time Rust HTML DSL with escaped values, control flow, explicit trusted markup, and writer-sharing component calls.
- Reject `return`, and `break` or `continue` that would leave a template expression, at compile time, since they would skip the remaining markup and could let escaped text be parsed as attributes.
- Require Rust 1.88, and document the optional HTTP body on docs.rs.
- Coalesce adjacent static markup and render incrementally to I/O destinations.
- Add Base project tooling, syntax diagnostics, integration tests, and an exploratory handwritten-baseline benchmark.
