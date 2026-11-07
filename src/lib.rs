// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Compiled HTML templates that write directly into a caller-selected destination.
//!
//! ```
//! use base_web_html::{BlockingWriter, HtmlWriter, html};
//!
//! # futures::executor::block_on(async {
//! let mut out = HtmlWriter::new(BlockingWriter::new(Vec::new()));
//! let name = "Ada & Grace";
//! html! { out;
//!     p class="greeting" { "Hello, " (name) "!" }
//! }.await?;
//! assert_eq!(out.finish().await?.into_inner(), b"<p class=\"greeting\">Hello, Ada &amp; Grace!</p>");
//! # Ok::<(), std::io::Error>(())
//! # }).unwrap();
//! ```
//!
//! The [Template Syntax and Rendering](https://github.com/base-rs/base-web-html/blob/main/context/templates.md) guide describes the template language, and the [Streaming HTML](https://github.com/base-rs/base-web-html/blob/main/context/streaming.md) guide describes buffering, flushing, HTTP bodies, and cancellation. Both are included in the package under `context/`.

mod blocking_writer;
#[cfg(feature = "http-body")]
mod html_body;
#[cfg(feature = "http-body")]
mod html_body_sink;
mod html_text;
mod html_writer;
mod trusted_html;

pub use base_web_html_macros::html;
pub use blocking_writer::BlockingWriter;
pub use futures_io::AsyncWrite;
#[cfg(feature = "http-body")]
pub use html_body::{HtmlBody, html_body};
#[cfg(feature = "http-body")]
pub use html_body_sink::HtmlBodySink;
pub use html_text::HtmlText;
pub use html_writer::HtmlWriter;
#[doc(hidden)]
pub use html_writer::TemplateGuard;
pub use trusted_html::TrustedHtml;

#[cfg(doctest)]
#[doc = include_str!("../context/templates.md")]
struct TemplatesGuide;

#[cfg(doctest)]
#[doc = include_str!("../context/streaming.md")]
struct StreamingGuide;

#[cfg(doctest)]
#[doc = include_str!("../readme.md")]
struct Readme;
