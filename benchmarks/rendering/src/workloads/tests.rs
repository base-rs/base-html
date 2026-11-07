// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use super::*;
use base_benchmark::Measurement;
use base_html::AsyncWrite;
use futures::task::{ArcWake, waker};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::Context;

fn small(workload: Workload) -> BenchmarkConfig {
    BenchmarkConfig {
        workload,
        iterations: 2,
        warmup: 1,
        items: 2,
        text_bytes: 17,
        buffer_bytes: 7,
        ..BenchmarkConfig::default()
    }
}

#[test]
fn every_workload_variant_and_corpus_completes() {
    for workload in [
        Workload::Static,
        Workload::List,
        Workload::Escape,
        Workload::Http,
    ] {
        for corpus in [Corpus::Plain, Corpus::Escaped, Corpus::Unicode] {
            for variant in [Variant::Engine, Variant::Reference] {
                let config = BenchmarkConfig {
                    corpus,
                    variant,
                    consumer_yields: usize::from(workload == Workload::Http),
                    ..small(workload)
                };
                let record = crate::run(&config).unwrap();
                record.validate().unwrap();
                assert_eq!(record.workload, WORKLOAD);
                let sample = &record.samples[0];
                assert_eq!(sample.operations, config.iterations);
                let total = |name: &str| match sample.metric(name).map(|metric| &metric.measurement)
                {
                    Some(Measurement::Total { value }) => *value,
                    other => panic!("{name}: {other:?}"),
                };
                assert!(total("output_bytes") > 0 && total("sink_events") > 0);
                assert_eq!(
                    sample.metric("first_frame_time").is_some(),
                    workload == Workload::Http
                );
                assert!(
                    record
                        .signature
                        .unwrap()
                        .starts_with(&format!("{}:", total("output_bytes") / config.iterations))
                );
            }
        }
    }
}

#[test]
fn byte_exact_sources_and_empty_inputs_are_supported() {
    for corpus in [Corpus::Plain, Corpus::Escaped, Corpus::Unicode] {
        for length in 0..32 {
            assert_eq!(source_text(corpus, length).unwrap().len(), length);
        }
    }
    for workload in [
        Workload::Static,
        Workload::List,
        Workload::Escape,
        Workload::Http,
    ] {
        crate::run(&BenchmarkConfig {
            items: 0,
            text_bytes: 0,
            warmup: 0,
            ..small(workload)
        })
        .unwrap();
    }
    for workload in [Workload::List, Workload::Http] {
        crate::run(&BenchmarkConfig {
            items: 2,
            text_bytes: 0,
            warmup: 0,
            ..small(workload)
        })
        .unwrap();
    }
}

#[test]
fn bodies_preserve_complete_bytes_and_frame_sequences_across_yields() {
    for capacity in [1, 13, 14, 15, 8192] {
        for items in [0, 2] {
            let config = BenchmarkConfig {
                buffer_bytes: capacity,
                items,
                ..small(Workload::Http)
            };
            let text = source_text(Corpus::Unicode, 17).unwrap();
            let expected =
                expected_output(&config, &text, output_capacity(&config).unwrap()).unwrap();
            let engine = block_on(preflight_output(&config, &text));
            let reference = block_on(preflight_output(
                &BenchmarkConfig {
                    variant: Variant::Reference,
                    consumer_yields: 2,
                    ..config
                },
                &text,
            ));
            assert_eq!(engine.captured.as_deref(), Some(expected.as_slice()));
            assert_eq!(engine.captured, reference.captured);
            assert_eq!(engine.frame_sizes, reference.frame_sizes);
            assert!(engine.frame_sizes.iter().all(|size| *size <= capacity));
        }
    }
}

#[test]
fn output_and_shape_checks_reject_mismatches() {
    let mut stats = OutputStats::new(true);
    stats.observe(b"actual");
    assert!(output::validate(&stats, b"expected").is_err());
    assert!(output::validate_frames(&[1, 2], &[3]).is_err());
    let rendered = |bytes: &[&[u8]]| {
        let mut stats = OutputStats::new(true);
        bytes.iter().for_each(|frame| stats.observe(frame));
        stats
    };
    assert!(preflight(b"ab", |_| rendered(&[b"ab"])).is_ok());
    assert!(preflight(b"ab", |_| rendered(&[b"ba"])).is_err());
    assert!(
        preflight(b"ab", |variant| rendered(if variant == Variant::Engine {
            &[b"ab"]
        } else {
            &[b"ba"]
        }))
        .is_err()
    );
    assert!(
        preflight(b"ab", |variant| rendered(if variant == Variant::Engine {
            &[b"ab"]
        } else {
            &[b"a", b"b"]
        }))
        .is_err()
    );
    assert_eq!(output::checksum(b"abc"), 0xe71fa2190541574b);
    assert_ne!(output::checksum(b"abc"), output::checksum(b"cba"));
}

#[test]
fn impossible_sizes_fail_before_becoming_samples() {
    let config = BenchmarkConfig {
        items: usize::MAX,
        text_bytes: 0,
        ..small(Workload::List)
    };
    assert!(crate::run(&config).is_err());
    let config = BenchmarkConfig {
        items: 0,
        text_bytes: usize::MAX,
        ..small(Workload::Static)
    };
    assert!(crate::run(&config).is_err());
    let config = BenchmarkConfig {
        items: usize::MAX / 18 - 4,
        text_bytes: 0,
        ..small(Workload::List)
    };
    assert!(crate::run(&config).is_err());
    assert!(
        output_capacity(&BenchmarkConfig {
            text_bytes: usize::MAX,
            ..small(Workload::Escape)
        })
        .is_err()
    );
}

#[test]
fn failed_resource_samples_are_not_reported() {
    let config = small(Workload::Static);
    for failure in [0, 1] {
        assert!(
            run_with(&config, Failing(failure), |preflight, text| block_on(
                preflight_output(preflight, text)
            ))
            .is_err()
        );
    }
}

#[test]
fn preflight_mismatches_fail_before_measurement() {
    let config = small(Workload::Static);
    let error = run_with(&config, CurrentProcess, |_, _| OutputStats::new(true)).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{error}");
}

/// Fails the `n`th resource snapshot.
struct Failing(usize);

impl ResourceSource for Failing {
    fn snapshot(&mut self) -> io::Result<base_benchmark::ResourceUsage> {
        if self.0 == 0 {
            return Err(io::Error::other("resource sample failed"));
        }
        self.0 -= 1;
        CurrentProcess.snapshot()
    }
    fn resolution_ns(&self) -> u64 {
        1000
    }
}

struct FailureSink {
    remaining: usize,
    fail_flush: bool,
}

impl AsyncWrite for FailureSink {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.remaining == 0 {
            return Poll::Ready(Err(io::Error::other("write failed")));
        }
        let count = self.remaining.min(bytes.len());
        self.remaining -= count;
        Poll::Ready(Ok(count))
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        if self.fail_flush {
            Poll::Ready(Err(io::Error::other("flush failed")))
        } else {
            Poll::Ready(Ok(()))
        }
    }
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}

#[test]
fn renderer_failures_are_failures_not_partial_samples() {
    block_on(async {
        for workload in [
            Workload::Static,
            Workload::List,
            Workload::Escape,
            Workload::Http,
        ] {
            for variant in [Variant::Engine, Variant::Reference] {
                let config = BenchmarkConfig {
                    variant,
                    ..small(workload)
                };
                let text = "a&<>\"'z";
                let expected = expected_output(&config, text, 8192).unwrap();
                for remaining in 0..expected.len() {
                    let mut out = HtmlWriter::with_capacity(
                        FailureSink {
                            remaining,
                            fail_flush: false,
                        },
                        1,
                    )
                    .unwrap();
                    assert!(templates::render(&mut out, &config, text).await.is_err());
                }
            }
        }
        let mut out = HtmlWriter::new(FailureSink {
            remaining: usize::MAX,
            fail_flush: true,
        });
        let config = BenchmarkConfig {
            variant: Variant::Reference,
            ..small(Workload::Http)
        };
        assert!(templates::render(&mut out, &config, "text").await.is_err());
    });
}

#[test]
fn counting_sink_close_and_finite_consumer_yields() {
    let counter = Arc::new(WakeCount(AtomicUsize::new(0)));
    let wake = waker(counter.clone());
    let mut cx = Context::from_waker(&wake);
    let mut sink = OutputSink::default();
    assert_eq!(
        Pin::new(&mut sink).poll_close(&mut cx).map(Result::unwrap),
        Poll::Ready(())
    );
    let mut yields = Box::pin(yield_consumer(2));
    assert!(yields.as_mut().poll(&mut cx).is_pending());
    assert!(yields.as_mut().poll(&mut cx).is_pending());
    assert!(yields.as_mut().poll(&mut cx).is_ready());
    assert_eq!(counter.0.load(Ordering::Relaxed), 2);
}

struct WakeCount(AtomicUsize);
impl ArcWake for WakeCount {
    fn wake_by_ref(arc: &Arc<Self>) {
        arc.0.fetch_add(1, Ordering::Relaxed);
    }
}
