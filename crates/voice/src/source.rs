//! What a connection plays: Opus frames, one after the other. Discord wants Opus at 48 kHz; this
//! crate does not encode, so a source hands over frames already encoded, such as those of an Ogg
//! Opus file (`ffmpeg -i in.mp3 -c:a libopus -ar 48000 -ac 2 out.opus`).

use crate::VoiceError;
use bytes::Bytes;
use futures::future::BoxFuture;
use std::collections::VecDeque;
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};

/// A source of Opus frames.
pub trait AudioSource: Send + 'static {
    /// The next frame, or `None` at the end.
    fn next_frame(&mut self) -> BoxFuture<'_, Result<Option<Bytes>, VoiceError>>;
}

/// Frames held in memory.
#[derive(Clone, Debug, Default)]
pub struct OpusFrames(VecDeque<Bytes>);

impl OpusFrames {
    /// These frames, in order.
    pub fn new(frames: impl IntoIterator<Item = impl Into<Bytes>>) -> Self {
        Self(frames.into_iter().map(Into::into).collect())
    }
}

impl AudioSource for OpusFrames {
    fn next_frame(&mut self) -> BoxFuture<'_, Result<Option<Bytes>, VoiceError>> {
        let next = self.0.pop_front();
        Box::pin(async move { Ok(next) })
    }
}

/// How long an Opus packet plays, from its TOC byte (RFC 6716 3.1). `None` for an empty or
/// malformed packet.
pub fn opus_duration(packet: &[u8]) -> Option<Duration> {
    let toc = *packet.first()?;
    let config = toc >> 3;
    // In units of 2.5 ms.
    let units: u32 = match config {
        0..=11 => [4, 8, 16, 24][usize::from(config % 4)],
        12..=15 => [4, 8][usize::from(config % 2)],
        _ => [1, 2, 4, 8][usize::from(config % 4)],
    };
    let frames: u32 = match toc & 0x03 {
        0 => 1,
        1 | 2 => 2,
        _ => u32::from(*packet.get(1)? & 0x3f),
    };
    let micros = u64::from(units * frames) * 2500;
    (micros > 0 && micros <= 120_000).then(|| Duration::from_micros(micros))
}

/// Reads the Opus packets of an Ogg Opus stream (RFC 7845): the first logical stream, without its
/// `OpusHead` and `OpusTags` headers. No decoding, no checksum check.
pub struct OggOpus<R> {
    reader: R,
    serial: Option<u32>,
    headers_left: u8,
    partial: Vec<u8>,
    ready: VecDeque<Bytes>,
    done: bool,
}

impl<R> std::fmt::Debug for OggOpus<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OggOpus").field("serial", &self.serial).finish_non_exhaustive()
    }
}

impl OggOpus<tokio::fs::File> {
    /// Opens an `.opus` or `.ogg` file.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, VoiceError> {
        Ok(Self::new(tokio::fs::File::open(path).await.map_err(VoiceError::Io)?))
    }
}

impl OggOpus<std::io::Cursor<Bytes>> {
    /// Reads from bytes in memory.
    pub fn from_bytes(bytes: impl Into<Bytes>) -> Self {
        Self::new(std::io::Cursor::new(bytes.into()))
    }
}

impl<R: AsyncRead + Unpin + Send + 'static> OggOpus<R> {
    /// Reads from any byte stream.
    pub fn new(reader: R) -> Self {
        Self { reader, serial: None, headers_left: 2, partial: Vec::new(), ready: VecDeque::new(), done: false }
    }

    /// Reads one page. False at the end of the stream.
    async fn page(&mut self) -> Result<bool, VoiceError> {
        let mut head = [0u8; 27];
        match self.reader.read_exact(&mut head).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(false),
            Err(e) => return Err(VoiceError::Io(e)),
        }
        if &head[0..4] != b"OggS" || head[4] != 0 {
            return Err(VoiceError::Source("not an Ogg stream".into()));
        }
        let serial = u32::from_le_bytes([head[14], head[15], head[16], head[17]]);
        let mut lacing = vec![0u8; usize::from(head[26])];
        self.reader.read_exact(&mut lacing).await.map_err(VoiceError::Io)?;
        let mut body = vec![0u8; lacing.iter().map(|l| usize::from(*l)).sum()];
        self.reader.read_exact(&mut body).await.map_err(VoiceError::Io)?;
        let serial_ok = *self.serial.get_or_insert(serial) == serial;
        if !serial_ok {
            return Ok(true);
        }
        let mut at = 0;
        for len in lacing {
            let len = usize::from(len);
            self.partial.extend_from_slice(&body[at..at + len]);
            at += len;
            if len < 255 {
                let packet = std::mem::take(&mut self.partial);
                if self.headers_left > 0 {
                    if self.headers_left == 2 && !packet.starts_with(b"OpusHead") {
                        return Err(VoiceError::Source("an Ogg stream that is not Opus".into()));
                    }
                    self.headers_left -= 1;
                } else if !packet.is_empty() {
                    self.ready.push_back(Bytes::from(packet));
                }
            }
        }
        Ok(true)
    }
}

impl<R: AsyncRead + Unpin + Send + 'static> AudioSource for OggOpus<R> {
    fn next_frame(&mut self) -> BoxFuture<'_, Result<Option<Bytes>, VoiceError>> {
        Box::pin(async move {
            while self.ready.is_empty() && !self.done {
                if !self.page().await? {
                    self.done = true;
                }
            }
            Ok(self.ready.pop_front())
        })
    }
}

/// An Ogg Opus stream in memory, for tests and examples: one page per packet after the two headers.
pub fn ogg_opus_bytes(packets: &[&[u8]]) -> Vec<u8> {
    let mut head = b"OpusHead".to_vec();
    head.extend_from_slice(&[1, 2, 0x38, 0x01, 0x80, 0xbb, 0, 0, 0, 0, 0]);
    let tags = b"OpusTags\x00\x00\x00\x00\x00\x00\x00\x00".to_vec();
    let mut out = Vec::new();
    let mut all: Vec<&[u8]> = vec![&head, &tags];
    all.extend_from_slice(packets);
    for (sequence, packet) in all.iter().enumerate() {
        let mut lacing = vec![255u8; packet.len() / 255];
        lacing.push(u8::try_from(packet.len() % 255).unwrap_or(0));
        out.extend_from_slice(b"OggS\x00");
        out.push(if sequence == 0 { 2 } else { 0 });
        out.extend_from_slice(&[0; 8]);
        out.extend_from_slice(&7u32.to_le_bytes());
        out.extend_from_slice(&u32::try_from(sequence).unwrap_or(0).to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        out.push(u8::try_from(lacing.len()).unwrap_or(0));
        out.extend_from_slice(&lacing);
        out.extend_from_slice(packet);
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[tokio::test]
    async fn an_ogg_stream_yields_its_packets() {
        let long = vec![0xfc; 300];
        let bytes = ogg_opus_bytes(&[b"\xfc\x01", &long, b"\xfc\x03"]);
        let mut source = OggOpus::from_bytes(bytes);
        assert_eq!(source.next_frame().await.unwrap().unwrap().as_ref(), b"\xfc\x01");
        assert_eq!(source.next_frame().await.unwrap().unwrap().len(), 300, "a packet across lacing values");
        assert_eq!(source.next_frame().await.unwrap().unwrap().as_ref(), b"\xfc\x03");
        assert!(source.next_frame().await.unwrap().is_none());
        assert!(OggOpus::from_bytes(b"RIFF....".repeat(4)).next_frame().await.is_err());
    }

    #[test]
    fn durations_come_from_the_toc() {
        assert_eq!(opus_duration(&[0xfc]), Some(Duration::from_millis(20)), "CELT fullband 20 ms");
        assert_eq!(opus_duration(&[0xf8, 0xff, 0xfe]), Some(Duration::from_millis(20)), "silence");
        assert_eq!(opus_duration(&[0x08]), Some(Duration::from_millis(20)), "SILK 20 ms");
        assert_eq!(opus_duration(&[0xfd]), Some(Duration::from_millis(40)), "two frames");
        assert_eq!(opus_duration(&[0xe3, 0x03]), Some(Duration::from_micros(7500)), "three of 2.5 ms");
        assert_eq!(opus_duration(&[]), None);
    }
}
