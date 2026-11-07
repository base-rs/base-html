# Exploratory Rendering Baseline

## Current benchmark suite

The private [`base-web-html-benchmark-rendering` package](https://github.com/base-rs/base-web-html/tree/main/benchmarks/rendering) now supplies parameterized static-page, component-list, escaping, and HTTP-body workloads. Each process emits a shared `base-benchmark` record with measured-interval wall, CPU, and context-switch totals after independent full-byte preflight. The `benchmark:*` Bake tasks prepare frozen builds and a layout replica, collect randomized paired rounds, and retain raw records and summaries with provenance. See that guide for smoke commands and the requirements for recorded measurements. Record optimization attempts, including rejected ones, with the shared investigation template.

The observations below predate the shared benchmark/performance skills. They remain historical exploratory records, not controlled evidence or a regression baseline. The new harness has different sink, lifecycle, and validation boundaries; do not compare its timings directly with these old numbers. No new controlled measurement campaign has been performed, and no numerical target has been added.

## Workload and procedure

Run `cargo bench --bench render` from the repository root. The benchmark compares the template macro with handwritten async `HtmlWriter` calls using the same escaping implementation. Each renders four user names, including Unicode and HTML-sensitive characters. Both the engine output buffer and a preallocated destination `Vec<u8>` are reused. Each render explicitly flushes; the measuring destination observes and clears its bytes on flush. Each batch contains 100,000 renders inside one executor invocation; five batches run with the template measured before the handwritten baseline each time.

This isolates generated-code overhead relative to the writer implementation. It does not compare the engine with an independently optimized escaper or other template engines. It does not measure network output, time to first byte, peak memory, allocation counts, compilation time, or agent success. These remain separate evaluation questions. There is no agreed performance regression threshold yet.

## Async-first observation

Measured on October 27, 2026, on Linux x86_64 with an AMD Ryzen 9 9950X3D, Rust 1.99.1 (Arch Linux build), LLVM 23.1.3, and Cargo's default optimized bench profile. No custom `RUSTFLAGS`, CPU affinity, frequency controls, or statistical harness were configured. Background development activity was not controlled.

| Batch | Template, 100,000 renders | Handwritten async writes, 100,000 renders |
| --- | --- | --- |
| 1 | 13.493470 ms | 14.537712 ms |
| 2 | 14.296978 ms | 14.382748 ms |
| 3 | 14.255086 ms | 13.452060 ms |
| 4 | 13.514442 ms | 14.049912 ms |
| 5 | 13.443690 ms | 13.562252 ms |

The generated and handwritten async paths have similar observed timings in this experiment. This does not establish statistical equivalence. The previous synchronous prototype below was faster in its tiny in-memory workload; buffering, extra copies, async state machines, and the changed measurement harness all differ, so this comparison does not isolate the cost of async itself. Async-first is an architectural choice, not a claim that async improves in-memory rendering throughput.

## Historical synchronous prototype

The following observation belongs to the earlier synchronous implementation, not the current API or benchmark harness. That harness reused only the destination buffer and wrote directly into it without the async writer's additional buffering layer.

Measured on October 27, 2026, on Linux x86_64 with an AMD Ryzen 9 9950X3D, Rust 1.99.1 (Arch Linux build), LLVM 23.1.3, and Cargo's default optimized bench profile. No custom `RUSTFLAGS`, CPU affinity, frequency controls, or statistical harness were configured.

| Batch | Template, 100,000 renders | Handwritten, 100,000 renders |
| --- | --- | --- |
| 1 | 4.336087 ms | 4.377269 ms |
| 2 | 4.489491 ms | 4.397718 ms |
| 3 | 4.479401 ms | 4.414420 ms |
| 4 | 4.479870 ms | 4.425870 ms |
| 5 | 4.493091 ms | 4.389918 ms |

Both observations have fixed measurement order and uncontrolled warm-up, scheduling, and optimizer effects. Neither is a statistically established performance guarantee or acceptance baseline, nor evidence of improved agent outcomes.
