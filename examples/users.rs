// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_web_html::{AsyncWrite, BlockingWriter, HtmlWriter, html};
use std::io;

async fn user_row<W: AsyncWrite + Unpin>(out: &mut HtmlWriter<W>, name: &str) -> io::Result<()> {
    html! { out; li class="user" { (name) } }.await
}

fn main() -> io::Result<()> {
    futures::executor::block_on(async {
        let stdout = io::stdout();
        let mut out = HtmlWriter::new(BlockingWriter::new(stdout.lock()));
        let users = ["Ada", "Grace & friends", "<anonymous>"];
        html! { out;
            @doctype;
            html lang="en" {
                head { title { "Users" } }
                body {
                    h1 { "Users" }
                    ul {
                        @for user in users {
                            @render user_row(user);
                        }
                    }
                }
            }
        }
        .await?;
        out.flush().await
    })
}
