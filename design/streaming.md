# Streaming

## Requirement

Templates must be able to suspend on data acquisition and downstream output capacity, including inside components. Use one async-first template and component interface rather than parallel synchronous and asynchronous rendering interfaces. Synchronous destinations are supported through an explicitly potentially blocking adapter. Preserve default escaping and explicit trust boundaries.

Engine-controlled pending output must be bounded by configured byte limits, independently of total document or interpolation size. Strings must be escaped incrementally rather than first formatting or escaping the entire value into an owned string. An arbitrary synchronous `Display` implementation must not introduce an implicitly unbounded staging buffer; a fallback must have an explicit limit and fail if exceeded.

Explicit flushing must publish the current buffered prefix before execution continues past the flush operation. Full buffers must make progress without requiring explicit flushes. Flushing does not guarantee that a browser or intermediary immediately displays the bytes.

The HTTP body adapter must follow consumer demand without requiring a spawned rendering task or an unbounded queue. A slow consumer must stop further rendering once bounded pending output is exhausted. Dropping the body must drop its rendering future, including pending data acquisition. Output already handed to the consumer is not owned by the engine and is outside its buffering limit.

Write and rendering errors must stop the rendering operation and propagate to the caller or body consumer. Previously published output may remain visible; a late rendering error must not be converted into a successful, silently truncated body. Dropping an unfinished writer must not implicitly flush or perform background work. Cancellation may leave a partially rendered document; restarting an entire interpolation is not transactional.

## Rationale

The user accepted the proposed first-class streaming design and challenged bifurcating the interface. One async-first interface keeps demand-driven execution, bounded buffering, explicit flush boundaries, cancellation, and a pull-based HTTP adapter central rather than optional template semantics. These properties make streaming meaningful for large responses and progressively arriving data rather than merely splitting a completed string into chunks.

## Scope

The writer, template expansion, text rendering, synchronous destination adapter, and optional HTTP body adapter. Application-owned source data, downstream transport buffers, and chunks retained by consumers are not included in the engine's pending-output bound. No web framework or concrete executor is required by the core implementation.

## Validation

Use manually polled test sinks and bodies to exercise repeated `Pending`, short writes, zero writes, output errors, flush errors, data acquisition suspension, and consumer demand. Render large Unicode strings containing every escaped character with a tiny buffer, and assert the maximum buffered and queued byte counts, accounting separately for the fixed 512-byte formatting scratch space. Exercise bounded-formatting overflow without publishing the failed value. Verify components, native awaited expressions, static coalescing, and the same escaping for asynchronous and synchronous destinations.

Drop a body while awaiting output capacity and while awaiting application data; assert the renderer and owned pending operations are dropped, with no additional work. Verify published prefixes followed by rendering errors produce a body error and then termination. Preserve existing HTML behavior tests while migrating them to the single async interface. Compare the `render` benchmark with equivalent handwritten async writes and retain historical measurements as explicitly historical, not current performance claims. Numerical streaming throughput and latency targets remain open questions.
