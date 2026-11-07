// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use futures::io::AsyncWrite;
use std::cell::RefCell;
use std::hint::black_box;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Default)]
pub(super) struct OutputStats {
    pub bytes: usize,
    pub events: u64,
    pub first_frame_ns: u128,
    pub captured: Option<Vec<u8>>,
    pub frame_sizes: Vec<usize>,
}

impl OutputStats {
    pub fn new(collect: bool) -> Self {
        Self {
            captured: collect.then(Vec::new),
            ..Self::default()
        }
    }

    pub fn observe(&mut self, bytes: &[u8]) {
        self.bytes += bytes.len();
        self.events += 1;
        if let Some(captured) = self.captured.as_mut() {
            captured.extend_from_slice(bytes);
            self.frame_sizes.push(bytes.len());
        }
        // Timed output is observed without a per-byte checksum or an additional full-output copy.
        black_box(bytes);
    }
}

#[derive(Default)]
pub(super) struct OutputSink(RefCell<OutputStats>);

impl OutputSink {
    pub fn collecting() -> Self {
        Self(RefCell::new(OutputStats::new(true)))
    }
    pub fn take(&self) -> OutputStats {
        self.0.take()
    }
}

impl AsyncWrite for OutputSink {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.0.borrow_mut().observe(bytes);
        Poll::Ready(Ok(bytes.len()))
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}

pub(super) fn validate(stats: &OutputStats, expected: &[u8]) -> io::Result<()> {
    if stats.captured.as_deref() != Some(expected) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "preflight output differs from independently escaped expected bytes",
        ));
    }
    Ok(())
}

pub(super) fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

pub(super) fn validate_frames(actual: &[usize], expected: &[usize]) -> io::Result<()> {
    if actual != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "engine/reference output boundaries differ",
        ));
    }
    Ok(())
}
