//! Transport compression. Each connection gets a new [`Decompressor`]. A frame is untrusted input, so
//! this module does no direct indexing.
#![deny(clippy::indexing_slicing)]

#[cfg(feature = "zlib")]
mod zlib;

use crate::GatewayError;

/// Which transport compression to ask for. Not gated on a feature, so that the signature does not
/// change with features. Asking for one that was compiled out fails at [`Decompressor::new`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Compression {
    /// None.
    None,
    /// `zlib-stream`.
    ZlibStream,
}

impl Default for Compression {
    /// `zlib-stream` when the `zlib` feature is on, otherwise none.
    fn default() -> Self {
        if cfg!(feature = "zlib") { Self::ZlibStream } else { Self::None }
    }
}

impl Compression {
    /// The value of the URL's `compress` query, if any.
    pub const fn query_value(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::ZlibStream => Some("zlib-stream"),
        }
    }
}

/// Turns WebSocket messages into complete gateway frames.
#[derive(Debug)]
pub struct Decompressor {
    #[cfg(feature = "zlib")]
    zlib: Option<zlib::ZlibStream>,
}

impl Decompressor {
    /// A new stream. `ZlibStream` without the `zlib` feature fails with `Connect`.
    pub fn new(mode: Compression) -> Result<Self, GatewayError> {
        match mode {
            Compression::None => Ok(Self {
                #[cfg(feature = "zlib")]
                zlib: None,
            }),
            #[cfg(feature = "zlib")]
            Compression::ZlibStream => Ok(Self { zlib: Some(zlib::ZlibStream::new()) }),
            #[cfg(not(feature = "zlib"))]
            Compression::ZlibStream => Err(GatewayError::Connect(String::from("the `zlib` feature is off"))),
        }
    }

    /// Takes one binary WebSocket message. `Some` is a complete frame, and `None` means more
    /// messages are needed: a `zlib-stream` frame ends in `00 00 FF FF` at a message boundary, and
    /// may span several messages. A corrupt stream fails with `Inflate`, and the shard reconnects.
    /// In `None` mode the message is returned as it is.
    pub fn push(&mut self, message: &[u8]) -> Result<Option<Vec<u8>>, GatewayError> {
        #[cfg(feature = "zlib")]
        if let Some(stream) = &mut self.zlib {
            return stream.push(message);
        }
        Ok(Some(message.to_vec()))
    }
}
