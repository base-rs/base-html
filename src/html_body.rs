// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::html_body_sink::BodyState;
use crate::{HtmlBodySink, HtmlWriter};
use bytes::Bytes;
use http_body::{Body, Frame};
use std::future::Future;
use std::io;
use std::ops::AsyncFnOnce;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

/// A demand-driven HTTP response body owning its rendering future.
///
/// Requires the `http-body` feature. Construct with [`html_body`]. Dropping the body cancels rendering by dropping the future; no task or output queue survives it. The body need not be `Send` or `'static`, but can be when its concrete renderer and captures permit it.
pub struct HtmlBody<F> {
    renderer: Option<Pin<Box<F>>>,
    shared: Arc<Mutex<BodyState>>,
    error: Option<io::Error>,
}

/// Create an HTTP body from an async closure borrowing a bounded HTML writer.
///
/// The closure is not called until the body is polled. Successful rendering automatically flushes its final output. At most `2 * capacity` output bytes are buffered or queued internally, plus a fixed 512-byte formatting scratch buffer when needed. Application source data, consumer-retained frames, and transport buffers are excluded.
///
/// Errors follow any already queued prefix, then the body terminates. A response can therefore fail after its headers and initial markup have been sent. Buffer limits do not imply that browsers or intermediaries immediately display flushed output.
pub fn html_body<R>(
    capacity: usize,
    render: R,
) -> io::Result<HtmlBody<impl Future<Output = io::Result<()>>>>
where
    R: AsyncFnOnce(&mut HtmlWriter<HtmlBodySink>) -> io::Result<()>,
{
    let shared = Arc::new(Mutex::new(BodyState {
        chunk: None,
        writer_waker: None,
    }));
    let sink = HtmlBodySink {
        shared: shared.clone(),
        capacity,
    };
    let mut out = HtmlWriter::with_capacity(sink, capacity)?;
    let renderer = async move {
        render(&mut out).await?;
        // A renderer that abandoned a template and then ignored the refused writes must not end as a successful, truncated body.
        if out.is_abandoned() {
            return Err(io::Error::other(
                "a template in the HTML body was cancelled or failed part-way; the body is incomplete",
            ));
        }
        out.flush().await
    };
    Ok(HtmlBody {
        renderer: Some(Box::pin(renderer)),
        shared,
        error: None,
    })
}

impl<F: Future<Output = io::Result<()>>> Body for HtmlBody<F> {
    type Data = Bytes;
    type Error = io::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, io::Error>>> {
        let this = self.get_mut();
        if let Some(renderer) = this.renderer.as_mut()
            && let Poll::Ready(result) = renderer.as_mut().poll(cx)
        {
            this.renderer = None;
            this.error = result.err();
        }
        let (chunk, writer_waker) = {
            let mut shared = this
                .shared
                .lock()
                .expect("body slot is never held across user code");
            (shared.chunk.take(), shared.writer_waker.take())
        };
        if let Some(waker) = writer_waker {
            waker.wake();
        }
        if let Some(chunk) = chunk {
            return Poll::Ready(Some(Ok(Frame::data(chunk))));
        }
        if let Some(error) = this.error.take() {
            return Poll::Ready(Some(Err(error)));
        }
        if this.renderer.is_none() {
            Poll::Ready(None)
        } else {
            Poll::Pending
        }
    }

    fn is_end_stream(&self) -> bool {
        self.renderer.is_none()
            && self.error.is_none()
            && self
                .shared
                .lock()
                .expect("body slot is never held across user code")
                .chunk
                .is_none()
    }
}
