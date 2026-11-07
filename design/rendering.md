# HTML Rendering

## Requirement

The engine must support generating HTML from Rust values, with performance and reliable agent-authored changes as primary design goals. Dynamic text and quoted attribute values must be escaped by default. Emitting trusted, unescaped markup must require an explicit operation.

## Rationale

The user requested a Rust HTML template engine similar in spirit to XRB, optimized for agents and performance, and permits templates in ordinary `.rs` files. Safe defaults and compiler feedback reduce the number of mistakes an agent must detect manually.

## Scope

The public template syntax, async-first rendering API, compiler diagnostics, and template composition. The implementation uses a compile-time Rust DSL as an experiment, not as a requirement to exclude other frontends. Additional asynchronous contracts are described in [Streaming](streaming.md).

## Validation

Exercise escaped text, quoted attributes, explicit trusted markup, nested components, Rust control flow, and propagation of output errors through public integration tests. Compile-fail tests must demonstrate diagnostics for malformed template syntax and prohibited interpolation contexts. Inspect generated code to confirm that the rendering path does not require a runtime template parser, DOM, or intermediate component strings.

Use the `render` benchmark to measure repeated rendering of a user list into a reused buffer against a handwritten writer implementation under the same release build. Record environment and measurements when making performance claims. No numerical performance threshold or agent-success target has been agreed; neither is an acceptance criterion yet.
