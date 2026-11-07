// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

mod output;
mod templates;

use crate::{BenchmarkConfig, Corpus, Variant, Workload};
use base_benchmark::{CurrentProcess, Interval, Metric, ProcessRecord, ResourceSource, Unit};
use base_html::{HtmlWriter, html_body};
use futures::executor::block_on;
use http_body::Body;
use output::{OutputSink, OutputStats};
use std::future::poll_fn;
use std::hint::black_box;
use std::io;
use std::pin::Pin;
use std::task::Poll;
use std::time::Instant;

/// The stable workload identity in every record.
pub(crate) const WORKLOAD: &str = "base-html/rendering";

pub(crate) fn run(config: &BenchmarkConfig) -> io::Result<ProcessRecord> {
    run_with(config, CurrentProcess, |preflight, text| {
        block_on(preflight_output(preflight, text))
    })
}

/// Run a configuration, rendering its preflights with `render`, which tests replace to exercise mismatches.
fn run_with(
    config: &BenchmarkConfig,
    usage: impl ResourceSource,
    mut render: impl FnMut(&BenchmarkConfig, &str) -> OutputStats,
) -> io::Result<ProcessRecord> {
    let capacity = output_capacity(config)?;
    let text = source_text(config.corpus, config.text_bytes)?;
    let expected = expected_output(config, &text, capacity)?;
    // Mandatory independent full-byte preflight for both paths, outside the measured interval.
    preflight(&expected, |variant| {
        let preflight = BenchmarkConfig {
            variant,
            ..config.clone()
        };
        render(&preflight, &text)
    })?;
    let checksum = output::checksum(&expected);
    let mut out = HtmlWriter::with_capacity(OutputSink::default(), config.buffer_bytes)
        .expect("validated capacity is nonzero");
    block_on(async {
        for _ in 0..config.warmup {
            let stats = operation(config, &text, &mut out).await;
            assert_eq!(
                stats.bytes,
                expected.len(),
                "warmup output length changed after preflight"
            );
        }
    });
    let interval = Interval::start_with(usage)?;
    let mut events = 0_u64;
    let mut first_frame_ns = 0_u128;
    block_on(async {
        for _ in 0..config.iterations {
            let stats = operation(config, black_box(&text), &mut out).await;
            assert_eq!(
                stats.bytes,
                expected.len(),
                "timed output length changed after preflight"
            );
            events += stats.events;
            first_frame_ns += stats.first_frame_ns;
            black_box(stats.bytes);
        }
    });
    let mut sample = interval
        .finish(config.iterations)?
        .with(Metric::total(
            // SECURITY: These totals are unchecked, but each iteration verifies it produced `expected.len()` bytes, so wrapping would need more than 2^64 rendered bytes, and the parameters are trusted operator arguments.
            "output_bytes",
            Unit::Bytes,
            expected.len() as u64 * config.iterations,
        ))
        .with(Metric::total("sink_events", Unit::Count, events));
    if config.workload == Workload::Http {
        let first_frame_ns =
            u64::try_from(first_frame_ns).expect("first-frame time fits in u64 nanoseconds");
        sample.push(
            Metric::total("first_frame_time", Unit::Nanoseconds, first_frame_ns).lower_is_better(),
        );
    }
    let parameters = serde_json::to_value(config).expect("configurations serialize");
    Ok(ProcessRecord::new(WORKLOAD, parameters)
        .with_signature(format!("{}:{checksum:016x}", expected.len()))
        .with_sample(sample))
}

async fn operation(
    config: &BenchmarkConfig,
    text: &str,
    out: &mut HtmlWriter<OutputSink>,
) -> OutputStats {
    if config.workload == Workload::Http {
        http_operation(config, text, false).await
    } else {
        templates::render(out, config, text)
            .await
            .expect("counting sink and string rendering cannot fail");
        out.flush()
            .await
            .expect("counting sink cannot fail to flush");
        out.get_ref().take()
    }
}

/// Check that both variants produce exactly the expected bytes with the same output boundaries.
fn preflight(expected: &[u8], mut render: impl FnMut(Variant) -> OutputStats) -> io::Result<()> {
    let engine = render(Variant::Engine);
    output::validate(&engine, expected)?;
    let reference = render(Variant::Reference);
    output::validate(&reference, expected)?;
    output::validate_frames(&reference.frame_sizes, &engine.frame_sizes)
}

async fn preflight_output(config: &BenchmarkConfig, text: &str) -> OutputStats {
    if config.workload == Workload::Http {
        http_operation(config, text, true).await
    } else {
        let mut out = HtmlWriter::with_capacity(OutputSink::collecting(), config.buffer_bytes)
            .expect("validated capacity is nonzero");
        templates::render(&mut out, config, text)
            .await
            .expect("in-memory preflight rendering cannot fail");
        out.flush()
            .await
            .expect("in-memory sink cannot fail to flush");
        out.into_inner().take()
    }
}

async fn http_operation(config: &BenchmarkConfig, text: &str, collect: bool) -> OutputStats {
    // Includes body construction, frame consumption, final flush, and body destruction.
    let start = Instant::now();
    let mut body = html_body(config.buffer_bytes, async |out| {
        templates::render(out, config, text).await
    })
    .expect("validated capacity is nonzero");
    let mut stats = OutputStats::new(collect);
    while let Some(frame) = poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await {
        let bytes = frame
            .expect("in-process HTML body renderer cannot fail")
            .into_data()
            .expect("HTML bodies emit only data frames");
        if stats.events == 0 {
            stats.first_frame_ns = start.elapsed().as_nanos();
        }
        stats.observe(&bytes);
        drop(bytes);
        yield_consumer(config.consumer_yields).await;
    }
    stats
}

async fn yield_consumer(count: usize) {
    for _ in 0..count {
        let mut yielded = false;
        poll_fn(|cx| {
            if yielded {
                Poll::Ready(())
            } else {
                yielded = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        })
        .await;
    }
}

fn source_text(corpus: Corpus, bytes: usize) -> io::Result<String> {
    let pattern = match corpus {
        Corpus::Plain => "Ada Grace plain text ",
        Corpus::Escaped => "&<>\"'",
        Corpus::Unicode => "日本語🦀 &<>\"' ",
    };
    let mut output = String::new();
    output.try_reserve_exact(bytes).map_err(io::Error::other)?;
    for character in pattern.chars().cycle() {
        if output.len() + character.len_utf8() > bytes {
            break;
        }
        output.push(character);
    }
    while output.len() < bytes {
        output.push('x');
    }
    Ok(output)
}

fn output_capacity(config: &BenchmarkConfig) -> io::Result<usize> {
    let size = match config.workload {
        Workload::Static => Some(templates::STATIC_PAGE.len()),
        Workload::Escape => config.text_bytes.checked_mul(6),
        Workload::List | Workload::Http => config
            .text_bytes
            .checked_mul(12)
            .and_then(|size| size.checked_add(18))
            .and_then(|row| row.checked_mul(config.items))
            .and_then(|size| size.checked_add(64)),
    };
    size.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "document size overflows usize"))
}

fn expected_output(config: &BenchmarkConfig, text: &str, capacity: usize) -> io::Result<Vec<u8>> {
    let mut value = String::new();
    value
        .try_reserve_exact(capacity)
        .map_err(io::Error::other)?;
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;");
    match config.workload {
        Workload::Static => value.push_str(templates::STATIC_PAGE),
        Workload::Escape => value.push_str(&escaped),
        Workload::List | Workload::Http => {
            if config.workload == Workload::Http {
                value.push_str("<h1>Users</h1>");
            }
            value.push_str("<ul class=\"users\">");
            for _ in 0..config.items {
                value.push_str("<li title=\"");
                value.push_str(&escaped);
                value.push_str("\">");
                value.push_str(&escaped);
                value.push_str("</li>");
            }
            value.push_str("</ul>");
        }
    }
    Ok(value.into_bytes())
}

#[cfg(all(test, unix))]
mod tests;
