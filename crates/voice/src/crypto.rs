//! Transport encryption of RTP packets, the `_rtpsize` AEAD modes. The RTP header is the additional
//! data, a 32-bit counter is the nonce (zero padded to the cipher's size), and the counter goes, big
//! endian, after the ciphertext and its 16 byte tag.

use aes_gcm::Aes256Gcm;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::XChaCha20Poly1305;

/// The RTP payload type of Opus on Discord.
pub const OPUS_PAYLOAD_TYPE: u8 = 0x78;
/// The length of the counter sent after each packet.
pub const NONCE_SUFFIX: usize = 4;
/// The AEAD tag length of both modes.
pub const TAG: usize = 16;
/// The fixed RTP header length.
pub const RTP_HEADER: usize = 12;

/// The encryption modes this crate speaks, best first.
pub const SUPPORTED_MODES: [EncryptionMode; 2] = [EncryptionMode::Aes256Gcm, EncryptionMode::XChaCha20Poly1305];

/// An encryption mode of the voice server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncryptionMode {
    /// `aead_aes256_gcm_rtpsize`, Discord's preferred one.
    Aes256Gcm,
    /// `aead_xchacha20_poly1305_rtpsize`, which every server supports.
    XChaCha20Poly1305,
}

impl EncryptionMode {
    /// The name on the wire.
    pub fn name(self) -> &'static str {
        match self {
            Self::Aes256Gcm => "aead_aes256_gcm_rtpsize",
            Self::XChaCha20Poly1305 => "aead_xchacha20_poly1305_rtpsize",
        }
    }

    /// The mode with this wire name.
    pub fn from_name(name: &str) -> Option<Self> {
        SUPPORTED_MODES.into_iter().find(|m| m.name() == name)
    }

    /// The best mode among those the server offers.
    pub fn choose<S: AsRef<str>>(offered: &[S]) -> Option<Self> {
        SUPPORTED_MODES.into_iter().find(|m| offered.iter().any(|o| o.as_ref() == m.name()))
    }

    fn nonce_len(self) -> usize {
        match self {
            Self::Aes256Gcm => 12,
            Self::XChaCha20Poly1305 => 24,
        }
    }
}

/// Encryption failed, or a packet did not decrypt.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    /// The key is not 32 bytes.
    #[error("the voice key must be 32 bytes")]
    BadKey,
    /// The AEAD refused.
    #[error("the packet did not encrypt or decrypt")]
    Aead,
    /// Too short, or not RTP version 2 with Opus.
    #[error("not a voice packet")]
    NotVoice,
}

enum Cipher {
    Aes(Box<Aes256Gcm>),
    XChaCha(Box<XChaCha20Poly1305>),
}

/// A key and its mode, which seals and opens packets.
pub struct Transport {
    mode: EncryptionMode,
    cipher: Cipher,
}

impl std::fmt::Debug for Transport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Transport").field("mode", &self.mode).finish_non_exhaustive()
    }
}

impl Transport {
    /// A transport over `key`, from the session description.
    pub fn new(mode: EncryptionMode, key: &[u8]) -> Result<Self, CryptoError> {
        let cipher = match mode {
            EncryptionMode::Aes256Gcm => {
                Cipher::Aes(Box::new(Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::BadKey)?))
            }
            EncryptionMode::XChaCha20Poly1305 => {
                Cipher::XChaCha(Box::new(XChaCha20Poly1305::new_from_slice(key).map_err(|_| CryptoError::BadKey)?))
            }
        };
        Ok(Self { mode, cipher })
    }

    /// The mode.
    pub fn mode(&self) -> EncryptionMode {
        self.mode
    }

    fn nonce(&self, counter: [u8; 4]) -> Vec<u8> {
        let mut nonce = vec![0u8; self.mode.nonce_len()];
        nonce[..4].copy_from_slice(&counter);
        nonce
    }

    fn encrypt(&self, nonce: &[u8], aad: &[u8], msg: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let payload = Payload { msg, aad };
        match &self.cipher {
            Cipher::Aes(c) => c.encrypt(nonce.into(), payload),
            Cipher::XChaCha(c) => c.encrypt(nonce.into(), payload),
        }
        .map_err(|_| CryptoError::Aead)
    }

    fn decrypt(&self, nonce: &[u8], aad: &[u8], msg: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let payload = Payload { msg, aad };
        match &self.cipher {
            Cipher::Aes(c) => c.decrypt(nonce.into(), payload),
            Cipher::XChaCha(c) => c.decrypt(nonce.into(), payload),
        }
        .map_err(|_| CryptoError::Aead)
    }

    /// A whole packet: `header`, the sealed `payload` with its tag, then `counter`.
    pub fn seal(&self, header: &[u8; RTP_HEADER], counter: u32, payload: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let counter = counter.to_be_bytes();
        let sealed = self.encrypt(&self.nonce(counter), header, payload)?;
        let mut packet = Vec::with_capacity(RTP_HEADER + sealed.len() + NONCE_SUFFIX);
        packet.extend_from_slice(header);
        packet.extend_from_slice(&sealed);
        packet.extend_from_slice(&counter);
        Ok(packet)
    }

    /// The Opus payload of a received packet, with the header extension and padding removed.
    pub fn open(&self, packet: &[u8]) -> Result<ReceivedRtp, CryptoError> {
        let header = RtpHeader::parse(packet).ok_or(CryptoError::NotVoice)?;
        let clear = header.clear_len();
        if packet.len() < clear + TAG + NONCE_SUFFIX {
            return Err(CryptoError::NotVoice);
        }
        let (body, counter) = packet.split_at(packet.len() - NONCE_SUFFIX);
        let counter: [u8; 4] = counter.try_into().map_err(|_| CryptoError::NotVoice)?;
        let mut opened = self.decrypt(&self.nonce(counter), &body[..clear], &body[clear..])?;
        if header.padding
            && let Some(&amount) = opened.last()
            && usize::from(amount) < opened.len()
        {
            opened.truncate(opened.len() - usize::from(amount));
        }
        let skip = header.extension_words.map_or(0, |w| 4 * usize::from(w)).min(opened.len());
        opened.drain(..skip);
        Ok(ReceivedRtp { sequence: header.sequence, timestamp: header.timestamp, ssrc: header.ssrc, opus: opened })
    }
}

/// A decrypted packet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReceivedRtp {
    /// The RTP sequence.
    pub sequence: u16,
    /// The RTP timestamp, in 48 kHz samples.
    pub timestamp: u32,
    /// Who sent it.
    pub ssrc: u32,
    /// The Opus packet, still end-to-end encrypted when DAVE is on.
    pub opus: Vec<u8>,
}

/// The parts of an RTP header that matter here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RtpHeader {
    /// The sequence.
    pub sequence: u16,
    /// The timestamp.
    pub timestamp: u32,
    /// The SSRC.
    pub ssrc: u32,
    /// The number of CSRCs after the fixed header.
    pub csrc_count: u8,
    /// The header extension's length in 32-bit words, when there is one.
    pub extension_words: Option<u16>,
    /// Whether padding ends the payload.
    pub padding: bool,
}

impl RtpHeader {
    /// A version 2 Opus header, or `None`.
    pub fn parse(packet: &[u8]) -> Option<Self> {
        if packet.len() < RTP_HEADER || packet[0] >> 6 != 2 || packet[1] & 0x7f != OPUS_PAYLOAD_TYPE {
            return None;
        }
        let csrc_count = packet[0] & 0x0f;
        let extension = packet[0] & 0x10 != 0;
        let at = RTP_HEADER + 4 * usize::from(csrc_count);
        let extension_words = if extension {
            let bytes = packet.get(at + 2..at + 4)?;
            Some(u16::from_be_bytes([bytes[0], bytes[1]]))
        } else {
            None
        };
        Some(Self {
            sequence: u16::from_be_bytes([packet[2], packet[3]]),
            timestamp: u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]),
            ssrc: u32::from_be_bytes([packet[8], packet[9], packet[10], packet[11]]),
            csrc_count,
            extension_words,
            padding: packet[0] & 0x20 != 0,
        })
    }

    /// The bytes left in the clear: the fixed header, the CSRCs, and the extension's 4 byte header.
    pub fn clear_len(&self) -> usize {
        RTP_HEADER + 4 * usize::from(self.csrc_count) + if self.extension_words.is_some() { 4 } else { 0 }
    }

    /// The header this client sends: version 2, Opus, no extension.
    pub fn build(sequence: u16, timestamp: u32, ssrc: u32) -> [u8; RTP_HEADER] {
        let mut h = [0u8; RTP_HEADER];
        h[0] = 0x80;
        h[1] = OPUS_PAYLOAD_TYPE;
        h[2..4].copy_from_slice(&sequence.to_be_bytes());
        h[4..8].copy_from_slice(&timestamp.to_be_bytes());
        h[8..12].copy_from_slice(&ssrc.to_be_bytes());
        h
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn both_modes_round_trip() {
        for mode in SUPPORTED_MODES {
            let t = Transport::new(mode, &[7u8; 32]).unwrap();
            let header = RtpHeader::build(9, 960, 42);
            let packet = t.seal(&header, 3, b"opus").unwrap();
            assert_eq!(&packet[..12], &header, "{mode:?}");
            assert_eq!(&packet[packet.len() - 4..], &[0, 0, 0, 3]);
            assert_eq!(packet.len(), 12 + 4 + TAG + 4);
            let got = t.open(&packet).unwrap();
            assert_eq!(got, ReceivedRtp { sequence: 9, timestamp: 960, ssrc: 42, opus: b"opus".to_vec() });
            let mut bad = packet.clone();
            bad[13] ^= 1;
            assert!(t.open(&bad).is_err(), "a changed byte fails");
        }
    }

    #[test]
    fn a_header_extension_stays_out_of_the_payload() {
        let t = Transport::new(EncryptionMode::Aes256Gcm, &[1u8; 32]).unwrap();
        // As Discord sends it: the extension's 4 byte header in the clear, its body encrypted.
        let mut header = RtpHeader::build(1, 2, 3).to_vec();
        header[0] |= 0x10;
        header.extend_from_slice(&[0xbe, 0xde, 0, 1]);
        let counter = 5u32.to_be_bytes();
        let nonce = t.nonce(counter);
        let mut clear = vec![0xaa, 0xbb, 0xcc, 0xdd];
        clear.extend_from_slice(b"voice");
        let sealed = t.encrypt(&nonce, &header, &clear).unwrap();
        let packet = [header, sealed, counter.to_vec()].concat();
        assert_eq!(t.open(&packet).unwrap().opus, b"voice");
    }

    #[test]
    fn modes_are_chosen_by_preference() {
        let offered = ["aead_xchacha20_poly1305_rtpsize", "aead_aes256_gcm_rtpsize", "xsalsa20_poly1305"];
        assert_eq!(EncryptionMode::choose(&offered), Some(EncryptionMode::Aes256Gcm));
        assert_eq!(EncryptionMode::choose(&["xsalsa20_poly1305"]), None);
        assert!(Transport::new(EncryptionMode::Aes256Gcm, &[0; 16]).is_err());
        assert!(RtpHeader::parse(&[0x80, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).is_none(), "not Opus");
    }
}
