# `base-html`

An experimental, async-first Rust HTML template engine focused on streaming and reliable agent-authored code. Templates compile into escaped writes, with bounded buffering and no runtime parser, DOM, or intermediate component strings.

## Usage

Add the crate with `cargo add base-html`, or run `cargo run --example users` from this checkout.

```rust
use base_html::{BlockingWriter, HtmlWriter, html};

async fn render_users() -> std::io::Result<Vec<u8>> {
    let names = ["Ada", "Grace & friends"];
    let mut out = HtmlWriter::new(BlockingWriter::new(Vec::new()));

    html! { out;
        ul class="users" {
            @for name in names {
                li { (name) }
            }
        }
    }.await?;

    Ok(out.finish().await?.into_inner())
}
```

Use ordinary async Rust functions for components. `@render user_row(name);` awaits the function with the current writer as its first argument. `@flush;` publishes a buffered prefix before awaiting more data. There is one rendering interface, including for memory buffers; `BlockingWriter` explicitly adapts potentially blocking synchronous destinations.

The optional `http-body` feature provides a pull-based response body with bounded render-ahead and cancellation, without a background task. Try `cargo run --example streaming --features http-body`. See the [template guide](context/templates.md) and [streaming guide](context/streaming.md).

HTML escaping does not validate URL schemes or sanitize arbitrary markup. Scripts, styles, and other special contexts have additional restrictions described in the template guide.

## Releasing

Prepare a release with `cargo bake cargo:release:patch` (or `minor`, `major`, or `bump --version X.Y.Z`), which commits the version bump with a `Release: vVERSION` trailer, validates it, and opens a pull request. After review and merge, GitHub Actions publishes both crates through the `crates-io` environment. Follow the shared [Releasing skill](https://github.com/base-rs/base-project/blob/main/context/releasing.md).

## Releases

See [releases.md](releases.md).

## See Also

- [Maud](https://maud.lambda.xyz/) and [Hypertext](https://docs.rs/hypertext/), related Rust macro template engines.
- [XRB](https://github.com/socketry/xrb), the Ruby inspiration.

## Contributing

Please open an issue or pull request on [GitHub](https://github.com/base-rs/base-html).

Install the [Bake launcher](https://github.com/base-rs/bake) and run `cargo bake test`. Use `cargo bake test:coverage --all-targets true --all-features true` for the shared source-region coverage gate including HTTP streaming. See the [benchmark suite](benchmarks/rendering/readme.md) for parameterized workloads and collection commands, and the [performance record](context/performance.md) for historical observations. The older `cargo bench --bench render` remains an exploratory microbenchmark, not a controlled measurement campaign.

### Agent Context

Run `cargo bake agent:context:install` to install shared context and skills. Use `cargo bake agent:context:list` and `cargo bake agent:context:show` to discover relevant guides, follow repository guidance in `.agents/`, and apply skills under `.agents/skills/`. The installer preserves repository-owned `agents.md`; it does not create or regenerate that file.

Before planning or changing behavior, discover and read applicable `design/*.md`. Use `base-design-review` to assess plans and changes, and `base-design-author` when user requirements are introduced, changed, or removed. Review design compliance before opening or updating a pull request.
