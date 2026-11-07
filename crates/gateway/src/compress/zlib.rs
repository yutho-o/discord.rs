//! The `zlib-stream` state: one `flate2::Decompress` per connection, through pure Rust zlib-rs, and
//! the buffer of a partial frame. Reached only through `Decompressor`.

use crate::GatewayError;
use flate2::{Decompress, FlushDecompress, Status};

const SUFFIX: [u8; 4] = [0x00, 0x00, 0xFF, 0xFF];
const CHUNK: usize = 64 * 1024;

pub(super) struct ZlibStream {
    inflate: Decompress,
    pending: Vec<u8>,
}

impl std::fmt::Debug for ZlibStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ZlibStream").field("pending", &self.pending.len()).finish_non_exhaustive()
    }
}

impl ZlibStream {
    pub(super) fn new() -> Self {
        Self { inflate: Decompress::new(true), pending: Vec::new() }
    }

    pub(super) fn push(&mut self, message: &[u8]) -> Result<Option<Vec<u8>>, GatewayError> {
        self.pending.extend_from_slice(message);
        if !self.pending.ends_with(&SUFFIX) {
            return Ok(None);
        }
        let input = std::mem::take(&mut self.pending);
        let mut output = Vec::with_capacity(input.len().saturating_mul(4).max(CHUNK));
        let mut consumed = 0usize;
        loop {
            let rest = input.get(consumed..).unwrap_or_default();
            if output.capacity() == output.len() {
                output.reserve(CHUNK.max(output.len()));
            }
            let in_before = self.inflate.total_in();
            let out_before = self.inflate.total_out();
            let status = self
                .inflate
                .decompress_vec(rest, &mut output, FlushDecompress::Sync)
                .map_err(|e| GatewayError::Inflate(e.to_string()))?;
            let read = usize::try_from(self.inflate.total_in() - in_before).unwrap_or(usize::MAX);
            let wrote = self.inflate.total_out() - out_before;
            consumed = consumed.saturating_add(read);
            let input_done = consumed >= input.len();
            let output_full = output.len() == output.capacity();
            match status {
                Status::StreamEnd => break,
                Status::Ok | Status::BufError => {
                    if input_done && !output_full {
                        break;
                    }
                    if read == 0 && wrote == 0 && !output_full {
                        return Err(GatewayError::Inflate(String::from("the stream stopped making progress")));
                    }
                }
            }
        }
        Ok(Some(output))
    }
}
