# Template Syntax and Rendering

Templates live in ordinary `.rs` files. The `html!` macro takes an `HtmlWriter` destination, a semicolon, and template content. It returns a borrowing future producing `std::io::Result<()>`; call `.await` to render. The destination is evaluated once, when the future is first polled, and borrowed for rendering. Creating or dropping an unpolled template future performs no rendering work. Static template whitespace is not output; write spaces explicitly inside text literals.

## Elements and values

```rust
use base_web_html::{BlockingWriter, HtmlWriter, html};

async fn greeting_page() -> std::io::Result<Vec<u8>> {
    let mut out = HtmlWriter::new(BlockingWriter::new(Vec::new()));
    let name = "Ada & Grace";
    html! { out;
        @doctype;
        div class="greeting" data-name=(name) {
            "Hello, " (name) "!"
            br;
        }
    }.await?;
    Ok(out.finish().await?.into_inner())
}
```

Non-void elements require `{ ... }`; void elements such as `br`, `img`, and `input` require `;`. Names are static ASCII identifiers with optional hyphens or namespace colons. Unknown elements and attributes are allowed, including custom elements. Duplicate attribute names are rejected case-insensitively. This is not a full HTML content-model validator; browser HTML parsing can repair invalid nesting.

String literals denote text, not markup, and are escaped at compile time. Parentheses contain one ordinary Rust expression whose `HtmlText` output is escaped while rendering. Strings stream directly; numbers, characters, and booleans use bounded scratch storage. There is no blanket `Display` implementation: custom values can implement `HtmlText`, or use the explicit size-limited `write_display` writer operation. Attribute values must be string literals or parenthesized expressions, and are always emitted in double quotes. Boolean/optional attribute shorthand is not implemented; HTML boolean attributes are true by presence, so `disabled=(false)` does not disable a boolean attribute.

The deliberately small directive set is:

| Directive | Meaning |
| --- | --- |
| `@doctype;` | Emit the HTML doctype. |
| `@let pattern = expression;` | Bind a Rust value for subsequent content. |
| `@for pattern in expression { ... }` | Iterate with Rust borrowing and types. |
| `@if condition { ... } @else if condition { ... } @else { ... }` | Conditional rendering; `if let` is also supported. |
| `@render function(arguments);` | Await a component, inserting the current writer as its first argument. |
| `@raw(expression);` | Emit an explicitly constructed `TrustedHtml` value without escaping. |
| `@flush;` | Await publication of the buffered prefix and flush the destination. |

Bindings inside a nested element are scoped to the surrounding generated Rust control-flow block, not to the element itself. Use control-flow blocks or component functions when separate Rust scopes are needed. Conditions and iterators use Rust's usual brace restrictions; parenthesize a struct literal where necessary. Expressions may use native `.await`, including attributes, bindings, conditions, iterators, raw values, and component arguments. `?` ends rendering with an error from the macro's generated async block; errors must be convertible to `std::io::Error`. `return`, and `break` or `continue` that would leave the expression, are rejected at compile time, because they would skip the remaining markup, such as a closing quote, and let later escaped text be read as attributes. Loops, closures, and async blocks inside an expression may use them internally. Macro arguments cannot be checked as code, so these keywords are rejected anywhere in the arguments of a macro used in an expression; move such a loop into a helper function. Control flow produced by a macro's own expansion cannot be checked either, so do not use macros that return or break there.

## Components

```rust
use base_web_html::{AsyncWrite, BlockingWriter, HtmlWriter, html};
use std::io;

async fn greeting<W: AsyncWrite + Unpin>(out: &mut HtmlWriter<W>, name: &str) -> io::Result<()> {
    html! { out; p { "Hello, " (name) } }.await
}

async fn page() -> io::Result<Vec<u8>> {
    let mut out = HtmlWriter::new(BlockingWriter::new(Vec::new()));
    html! { out; main { @render greeting("Ada"); } }.await?;
    Ok(out.finish().await?.into_inner())
}
```

Components write to the same destination without first allocating their own strings. A component's ordinary Rust argument and return types are checked by the compiler. The macro does not refer to a hard-coded `base_web_html` crate path, so dependency renaming works.

## Escaping and trust

Both text and attribute values escape `&`, `<`, `>`, `"`, and `'`. HTML escaping does not validate URLs: callers must validate schemes and other application-specific URL policies before writing `href`, `src`, or similar attributes. It also does not sanitize arbitrary HTML structures or make JavaScript and CSS safe. Serve documents with a declared UTF-8 encoding, through `Content-Type` or `meta charset`: escaping passes control characters through unchanged, and a browser guessing another encoding, such as ISO-2022-JP, could read them as escape sequences that shift escaped text out of its context. Dynamic `on*`, `style`, and `srcdoc` attributes are rejected, but attributes that client-side frameworks or SVG animation evaluate as code or markup, such as `x-on:click`, `hx-on:click`, `v-html`, or the `values` and `to` attributes of SVG `animate` and `set`, are ordinary attributes to the template: never give them untrusted values.

`script`, `style`, `xmp`, `iframe`, `noembed`, `noframes`, and `noscript` bodies accept only static string literals, emitted unchanged. No dynamic values, directives, or components are permitted in those bodies. Dynamic `on*`, `style`, and `srcdoc` attributes are rejected case-insensitively. `plaintext` is unsupported because it has no meaningful balanced closing tag. These restrictions are conservative and do not constitute a general-purpose HTML sanitizer or validator for every foreign-content context.

Use `@raw(TrustedHtml::new(fragment));` only for markup you have independently established is safe. This is an explicit trust decision, not a sanitization operation. `HtmlWriter::write_literal` also bypasses escaping and is intended for compiler-generated markup. Templates and static literals are trusted application code, never untrusted user-supplied programs.

## Output and performance

`HtmlWriter<W>` accepts `futures_io::AsyncWrite + Unpin`, re-exported as `base_web_html::AsyncWrite`. It owns a bounded output buffer (8 KiB by default), combines adjacent static fragments at compile time, and escapes strings incrementally. `BlockingWriter` adapts synchronous destinations but runs their operations on the polling thread; it does not make blocking files or sockets nonblocking. A pinned boxed destination can adapt an underlying writer that is not `Unpin`.

Full output buffers drain automatically. Template completion itself does not flush a partial buffer; use `@flush`, `out.flush().await`, or `out.finish().await`. Dropping a writer discards pending output without I/O. A write or formatting error stops rendering and may leave a partial document; rendering is not transactional. The original destination error is preserved. Components should return `io::Result<()>`; application-specific errors can be mapped to I/O errors at that boundary in this prototype.

The [streaming guide](https://github.com/base-rs/base-web-html/blob/main/context/streaming.md) describes backpressure, cancellation, bounded formatting, and the optional HTTP response body. No executor, `Send`, or `'static` constraint is imposed on core rendering. Whether a concrete rendering future or body is `Send` depends on its captures, destination, and text implementations; generic `HtmlText` alone does not promise a `Send` future.

The [benchmark suite](https://github.com/base-rs/base-web-html/tree/main/benchmarks/rendering) provides parameterized static-page, component-list, escaping, and HTTP-body workloads with machine-readable records. The older `cargo bench --bench render` remains an exploratory four-user microbenchmark. See [Performance](https://github.com/base-rs/base-web-html/blob/main/context/performance.md) for historical measurements and their limitations; neither harness establishes improved agent outcomes.
