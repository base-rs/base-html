// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use futures_io::AsyncWrite;
use std::io::{self, Write};
use std::pin::Pin;
use std::task::{Context, Poll};

/// Explicitly adapt a synchronous destination to the async rendering interface.
///
/// Writes run immediately on the polling thread. This is useful for memory buffers and command-line output; files or sockets can block an executor. It does not offload work, spawn a task, or make a blocking destination nonblocking.
pub struct BlockingWriter<W> {
    inner: W,
}

impl<W> BlockingWriter<W> {
    /// Wrap a destination, explicitly accepting its potentially blocking behavior.
    pub const fn new(inner: W) -> Self {
        Self { inner }
    }

    /// Return the destination without flushing it.
    pub fn into_inner(self) -> W {
        self.inner
    }

    /// Inspect the synchronous destination.
    pub const fn get_ref(&self) -> &W {
        &self.inner
    }
}

impl<W: Write + Unpin> AsyncWrite for BlockingWriter<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(self.inner.write(bytes))
    }

    fn poll_flush(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(self.inner.flush())
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}
