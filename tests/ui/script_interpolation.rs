use base_html::{BlockingWriter, HtmlWriter, html};

fn main() {
    let mut out = HtmlWriter::new(BlockingWriter::new(Vec::new()));
    let value = "untrusted";
    let _ = html! { out; script { (value) } };
}
