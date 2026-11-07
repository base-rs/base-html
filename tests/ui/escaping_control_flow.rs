use base_html::{BlockingWriter, HtmlWriter, html};

fn main() {
    let mut out = HtmlWriter::new(BlockingWriter::new(Vec::new()));
    let values = ["a", "skip", "b"];
    let _ = html! { out; ul { @for value in values { li title=({ if value == "skip" { continue; } value }) { (value) } } } };
}
