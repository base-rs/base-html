// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::HtmlWriter;
use futures_io::AsyncWrite;
use std::borrow::Cow;
use std::future::Future;
use std::io;

/// A text value that can render with backpressure without unbounded formatting storage.
///
/// Implementations must use escaped writer operations for text; this interface is application code, not a sanitizer for malicious implementations. Strings stream directly, primitives use fixed-size scratch storage, and custom implementations may await output or data. For markup components, use an ordinary async function and `@render` instead.
pub trait HtmlText {
    /// Render escaped text into the same bounded writer as the surrounding template.
    fn render_text<W: AsyncWrite + Unpin>(
        &self,
        out: &mut HtmlWriter<W>,
    ) -> impl Future<Output = io::Result<()>>;
}

impl HtmlText for str {
    async fn render_text<W: AsyncWrite + Unpin>(&self, out: &mut HtmlWriter<W>) -> io::Result<()> {
        out.write_escaped(self).await
    }
}

impl HtmlText for String {
    async fn render_text<W: AsyncWrite + Unpin>(&self, out: &mut HtmlWriter<W>) -> io::Result<()> {
        out.write_escaped(self).await
    }
}

impl HtmlText for Cow<'_, str> {
    async fn render_text<W: AsyncWrite + Unpin>(&self, out: &mut HtmlWriter<W>) -> io::Result<()> {
        out.write_escaped(self).await
    }
}

impl<T: HtmlText + ?Sized> HtmlText for &T {
    async fn render_text<W: AsyncWrite + Unpin>(&self, out: &mut HtmlWriter<W>) -> io::Result<()> {
        (*self).render_text(out).await
    }
}

macro_rules! primitive_text {
    ($($ty:ty),* $(,)?) => {
        $(impl HtmlText for $ty {
            async fn render_text<W: AsyncWrite + Unpin>(&self, out: &mut HtmlWriter<W>) -> io::Result<()> {
                out.write_display(self).await
            }
        })*
    };
}

primitive_text!(
    bool, char, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64
);
