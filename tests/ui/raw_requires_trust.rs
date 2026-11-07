// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_html::{BlockingWriter, HtmlWriter, html};

fn main() {
    let mut out = HtmlWriter::new(BlockingWriter::new(Vec::new()));
    let _ = html! { out; @raw("<b>not explicitly trusted</b>"); };
}
