use base_html::{BlockingWriter, HtmlWriter, html};

struct CustomDisplay;

impl std::fmt::Display for CustomDisplay {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("custom")
    }
}

fn main() {
    let mut out = HtmlWriter::new(BlockingWriter::new(Vec::new()));
    let _ = html! { out; (CustomDisplay) };
}
