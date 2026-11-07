# Streaming HTML

There is one async-first rendering interface: `html!` returns a future, `HtmlWriter` buffers output, and components are ordinary async functions writing to the same destination. Async does not require a spawned task or an allocation for every write. The engine does not select a runtime.

## Progressive rendering

```rust
use base_html::{AsyncWrite, HtmlWriter, html};

async fn page<W: AsyncWrite + Unpin>(out: &mut HtmlWriter<W>) -> std::io::Result<()> {
    html! { out;
        h1 { "Users" }
        @flush;
        @let names = async { ["Ada", "Grace & friends"] }.await;
        ul {
            @for name in names {
                li { (name) }
            }
        }
    }.await
}
```

Replace the illustrative awaited expression with application data acquisition. The prefix is published before continuing past `@flush`. This does not guarantee immediate browser display: transports, compression, proxies, and browsers may buffer further. Ordinary Rust loops around smaller `html!` blocks can consume asynchronously arriving records without another template-specific loop syntax.

## Buffering and values

Use `HtmlWriter::with_capacity(destination, bytes)` to choose a nonzero output-buffer limit; `new` defaults to 8 KiB. Exactly-full buffers drain immediately, even if the next operation awaits data. Short writes and `Pending` preserve progress. Output chunks may split UTF-8 sequences or HTML entities; the concatenated output remains valid escaped UTF-8.

Strings are scanned and escaped without a whole-value staging allocation. Built-in numeric, character, and boolean values use a fixed 512-byte formatting buffer. Arbitrary `Display` is not accepted implicitly; `out.write_display(&value).await` is an explicit fallback with that same 512-byte limit. Overflow returns `InvalidInput`, and formatter failure returns `Other`, without publishing any of the failed value. Previously published output is unaffected. Implement `HtmlText` or an async component to render a larger custom value incrementally.

The writer retains at most its configured output-buffer storage plus the fixed formatting scratch when used, excluding metadata and application-owned input values. Interpolating an application-created `String` does not make that source string engine-owned output storage.

## HTTP response bodies

Enable the optional `http-body` Cargo feature. The result implements the standard `http_body::Body` trait; a web framework can adapt it without changing templates.

```rust
#[cfg(feature = "http-body")]
fn response() -> std::io::Result<impl http_body::Body<Data = bytes::Bytes, Error = std::io::Error>> {
    use base_html::{html, html_body};

    html_body(8192, async |out| {
        html! { out;
            h1 { "Users" }
            @flush;
            @let name = async { "Ada & Grace" }.await;
            p { (name) }
        }.await
    })
}
```

The body owns the rendering future and polls it on consumer demand. There is no spawned renderer or channel. One owned chunk, no larger than the configured capacity, can be queued alongside the writer's buffer. Buffered and queued output therefore uses at most twice the configured capacity, plus the fixed 512-byte formatting scratch when used and small bookkeeping overhead. Consumer-retained chunks, transport buffers, and application source data are outside that bound. The adapter copies bytes into each owned HTTP chunk; this is not a zero-copy claim.

Successful body completion flushes the final partial buffer automatically. A rendering failure yields any already queued chunk first, then one error, then termination. Headers and earlier output cannot be replaced after they have been sent. A failure discards still-unpublished partial output rather than reporting successful truncation.

Dropping the body immediately drops its renderer and owned pending data operations. This cancels owned futures, not independently spawned application tasks or external side effects. No background work survives in the adapter. Slow consumers stop rendering once bounded render-ahead is exhausted. Local/non-`Send` renderers and borrowed data are allowed; concrete bodies can also be `Send` when their renderer permits it.

## Synchronous destinations and cancellation

Wrap memory buffers or intentionally blocking command-line output in `BlockingWriter`. It adapts `std::io::Write` directly; it does not offload operations or make them nonblocking. For network output, use an actual async writer or the HTTP adapter. Tokio's `AsyncWrite` is a distinct trait from `futures_io::AsyncWrite`; use a compatibility adapter when integrating Tokio destinations directly.

Explicit `flush` retains the offset of accepted short writes, so cancelling and retrying a flush does not duplicate bytes. Cancellation of an entire interpolation is not transactional: restarting it can duplicate a previously emitted prefix. Treat errors and cancellation as termination of the rendering operation. Standalone writers require `finish().await` or `flush().await` on success; neither template completion nor writer drop implicitly publishes a partial buffer.
