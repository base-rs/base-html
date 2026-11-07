// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::{BenchmarkConfig, Variant, Workload};
use base_web_html::{AsyncWrite, HtmlWriter, TrustedHtml, html};
use std::io;

pub(super) const STATIC_PAGE: &str = "<!DOCTYPE html><html><head><title>Benchmark</title></head><body><h1>Static page</h1><p>Fixed &amp; escaped text</p></body></html>";

pub(super) async fn render<W: AsyncWrite + Unpin>(
    out: &mut HtmlWriter<W>,
    config: &BenchmarkConfig,
    text: &str,
) -> io::Result<()> {
    match (config.workload, config.variant) {
        (Workload::Static, Variant::Engine) => html! { out; @doctype; html { head { title { "Benchmark" } } body { h1 { "Static page" } p { "Fixed & escaped text" } } } }.await,
        (Workload::Static, Variant::Reference) => out.write_literal(STATIC_PAGE).await,
        (Workload::Escape, Variant::Engine) => out.write_escaped(text).await,
        (Workload::Escape, Variant::Reference) => escape_reference(out, text).await,
        (Workload::List, _) => list(out, config, text).await,
        (Workload::Http, Variant::Engine) => html! { out; h1 { "Users" } @flush; @render list(config, text); }.await,
        (Workload::Http, Variant::Reference) => {
            out.write_literal("<h1>Users</h1>").await?;
            out.flush().await?;
            list(out, config, text).await
        }
    }
}

async fn list<W: AsyncWrite + Unpin>(
    out: &mut HtmlWriter<W>,
    config: &BenchmarkConfig,
    text: &str,
) -> io::Result<()> {
    match config.variant {
        Variant::Engine => {
            html! { out; ul class="users" { @for _ in 0..config.items { @render row(text); } } }
                .await
        }
        Variant::Reference => {
            out.write_literal("<ul class=\"users\">").await?;
            for _ in 0..config.items {
                reference_row(out, text).await?;
            }
            out.write_literal("</ul>").await
        }
    }
}

async fn row<W: AsyncWrite + Unpin>(out: &mut HtmlWriter<W>, text: &str) -> io::Result<()> {
    html! { out; li title=(text) { (text) } }.await
}

async fn reference_row<W: AsyncWrite + Unpin>(
    out: &mut HtmlWriter<W>,
    text: &str,
) -> io::Result<()> {
    out.write_literal("<li title=\"").await?;
    out.write_text(text).await?;
    out.write_literal("\">").await?;
    out.write_text(text).await?;
    out.write_literal("</li>").await
}

async fn escape_reference<W: AsyncWrite + Unpin>(
    out: &mut HtmlWriter<W>,
    text: &str,
) -> io::Result<()> {
    let mut start = 0;
    for (index, character) in text.char_indices() {
        let replacement = match character {
            '&' => "&amp;",
            '<' => "&lt;",
            '>' => "&gt;",
            '"' => "&quot;",
            '\'' => "&#39;",
            _ => continue,
        };
        // SECURITY: These slices bypass escaping, but each lies between the five escaped characters found above and so contains none of them; the input is generated benchmark text, and the preflight compares the output byte for byte with independently escaped output.
        out.write_trusted(TrustedHtml::new(&text[start..index]))
            .await?;
        out.write_literal(replacement).await?;
        start = index + character.len_utf8();
    }
    out.write_trusted(TrustedHtml::new(&text[start..])).await
}
