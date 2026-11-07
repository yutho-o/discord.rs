//! `zlib-stream` on the server side: one compressor per connection, sync flush at each message, so a
//! frame ends in `00 00 FF FF`. A script can split a frame across WebSocket messages.

use flate2::{Compress, Compression, FlushCompress};

/// The compressor of one connection.
#[derive(Debug)]
pub(crate) struct Deflater {
    inner: Compress,
}

impl Deflater {
    pub(crate) fn new() -> Self {
        Self { inner: Compress::new(Compression::default(), true) }
    }

    /// Compresses one message and flushes. An error from flate2 cannot happen with a growing output
    /// buffer, and would end the connection through an empty frame the client rejects.
    pub(crate) fn frame(&mut self, text: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(text.len() / 2 + 64);
        let start = self.inner.total_in();
        loop {
            let consumed = usize::try_from(self.inner.total_in() - start).unwrap_or(text.len());
            let rest = text.get(consumed..).unwrap_or_default();
            out.reserve(rest.len() / 2 + 64);
            if self.inner.compress_vec(rest, &mut out, FlushCompress::Sync).is_err() {
                return Vec::new();
            }
            let done = usize::try_from(self.inner.total_in() - start).unwrap_or(text.len()) >= text.len();
            // Room left over means the flush finished: zlib stopped for lack of work, not of space.
            if done && out.len() < out.capacity() && out.ends_with(&[0, 0, 0xFF, 0xFF]) {
                return out;
            }
        }
    }
}
