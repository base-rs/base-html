# `base-html-benchmark-rendering`

A private workspace package for rendering, escaping, and pull-body streaming experiments. It compares the engine with equivalent handwritten operations, not with a competing template engine. No runtime library depends on this package. The benchmark enables the library's HTTP feature; workspace-wide tests therefore unify it. CI separately checks the library without that feature.

The workload binary measures one configuration and writes one [`base-benchmark`](https://github.com/base-rs/base-benchmark) process record. Preparation, randomized rounds, layout-replica controls, locking, cleanup, validation, and summaries come from that shared tooling through the `benchmark:*` Bake tasks. No Python is involved.

## Quick validation

```sh
cargo test -p base-html-benchmark-rendering --locked
cargo bake benchmark:smoke
```

Smoke runs build the current tree and run every scenario once with tiny iteration counts. They check correctness and lifecycle only; their reports deliberately omit comparisons. The binary exits unsuccessfully, without a record, on invalid options, resource errors, output failures, or correctness mismatches, and the shared runner rejects malformed output.

## Workloads and references

| Workload | Engine | Reference | Operation boundary |
| --- | --- | --- | --- |
| `static` | Compile-time static page | One coalesced literal write | Render and final flush into a reused counting/discard sink. |
| `list` | Loop with text/attribute values and component calls | Equivalent handwritten async components and writer calls | Render and final flush into the same reused sink/writer. |
| `escape` | Core byte-scanning escaper | Independent character-scanning escaper, writing safe spans and replacements through the same writer | Escape one value and flush. |
| `http` | Progressive heading, explicit flush, then component list | Same content, components, escaping, and flush points through the same HTTP adapter | Construct a body, consume every data frame, include automatic final flush and drop the body. |

The list reference intentionally shares the engine's writer and escaper; it isolates generated-template overhead, not the efficiency of the shared writer. The escaping reference is independent of the engine's scan but still shares buffered output. Both HTTP variants include shared chunk allocation/copying, locking, and polling; neither is a baseline without that adapter.

Every process checks both variants against independently assembled, fully escaped expected bytes before timing. It also requires identical output boundaries. Tests compare complete bytes and frame-size sequences across tiny capacities and consumer yields. Sources and expected output are prepared before measurement. These preflights can retain whole documents; they are not bounded-streaming memory evidence.

Timed sinks observe slices with `black_box`, count bytes/events, and discard output without an extra copy or per-byte checksum. Each timed operation checks its byte count. The checksum in the record describes the untimed verified output. Input preparation, oracle construction, both full-output preflights, and warmup are excluded from interval timing and CPU samples. Counter resets, byte-count checks, final flush, and one executor entry for the timed batch are included. The regular writer/sink is reused across operations; the HTTP pipeline is constructed and destroyed per operation.

## Parameters

All options use `--name value`; defaults are defined in [`BenchmarkConfig`](src/benchmark_config.rs).

- `--workload`: `static`, `list`, `escape`, or `http`.
- `--variant`: `engine` or `reference`.
- `--iterations` and `--warmup`: measured and excluded operation counts. Iterations must be positive; warmup may be zero.
- `--items`: rows per list; zero is valid. Unused for static and escape output.
- `--text-bytes`: exact source UTF-8 byte length, including zero. Tiny lengths may be padded with ASCII to avoid splitting a scalar.
- `--corpus`: `plain`, `escaped`, or `unicode` (includes Unicode and all five escaped characters).
- `--buffer-bytes`: positive engine output capacity; HTTP uses the same limit for each queued chunk.
- `--consumer-yields`: finite self-waking consumer scheduling yields after each HTTP frame. Zero means immediate consumption. Rejected for other workloads.

Consumer yields are not sleeps, actual network delay, or counts of `Body::poll_frame` returning `Pending`. Time to first frame begins before body construction and ends when the consumer receives its first data frame. It includes any render-ahead after the initial flush and does not necessarily cover the entire heading when buffers are small. It is not browser time to first byte or time to flush completion. There is no server, socket, HTTP protocol encoding, compression, TLS, or separate client in this workload.

## Scenarios

[`Cargo.toml`](Cargo.toml) declares the `static`, `list`, `escape`, and `http` scenarios under `[package.metadata.base-benchmark]`. Every scenario has the variants `reference` and `engine`; `reference` is the baseline of each variant comparison. Iteration and warmup counts are the workload defaults, recorded in each record's `parameters`. To pin them for measurements, add `measure-arguments`, which apply only to screening and recorded runs, so they never collide with the smoke arguments. Run `cargo bake benchmark:list` to see the resolved configuration, including each scenario's baseline variant.

Each record reports, for the measured operations only, `wall_time`, `cpu_time`, `user_cpu_time`, `system_cpu_time`, both context-switch counts, `output_bytes`, `sink_events` (writes, or frames for HTTP), and for HTTP `first_frame_time`. Summaries divide these totals by operations. Total CPU time has microsecond resolution, but Linux splits it into user and system time by scheduler ticks, so the split is treated as 10 ms resolution. A percentage change is reported only when every round's baseline is at least 100 quanta; otherwise the metric is unresolved and only differences are shown. Counts are compared by difference, never as percentages. The record signature is the verified output's length and checksum. The runner rejects records whose workload or signature differ from the campaign's first record, and an artifact whose parameters for a variant differ from another artifact's, such as a changed default iteration count.

## Screening and recorded comparisons

```sh
cargo bake benchmark:prepare --tree current=. --replicas 1
cargo bake benchmark:run list --artifacts target/base-benchmark/prepared-TIME/artifacts.json --cpus 2
```

Prepare builds each labelled source tree with the same profile, then copies the baseline to a sibling directory and rebuilds it as a layout replica; `--replicas N` builds several, each one more sample of build layout. Run collects every artifact and variant in seeded random rounds under the host-wide lock. Each sample starts from an equal-length staged path in the campaign directory, in the workspace root, with only `PATH` and `LC_ALL=C` in its environment plus a random-length padding variable, so stack alignment varies between processes instead of being fixed per build. Reports show paired changes, better and worse round counts with differences below the measurement quantum counted as ties, and an exact sign test. Screening is triage, not acceptance evidence. Warnings mark missing CPU placement, disabled stack padding, missing layout replicas, changes that go both ways across rounds, build changes within the replicas' range, variant conclusions that reverse in or cannot be checked against the replica, and metrics that are missing or defined differently between subjects.

To compare revisions, check them out into separate directories (for example with `git worktree add`) and pass each with `--tree LABEL=PATH`, baseline first. Recorded runs (`--mode recorded`) additionally require at least 10 rounds, explicit `--cpus`, `--decision-rule`, `--idle-host true`, committed and unmodified trees and out-of-tree path dependencies, the same toolchain, Cargo version, build environment, and Cargo configuration for every artifact, and a layout replica when comparing separately built trees. The run reports every missing prerequisite at once. Use an otherwise idle dedicated Linux host, pin a physical core, and leave its SMT sibling idle.

Whole-process `perf stat` counters can be added with `--perf-event instructions:u`. They cover startup, both preflights, warmup, and serialization, so they are reported as totals and never divided by measured operations. Phase-matched hardware counters are not implemented.

For a reviewed measurement, keep the campaign directory (`metadata.json`, `samples.jsonl`, `summary.json`) under `results/<topic>/` and record the investigation with the [shared template](https://github.com/base-rs/base-benchmark/blob/main/templates/investigation.md), including rejected attempts. Derive figures from `summary.json`. Link results from the [performance record](../../context/performance.md). No controlled results or performance claims are included with this suite.
