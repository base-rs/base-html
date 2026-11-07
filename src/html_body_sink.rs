// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use bytes::Bytes;
use futures_io::AsyncWrite;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

/// The bounded destination supplied to the renderer by [`crate::html_body`].
///
/// One chunk, no larger than the configured capacity, can be queued. Rendering and consuming are polled by the same body, without a background producer. This type is constructed by `html_body`, not directly by applications.
pub struct HtmlBodySink {
    pub(crate) shared: Arc<Mutex<BodyState>>,
    pub(crate) capacity: usize,
}

pub(crate) struct BodyState {
    pub chunk: Option<Bytes>,
    pub writer_waker: Option<Waker>,
}

impl AsyncWrite for HtmlBodySink {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        if bytes.is_empty() {
            return Poll::Ready(Ok(0));
        }
        let waker = cx.waker().clone();
        let mut shared = self
            .shared
            .lock()
            .expect("body slot is never held across user code");
        if shared.chunk.is_some() {
            shared.writer_waker = Some(waker);
            return Poll::Pending;
        }
        let count = bytes.len().min(self.capacity);
        shared.chunk = Some(Bytes::copy_from_slice(&bytes[..count]));
        Poll::Ready(Ok(count))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::task::noop_waker;

    #[test]
    fn empty_writes_and_close_do_not_queue_frames() {
        let shared = Arc::new(Mutex::new(BodyState {
            chunk: None,
            writer_waker: None,
        }));
        let mut sink = HtmlBodySink {
            shared: shared.clone(),
            capacity: 4,
        };
        let waker = noop_waker();
        let mut cx = Context::from_waker(&waker);
        assert_eq!(
            Pin::new(&mut sink)
                .poll_write(&mut cx, b"")
                .map(Result::unwrap),
            Poll::Ready(0)
        );
        assert_eq!(
            Pin::new(&mut sink).poll_close(&mut cx).map(Result::unwrap),
            Poll::Ready(())
        );
        assert!(shared.lock().unwrap().chunk.is_none());
    }
}
