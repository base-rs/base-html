// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_html::{BlockingWriter, HtmlWriter, html};
use std::hint::black_box;
use std::io::{self, Write};
use std::ops::AsyncFnMut;
use std::time::{Duration, Instant};

type BufferWriter = HtmlWriter<BlockingWriter<ReuseBuffer>>;

async fn template(out: &mut BufferWriter, users: &[&str]) -> io::Result<()> {
    html! { out;
        ul class="users" {
            @for user in users {
                li { (user) }
            }
        }
    }
    .await
}

async fn handwritten(out: &mut BufferWriter, users: &[&str]) -> io::Result<()> {
    out.write_literal("<ul class=\"users\">").await?;
    for user in users {
        out.write_literal("<li>").await?;
        out.write_text(user).await?;
        out.write_literal("</li>").await?;
    }
    out.write_literal("</ul>").await
}

fn measure(mut render: impl AsyncFnMut(&mut BufferWriter, &[&str]) -> io::Result<()>) -> Duration {
    let mut out = HtmlWriter::new(BlockingWriter::new(ReuseBuffer(Vec::with_capacity(4096))));
    let users = ["Ada & Grace", "Linus", "日本語", "<anonymous>"];
    let start = Instant::now();
    futures::executor::block_on(async {
        // Reuse both the destination and the engine's output buffer.
        for _ in 0..100_000 {
            render(&mut out, black_box(&users)).await.unwrap();
            out.flush().await.unwrap();
        }
    });
    start.elapsed()
}

struct ReuseBuffer(Vec<u8>);

impl Write for ReuseBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        black_box(&self.0);
        self.0.clear();
        Ok(())
    }
}

fn main() {
    // An exploratory comparison, not a statistical performance acceptance test.
    for _ in 0..5 {
        println!(
            "template: {:?}; handwritten: {:?} (100000 renders)",
            measure(template),
            measure(handwritten)
        );
    }
}
