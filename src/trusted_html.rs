// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

/// Markup that the caller has explicitly approved for emission without escaping.
///
/// This type does not sanitize or validate its input. Never construct it from untrusted input without suitable sanitization.
#[derive(Clone, Copy, Debug)]
pub struct TrustedHtml<'a>(&'a str);

impl<'a> TrustedHtml<'a> {
    /// Trust an HTML fragment. This is a security decision, not a sanitization operation.
    pub const fn new(value: &'a str) -> Self {
        Self(value)
    }

    /// Return the trusted markup unchanged.
    pub const fn as_str(self) -> &'a str {
        self.0
    }
}
