// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::{HtmlText, TrustedHtml};
use futures_io::AsyncWrite;
use std::fmt::{self, Write as _};
use std::future::poll_fn;
use std::io;
use std::pin::Pin;

/// A bounded, buffered HTML writer that suspends on downstream capacity.
///
/// No executor or web framework is selected. Dropping the writer discards its pending buffer; it never flushes implicitly or starts background work. Finish successful rendering with [`Self::finish`] or [`Self::flush`].
pub struct HtmlWriter<W> {
    inner: W,
    buffer: Vec<u8>,
    capacity: usize,
    written: usize,
}

impl<W: AsyncWrite + Unpin> HtmlWriter<W> {
    /// Use an 8 KiB output buffer.
    pub fn new(inner: W) -> Self {
        Self::with_capacity(inner, 8192).expect("default capacity is nonzero")
    }

    /// Use an explicitly bounded output buffer. Zero capacity is rejected.
    pub fn with_capacity(inner: W, capacity: usize) -> io::Result<Self> {
        if capacity == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "HTML buffer capacity must be nonzero",
            ));
        }
        Ok(Self {
            inner,
            buffer: Vec::with_capacity(capacity),
            capacity,
            written: 0,
        })
    }

    /// Reborrow the writer for a template or component.
    pub fn by_ref(&mut self) -> &mut Self {
        self
    }

    /// The configured maximum number of buffered bytes.
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// The number of buffered bytes not yet accepted by the destination.
    pub fn buffered_len(&self) -> usize {
        self.buffer.len() - self.written
    }

    /// Inspect the destination without bypassing buffered output ordering.
    pub const fn get_ref(&self) -> &W {
        &self.inner
    }

    /// Return the destination, discarding any pending output without flushing.
    pub fn into_inner(self) -> W {
        self.inner
    }

    /// Flush buffered output and return the destination, without closing it.
    pub async fn finish(mut self) -> io::Result<W> {
        self.flush().await?;
        Ok(self.inner)
    }

    /// Write compiler-generated static markup without escaping.
    pub async fn write_literal(&mut self, value: &'static str) -> io::Result<()> {
        self.write_bytes(value.as_bytes()).await
    }

    /// Render an async text value, escaping HTML-sensitive characters.
    ///
    /// There is intentionally no blanket `Display` implementation: synchronous formatters cannot suspend on backpressure. Use [`Self::write_display`] for an explicitly size-limited formatting fallback.
    pub async fn write_text<T: HtmlText + ?Sized>(&mut self, value: &T) -> io::Result<()> {
        value.render_text(self).await
    }

    /// Escape a borrowed string incrementally, without a document-sized allocation.
    ///
    /// Escapes `&`, `<`, `>`, `"`, and `'`, not JavaScript, CSS, or URL schemes. Chunks may split UTF-8 sequences or HTML entities; their concatenation is the escaped UTF-8 document.
    pub async fn write_escaped(&mut self, value: &str) -> io::Result<()> {
        let mut start = 0;
        for (index, byte) in value.bytes().enumerate() {
            let replacement: &[u8] = match byte {
                b'&' => b"&amp;",
                b'<' => b"&lt;",
                b'>' => b"&gt;",
                b'"' => b"&quot;",
                b'\'' => b"&#39;",
                _ => continue,
            };
            self.write_bytes(&value.as_bytes()[start..index]).await?;
            self.write_bytes(replacement).await?;
            start = index + 1;
        }
        self.write_bytes(&value.as_bytes()[start..]).await
    }

    /// Format into a fixed 512-byte scratch buffer, then escape the completed value.
    ///
    /// Overflow returns `InvalidInput`; formatter failures return `Other`. No part of the value is published before formatting succeeds. Large strings should use [`Self::write_escaped`], not this bounded fallback.
    pub async fn write_display(&mut self, value: &(impl fmt::Display + ?Sized)) -> io::Result<()> {
        let mut scratch = FormatBuffer {
            bytes: [0; 512],
            length: 0,
            overflowed: false,
        };
        let result = write!(&mut scratch, "{value}");
        if scratch.overflowed {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "HTML Display value exceeds 512-byte formatting limit",
            ));
        }
        result.map_err(|_| io::Error::other("HTML value formatting failed"))?;
        let text = std::str::from_utf8(&scratch.bytes[..scratch.length])
            .expect("fmt::Write receives valid UTF-8");
        self.write_escaped(text).await
    }

    /// Write markup explicitly marked as trusted, without escaping.
    pub async fn write_trusted(&mut self, value: TrustedHtml<'_>) -> io::Result<()> {
        self.write_bytes(value.as_str().as_bytes()).await
    }

    /// Publish the buffered prefix and flush the destination.
    ///
    /// Progress through short writes is retained in the writer, so cancelling and retrying a flush does not duplicate already accepted bytes. Cancelling and restarting an entire interpolation is not transactional.
    pub async fn flush(&mut self) -> io::Result<()> {
        self.drain().await?;
        poll_fn(|cx| Pin::new(&mut self.inner).poll_flush(cx)).await
    }

    async fn write_bytes(&mut self, mut value: &[u8]) -> io::Result<()> {
        while !value.is_empty() {
            let count = value.len().min(self.capacity - self.buffer.len());
            self.buffer.extend_from_slice(&value[..count]);
            value = &value[count..];
            if self.buffer.len() == self.capacity {
                self.drain().await?;
            }
        }
        Ok(())
    }

    async fn drain(&mut self) -> io::Result<()> {
        while self.written < self.buffer.len() {
            let result = poll_fn(|cx| {
                Pin::new(&mut self.inner).poll_write(cx, &self.buffer[self.written..])
            })
            .await;
            match result {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "HTML destination accepted zero bytes",
                    ));
                }
                Ok(count) => self.written += count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        self.buffer.clear();
        self.written = 0;
        Ok(())
    }
}

struct FormatBuffer {
    bytes: [u8; 512],
    length: usize,
    overflowed: bool,
}

impl fmt::Write for FormatBuffer {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if value.len() > self.bytes.len() - self.length {
            self.overflowed = true;
            return Err(fmt::Error);
        }
        self.bytes[self.length..self.length + value.len()].copy_from_slice(value.as_bytes());
        self.length += value.len();
        Ok(())
    }
}
