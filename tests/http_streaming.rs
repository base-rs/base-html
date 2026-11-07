// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

#![cfg(feature = "http-body")]

use base_html::{html, html_body};
use futures::executor::block_on;
use futures::task::noop_waker;
use http_body::Body;
use std::cell::Cell;
use std::future::{pending, poll_fn};
use std::io;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

struct DropFlag(Rc<Cell<bool>>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

struct PendingAcquisition {
    started: Rc<Cell<bool>>,
    dropped: Rc<Cell<bool>>,
}

impl Future for PendingAcquisition {
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        self.started.set(true);
        Poll::Pending
    }
}

impl Drop for PendingAcquisition {
    fn drop(&mut self) {
        self.dropped.set(true);
    }
}

#[test]
fn body_is_lazy_and_success_flushes_the_final_partial_chunk() -> io::Result<()> {
    block_on(async {
        let called = Cell::new(false);
        let mut body = html_body(16, async |out| {
            called.set(true);
            html! { out; p { "ok" } }.await
        })?;
        assert!(!called.get());
        assert!(!body.is_end_stream());
        let frame = poll_fn(|cx| Pin::new(&mut body).poll_frame(cx))
            .await
            .unwrap()?;
        assert_eq!(frame.into_data().unwrap(), b"<p>ok</p>".as_slice());
        assert!(called.get());
        assert!(body.is_end_stream());
        assert!(
            poll_fn(|cx| Pin::new(&mut body).poll_frame(cx))
                .await
                .is_none()
        );
        Ok(())
    })
}

#[test]
fn explicit_flush_delivers_a_prefix_while_acquisition_is_pending() -> io::Result<()> {
    let dropped = Rc::new(Cell::new(false));
    let started = Rc::new(Cell::new(false));
    let acquisition = PendingAcquisition {
        started: started.clone(),
        dropped: dropped.clone(),
    };
    let mut body = html_body(16, async move |out| {
        html! { out; "prefix" @flush; }.await?;
        acquisition.await
    })?;
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    let Poll::Ready(Some(Ok(frame))) = Pin::new(&mut body).poll_frame(&mut cx) else {
        panic!("prefix must be available");
    };
    assert_eq!(frame.into_data().unwrap(), b"prefix".as_slice());
    assert!(started.get());
    assert!(Pin::new(&mut body).poll_frame(&mut cx).is_pending());
    assert!(!dropped.get());
    drop(body);
    assert!(dropped.get());
    Ok(())
}

#[test]
fn exact_capacity_is_published_before_waiting_for_data() -> io::Result<()> {
    let mut body = html_body(4, async |out| {
        out.write_literal("abcd").await?;
        pending::<()>().await;
        Ok(())
    })?;
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    let Poll::Ready(Some(Ok(frame))) = Pin::new(&mut body).poll_frame(&mut cx) else {
        panic!("full buffer must be published");
    };
    assert_eq!(frame.into_data().unwrap(), b"abcd".as_slice());
    Ok(())
}

#[test]
fn slow_consumers_bound_render_ahead_and_dropping_cancels_output() -> io::Result<()> {
    let calls = Rc::new(Cell::new(0));
    let dropped = Rc::new(Cell::new(false));
    let guard = DropFlag(dropped.clone());
    let progress = calls.clone();
    let mut body = html_body(4, async move |out| {
        let _guard = guard;
        for _ in 0..1_000_000 {
            progress.set(progress.get() + 1);
            out.write_literal("abcd").await?;
        }
        Ok(())
    })?;
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    for expected in [2, 3, 4] {
        let Poll::Ready(Some(Ok(frame))) = Pin::new(&mut body).poll_frame(&mut cx) else {
            panic!("one bounded chunk must be ready");
        };
        assert_eq!(frame.into_data().unwrap().len(), 4);
        assert_eq!(calls.get(), expected);
    }
    // No renderer task exists to run ahead between consumer polls.
    let progress = calls.get();
    drop(body);
    assert!(dropped.get());
    assert_eq!(calls.get(), progress);
    Ok(())
}

#[test]
fn queued_prefix_precedes_one_error_then_termination() -> io::Result<()> {
    block_on(async {
        let mut body = html_body(4, async |out| {
            out.write_literal("abcd").await?;
            Err(io::Error::other("late failure"))
        })?;
        let frame = poll_fn(|cx| Pin::new(&mut body).poll_frame(cx))
            .await
            .unwrap()?;
        assert_eq!(frame.into_data().unwrap(), b"abcd".as_slice());
        assert!(!body.is_end_stream());
        let error = poll_fn(|cx| Pin::new(&mut body).poll_frame(cx))
            .await
            .unwrap()
            .unwrap_err();
        assert_eq!(error.to_string(), "late failure");
        assert!(body.is_end_stream());
        assert!(
            poll_fn(|cx| Pin::new(&mut body).poll_frame(cx))
                .await
                .is_none()
        );

        let mut body = html_body(4, async |_| Err(io::Error::other("early failure")))?;
        assert!(
            poll_fn(|cx| Pin::new(&mut body).poll_frame(cx))
                .await
                .unwrap()
                .is_err()
        );
        assert!(body.is_end_stream());
        Ok(())
    })
}

#[test]
fn output_can_be_send_but_local_renderers_and_borrows_are_supported() -> io::Result<()> {
    fn assert_send<T: Send>(_: &T) {}
    let name = String::from("Ada & Grace");
    let body = html_body(4, async move |out| html! { out; p { (name) } }.await)?;
    assert_send(&body);
    let local = Rc::new(Cell::new(false));
    let body = html_body(4, async |out| {
        local.set(true);
        html! { out; "local" }.await
    })?;
    drop(body);
    assert!(!local.get());
    assert!(html_body(0, async |_| Ok(())).is_err());
    Ok(())
}

#[test]
fn large_escaped_body_chunks_stay_within_the_limit() -> io::Result<()> {
    block_on(async {
        let source = "日本語&<>\"'".repeat(10_000);
        let expected = source
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#39;");
        let mut body = html_body(7, async |out| html! { out; (source) }.await)?;
        let mut result = Vec::new();
        while let Some(frame) = poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await {
            let bytes = frame?.into_data().unwrap();
            assert!(bytes.len() <= 7);
            result.extend_from_slice(&bytes);
        }
        assert_eq!(result, expected.as_bytes());
        Ok(())
    })
}

#[test]
fn acquisition_can_wake_and_resume_the_body() -> io::Result<()> {
    let (send, receive) = futures::channel::oneshot::channel::<&str>();
    let mut body = html_body(16, async move |out| {
        html! { out;
            "prefix" @flush;
            @let name = receive.await.map_err(io::Error::other)?;
            p { (name) }
        }
        .await
    })?;
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    assert!(matches!(
        Pin::new(&mut body).poll_frame(&mut cx),
        Poll::Ready(Some(Ok(_)))
    ));
    assert!(Pin::new(&mut body).poll_frame(&mut cx).is_pending());
    send.send("Ada & Grace").unwrap();
    block_on(async {
        let mut bytes = Vec::new();
        while let Some(frame) = poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await {
            bytes.extend_from_slice(&frame?.into_data().unwrap());
        }
        assert_eq!(bytes, b"<p>Ada &amp; Grace</p>");
        Ok(())
    })
}

#[test]
fn capacity_wakes_nested_future_combinators() -> io::Result<()> {
    block_on(async {
        use futures::stream::{FuturesUnordered, StreamExt};
        let mut body = html_body(3, async |out| {
            let mut writes = FuturesUnordered::new();
            writes.push(out.write_literal("abcdefghijkl"));
            while let Some(result) = writes.next().await {
                result?;
            }
            Ok(())
        })?;
        let mut bytes = Vec::new();
        while let Some(frame) = poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await {
            bytes.extend_from_slice(&frame?.into_data().unwrap());
        }
        assert_eq!(bytes, b"abcdefghijkl");
        Ok(())
    })
}
