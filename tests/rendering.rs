// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_html::{AsyncWrite, BlockingWriter, HtmlText, HtmlWriter, TrustedHtml, html};
use futures::executor::block_on;
use std::borrow::Cow;
use std::cell::Cell;
use std::fmt;
use std::io::{self, Write};

fn writer<W: Write + Unpin>(inner: W) -> HtmlWriter<BlockingWriter<W>> {
    HtmlWriter::new(BlockingWriter::new(inner))
}

macro_rules! async_test {
    ($name:ident, $body:block) => {
        #[test]
        fn $name() {
            block_on(async $body).unwrap();
        }
    };
}

async_test!(static_markup_is_coalesced_and_escaped, {
    let mut out = writer(CountingWriter::default());
    html! { out;
        @doctype;
        html {
            body {
                h1 title="\"&<>'" { "Hello & <world> \"' 日本語" }
                br;
                custom-element data-label="example" { "child" }
            }
        }
    }
    .await?;
    let output = out.finish().await?.into_inner();
    assert_eq!(output.writes, 1);
    assert_eq!(output.flushes, 1);
    assert_eq!(
        String::from_utf8(output.bytes).unwrap(),
        "<!DOCTYPE html><html><body><h1 title=\"&quot;&amp;&lt;&gt;&#39;\">Hello &amp; &lt;world&gt; &quot;&#39; 日本語</h1><br><custom-element data-label=\"example\">child</custom-element></body></html>"
    );
    Ok::<_, io::Error>(())
});

async_test!(dynamic_values_and_native_awaits, {
    let mut out = writer(Vec::new());
    let value = "日本語 & < > \" '";
    html! { out;
        p title=(async { value }.await) {
            (value)
            ({ let number = async { 20 }.await; number + 2 })
            (String::from("owned &"))
            (Cow::Borrowed("borrowed &"))
        }
    }
    .await?;
    assert_eq!(
        String::from_utf8(out.finish().await?.into_inner()).unwrap(),
        "<p title=\"日本語 &amp; &lt; &gt; &quot; &#39;\">日本語 &amp; &lt; &gt; &quot; &#39;22owned &amp;borrowed &amp;</p>"
    );
    Ok::<_, io::Error>(())
});

async fn item<W: AsyncWrite + Unpin>(out: &mut HtmlWriter<W>, value: &str) -> io::Result<()> {
    html! { out; li { (value) } }.await
}

async fn empty_component<W: AsyncWrite + Unpin>(out: &mut HtmlWriter<W>) -> io::Result<()> {
    html! { out; }.await
}

async_test!(components_control_flow_and_trusted_markup, {
    let mut out = writer(Vec::new());
    let trusted = TrustedHtml::new("<b>trusted</b>");
    html! { out;
        @let names = async { std::hint::black_box(["Ada", "Grace & friends"]) }.await;
        ul {
            @for name in async { names }.await {
                @render item(async { name }.await);
            }
            @render empty_component();
        }
        @if async { names.is_empty() }.await { "empty" }
        @else if names.len() == 2 { "two" }
        @else { "many" }
        @raw(async { trusted }.await);
    }
    .await?;
    assert_eq!(
        out.finish().await?.into_inner(),
        b"<ul><li>Ada</li><li>Grace &amp; friends</li></ul>two<b>trusted</b>"
    );
    assert_eq!(trusted.as_str(), "<b>trusted</b>");
    Ok::<_, io::Error>(())
});

async_test!(conditionals_allow_if_let_and_all_branches, {
    let mut out = writer(Vec::new());
    for number in [None, Some(0), Some(1), Some(2)] {
        html! { out;
            @if let Some(value) = number {
                @if value == 0 { "zero" }
                @else if value == 1 { "one" }
                @else { "other" }
            }
            @else { "none" }
        }
        .await?;
    }
    html! { out; @if false { "unused" } }.await?;
    assert_eq!(out.finish().await?.into_inner(), b"nonezerooneother");
    Ok::<_, io::Error>(())
});

async_test!(special_contexts_only_emit_static_bodies, {
    let mut out = writer(Vec::new());
    html! { out;
        script type="module" { "if (a < b && c > d) console.log('ok');" }
        style { "a::after { content: '&'; }" }
        textarea { "<example>" ("&") }
        svg viewBox="0 0 10 10" { path d="M0 0" {} }
        input type="text" value=("hello");
    }
    .await?;
    assert_eq!(
        String::from_utf8(out.finish().await?.into_inner()).unwrap(),
        "<script type=\"module\">if (a < b && c > d) console.log('ok');</script><style>a::after { content: '&'; }</style><textarea>&lt;example&gt;&amp;</textarea><svg viewBox=\"0 0 10 10\"><path d=\"M0 0\"></path></svg><input type=\"text\" value=\"hello\">"
    );
    Ok::<_, io::Error>(())
});

async_test!(future_is_lazy_and_values_are_evaluated_once_in_order, {
    let calls = Cell::new(0);
    let mut out = writer(Vec::new());
    let __base_html_writer = "user variable";
    let future = html! { { calls.set(calls.get() + 1); &mut out };
        p data-first=({ calls.set(calls.get() + 1); calls.get() }) {
            ({ calls.set(calls.get() + 1); calls.get() })
            (__base_html_writer)
        }
    };
    assert_eq!(calls.get(), 0);
    future.await?;
    assert_eq!(calls.get(), 3);
    assert_eq!(
        out.finish().await?.into_inner(),
        b"<p data-first=\"2\">3user variable</p>"
    );
    let mut out = writer(Vec::new());
    drop(html! { { calls.set(99); &mut out }; "never polled" });
    assert_eq!(calls.get(), 3);
    Ok::<_, io::Error>(())
});

async_test!(flush_is_an_explicit_output_barrier, {
    let mut out = writer(CountingWriter::default());
    html! { out; "prefix" @flush; @flush; "suffix" }.await?;
    assert_eq!(out.get_ref().get_ref().bytes, b"prefix");
    let output = out.finish().await?.into_inner();
    assert_eq!(output.bytes, b"prefixsuffix");
    assert_eq!(output.writes, 2);
    assert_eq!(output.flushes, 3);
    Ok::<_, io::Error>(())
});

async_test!(borrowed_destinations_and_custom_text, {
    let mut bytes = Vec::new();
    let mut out = writer(&mut bytes);
    out.write_text("first & second").await?;
    out.write_text(&CustomText).await?;
    out.flush().await?;
    assert_eq!(
        out.into_inner().into_inner(),
        b"first &amp; secondcustom &lt;text&gt;"
    );
    Ok::<_, io::Error>(())
});

struct CustomText;

impl HtmlText for CustomText {
    async fn render_text<W: AsyncWrite + Unpin>(&self, out: &mut HtmlWriter<W>) -> io::Result<()> {
        out.write_escaped("custom ").await?;
        out.write_text("<text>").await
    }
}

struct BadDisplay;

impl fmt::Display for BadDisplay {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        Err(fmt::Error)
    }
}

async_test!(formatting_is_bounded_and_failed_values_are_not_published, {
    let mut out = writer(Vec::new());
    let error = out.write_display(&BadDisplay).await.unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Other);
    assert_eq!(out.buffered_len(), 0);
    let error = out.write_display(&"x".repeat(513)).await.unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert_eq!(out.buffered_len(), 0);
    out.write_display(&"&".repeat(512)).await?;
    assert_eq!(
        out.finish().await?.into_inner(),
        "&amp;".repeat(512).as_bytes()
    );
    Ok::<_, io::Error>(())
});

async_test!(all_primitive_types_use_bounded_formatting, {
    let mut out = writer(Vec::new());
    html! { out;
        (true) ('&') (1u8) (2u16) (3u32) (4u64) (5u128) (6usize)
        (-1i8) (-2i16) (-3i32) (-4i64) (-5i128) (-6isize)
        (1.5f32) (2.5f64)
    }
    .await?;
    assert_eq!(
        out.finish().await?.into_inner(),
        b"true&amp;123456-1-2-3-4-5-61.52.5"
    );
    let mut out = writer(Vec::new());
    out.write_text(&f64::from_bits(1)).await?;
    assert_eq!(
        out.finish().await?.into_inner(),
        f64::from_bits(1).to_string().as_bytes()
    );
    Ok::<_, io::Error>(())
});

async_test!(empty_templates_and_values_do_not_write, {
    let mut out = writer(CountingWriter::default());
    html! { out; "" ("") }.await?;
    assert_eq!(out.into_inner().into_inner().writes, 0);
    Ok::<_, io::Error>(())
});

#[derive(Default)]
struct CountingWriter {
    bytes: Vec<u8>,
    writes: usize,
    flushes: usize,
}

impl Write for CountingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        Ok(())
    }
}
