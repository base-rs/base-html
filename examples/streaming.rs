// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_html::{html, html_body};
use http_body::Body;
use std::future::poll_fn;
use std::io::{self, Write};
use std::pin::Pin;

fn main() -> io::Result<()> {
    futures::executor::block_on(async {
        // A server can use this body directly. Here we consume its frames explicitly.
        let mut body = html_body(64, async |out| {
            html! { out;
                @doctype;
                h1 { "Users" }
                @flush;
                @let names = async { ["Ada", "Grace & friends"] }.await;
                ul {
                    @for name in names {
                        li { (name) }
                    }
                }
            }
            .await
        })?;

        let stdout = io::stdout();
        let mut stdout = stdout.lock();
        while let Some(frame) = poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await {
            let bytes = frame?.into_data().expect("HTML bodies emit data frames");
            stdout.write_all(&bytes)?;
        }
        stdout.flush()
    })
}
