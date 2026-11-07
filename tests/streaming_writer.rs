// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_html::{AsyncWrite, BlockingWriter, HtmlWriter, TrustedHtml, html};
use futures::executor::block_on;
use futures::io::AsyncWriteExt;
use futures::task::noop_waker;
use std::cell::{Cell, RefCell};
use std::io;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

#[derive(Debug, Default)]
struct State {
    bytes: Vec<u8>,
    write_polls: usize,
    flush_polls: usize,
    max_chunk: usize,
    short: Option<usize>,
    fail_after: Option<usize>,
    pause_after: Option<usize>,
    pending_writes: usize,
    pending_flushes: usize,
    interrupted: usize,
    zero: bool,
    fail_flush: bool,
}

#[derive(Clone, Debug, Default)]
struct TestSink(Rc<RefCell<State>>);

impl AsyncWrite for TestSink {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut state = self.0.borrow_mut();
        state.write_polls += 1;
        state.max_chunk = state.max_chunk.max(bytes.len());
        if state.pending_writes > 0 {
            state.pending_writes -= 1;
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        if state
            .pause_after
            .is_some_and(|limit| state.bytes.len() >= limit)
        {
            return Poll::Pending;
        }
        if state.interrupted > 0 {
            state.interrupted -= 1;
            return Poll::Ready(Err(io::ErrorKind::Interrupted.into()));
        }
        if state.zero {
            return Poll::Ready(Ok(0));
        }
        let remaining = state
            .fail_after
            .map_or(usize::MAX, |limit| limit - state.bytes.len());
        if remaining == 0 {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "destination closed",
            )));
        }
        let count = bytes
            .len()
            .min(state.short.unwrap_or(usize::MAX))
            .min(remaining);
        state.bytes.extend_from_slice(&bytes[..count]);
        Poll::Ready(Ok(count))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let mut state = self.0.borrow_mut();
        state.flush_polls += 1;
        if state.pending_flushes > 0 {
            state.pending_flushes -= 1;
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        if state.fail_flush {
            return Poll::Ready(Err(io::Error::other("flush failed")));
        }
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}

#[test]
fn large_unicode_values_stream_with_tiny_buffers() -> io::Result<()> {
    block_on(async {
        let source = "日本語 &<>\"' plain text ".repeat(10_000);
        let expected = source
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#39;");
        for capacity in [1, 2, 7, 32] {
            let sink = TestSink::default();
            sink.0.borrow_mut().pending_writes = 3;
            sink.0.borrow_mut().short = Some(3);
            let mut out = HtmlWriter::with_capacity(sink.clone(), capacity)?;
            assert_eq!(out.capacity(), capacity);
            out.write_text(source.as_str()).await?;
            assert!(out.buffered_len() <= capacity);
            out.finish().await?;
            assert_eq!(sink.0.borrow().bytes, expected.as_bytes());
            assert!(sink.0.borrow().max_chunk <= capacity);
        }
        Ok(())
    })
}

#[test]
fn cancelling_and_resuming_a_partial_flush_does_not_duplicate_bytes() -> io::Result<()> {
    let sink = TestSink::default();
    sink.0.borrow_mut().short = Some(2);
    sink.0.borrow_mut().pause_after = Some(2);
    let mut out = HtmlWriter::with_capacity(sink.clone(), 8)?;
    block_on(out.write_literal("abcdef"))?;
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    let mut flush = Box::pin(out.flush());
    assert!(flush.as_mut().poll(&mut cx).is_pending());
    drop(flush);
    assert_eq!(sink.0.borrow().bytes, b"ab");
    assert_eq!(out.buffered_len(), 4);
    sink.0.borrow_mut().pause_after = None;
    block_on(out.flush())?;
    assert_eq!(out.buffered_len(), 0);
    assert_eq!(sink.0.borrow().bytes, b"abcdef");
    Ok(())
}

#[test]
fn full_buffers_publish_without_a_later_write_or_explicit_flush() -> io::Result<()> {
    block_on(async {
        let sink = TestSink::default();
        sink.0.borrow_mut().interrupted = 2;
        sink.0.borrow_mut().pending_flushes = 3;
        let mut out = HtmlWriter::with_capacity(sink.clone(), 4)?;
        out.write_literal("abcd").await?;
        assert_eq!(sink.0.borrow().bytes, b"abcd");
        assert_eq!(out.buffered_len(), 0);
        out.flush().await?;
        assert_eq!(sink.0.borrow().flush_polls, 4);
        Ok(())
    })
}

#[test]
fn output_failures_stop_expressions_and_keep_the_original_error() {
    block_on(async {
        for fail_after in [0, 3, 4, 8, 13] {
            let sink = TestSink::default();
            sink.0.borrow_mut().fail_after = Some(fail_after);
            let mut out = HtmlWriter::with_capacity(sink.clone(), 1).unwrap();
            let calls = Cell::new(0);
            let error = html! { out;
                p { ("a&b") ({ calls.set(1); "unreached" }) }
            }
            .await
            .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
            assert_eq!(error.to_string(), "destination closed");
            if fail_after < 10 {
                assert_eq!(calls.get(), 0);
            }
            assert_eq!(sink.0.borrow().bytes.len(), fail_after);
        }
    });
}

#[test]
fn zero_writes_flush_errors_and_finish_errors_propagate() -> io::Result<()> {
    block_on(async {
        let sink = TestSink::default();
        sink.0.borrow_mut().zero = true;
        let mut out = HtmlWriter::with_capacity(sink.clone(), 1)?;
        assert_eq!(
            out.write_literal("x").await.unwrap_err().kind(),
            io::ErrorKind::WriteZero
        );

        let sink = TestSink::default();
        sink.0.borrow_mut().fail_flush = true;
        let mut out = HtmlWriter::new(sink.clone());
        let reached = Cell::new(false);
        let error = html! { out; "prefix" @flush; ({ reached.set(true); "suffix" }) }
            .await
            .unwrap_err();
        assert!(!reached.get());
        assert_eq!(error.to_string(), "flush failed");
        assert_eq!(sink.0.borrow().bytes, b"prefix");
        assert_eq!(out.finish().await.unwrap_err().to_string(), "flush failed");
        let sink = TestSink::default();
        sink.0.borrow_mut().fail_after = Some(0);
        let mut out = HtmlWriter::new(sink);
        out.write_literal("buffered").await?;
        assert_eq!(
            out.flush().await.unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
        assert_eq!(
            out.finish().await.unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
        Ok(())
    })
}

#[test]
fn failures_during_text_and_trusted_markup_propagate() -> io::Result<()> {
    block_on(async {
        for fail_after in [0, 1, 6] {
            let sink = TestSink::default();
            sink.0.borrow_mut().fail_after = Some(fail_after);
            let mut out = HtmlWriter::with_capacity(sink, 1)?;
            assert_eq!(
                out.write_escaped("a&b").await.unwrap_err().kind(),
                io::ErrorKind::BrokenPipe
            );
        }
        let sink = TestSink::default();
        sink.0.borrow_mut().fail_after = Some(0);
        let mut out = HtmlWriter::with_capacity(sink, 1)?;
        assert!(out.write_display(&42).await.is_err());
        assert!(
            out.write_trusted(TrustedHtml::new("<b>hi</b>"))
                .await
                .is_err()
        );
        Ok(())
    })
}

#[test]
fn dropping_a_buffered_or_backpressured_writer_performs_no_io() -> io::Result<()> {
    let sink = TestSink::default();
    let mut out = HtmlWriter::with_capacity(sink.clone(), 64)?;
    block_on(out.write_literal("buffered"))?;
    drop(out);
    assert_eq!(sink.0.borrow().write_polls, 0);
    assert_eq!(sink.0.borrow().flush_polls, 0);

    sink.0.borrow_mut().pause_after = Some(0);
    let mut out = HtmlWriter::with_capacity(sink.clone(), 4)?;
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    let mut render = Box::pin(out.write_escaped("long & output"));
    assert!(render.as_mut().poll(&mut cx).is_pending());
    drop(render);
    assert_eq!(out.buffered_len(), 4);
    let polls = sink.0.borrow().write_polls;
    drop(out);
    assert_eq!(sink.0.borrow().write_polls, polls);
    assert_eq!(sink.0.borrow().flush_polls, 0);
    Ok(())
}

#[test]
fn zero_capacity_is_rejected_and_blocking_adapter_can_close() -> io::Result<()> {
    assert!(HtmlWriter::with_capacity(TestSink::default(), 0).is_err());
    block_on(async {
        let mut sink = BlockingWriter::new(Vec::new());
        sink.write_all(b"bytes").await?;
        sink.close().await?;
        assert_eq!(sink.into_inner(), b"bytes");
        Ok(())
    })
}
