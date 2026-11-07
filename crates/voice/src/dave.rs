//! DAVE, Discord's end-to-end encryption for voice, over the `davey` crate.
//!
//! The flow follows upstream's `DAVESession.ts` and libdave's sample session manager: a key package
//! after the session description, the MLS messages of opcodes 25 to 31, and transitions (21, 22,
//! 24) that switch the key in step with the other members. Each method returns what to send back.
//! Without the `dave` feature, the session stays at version 0 and every packet passes through.

use crate::protocol::{VoiceFrame, binary};
use serde_json::Value;

/// The Opus frame of silence, which DAVE leaves in the clear.
pub const SILENCE_FRAME: [u8; 3] = [0xf8, 0xff, 0xfe];

/// Something the session needs sent on the voice gateway.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DaveOut {
    /// A JSON frame.
    Json(String),
    /// A binary frame, `[op][payload]`.
    Binary(Vec<u8>),
}

fn json<T: serde::Serialize>(opcode: u8, d: T) -> Option<DaveOut> {
    VoiceFrame::new(opcode, d).to_text().ok().map(DaveOut::Json)
}

/// The highest DAVE version this build speaks, 0 without the `dave` feature.
pub fn max_protocol_version() -> u16 {
    #[cfg(feature = "dave")]
    {
        davey::DAVE_PROTOCOL_VERSION
    }
    #[cfg(not(feature = "dave"))]
    {
        0
    }
}

#[cfg(feature = "dave")]
mod imp {
    use super::*;
    use crate::protocol::{PrepareEpoch, PrepareTransition, TransitionId, op};
    use davey::{DaveSession, MediaType, ProposalsOperationType};

    /// Decryption failures in a row before the session asks for a new transition, as upstream.
    const FAILURE_TOLERANCE: u32 = 36;
    use std::collections::HashMap;
    use std::num::NonZeroU16;

    /// One call's DAVE state.
    pub struct Dave {
        session: Option<DaveSession>,
        version: u16,
        user: u64,
        channel: u64,
        pending: HashMap<u16, u16>,
        downgraded: bool,
        reinitializing: bool,
        last_transition: Option<u16>,
        failures: u32,
    }

    impl std::fmt::Debug for Dave {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Dave").field("version", &self.version).finish_non_exhaustive()
        }
    }

    impl Dave {
        /// The state after the session description, and the key package to send when the call
        /// is end-to-end encrypted.
        pub fn new(version: u16, user: u64, channel: u64) -> (Self, Vec<DaveOut>) {
            let mut dave = Self {
                session: None,
                version,
                user,
                channel,
                pending: HashMap::new(),
                downgraded: false,
                reinitializing: false,
                last_transition: None,
                failures: 0,
            };
            let out = dave.reinit();
            (dave, out)
        }

        /// The version in use: 0 when packets are not end-to-end encrypted.
        pub fn version(&self) -> u16 {
            self.version
        }

        /// The voice privacy code users can compare, once the group is established.
        pub fn privacy_code(&self) -> Option<String> {
            let code = self.session.as_ref()?.voice_privacy_code()?;
            (self.version > 0 && !code.is_empty()).then(|| code.to_owned())
        }

        /// Starts over at the current version: a new key package, or passthrough at version 0.
        fn reinit(&mut self) -> Vec<DaveOut> {
            let Some(version) = NonZeroU16::new(self.version) else {
                if let Some(session) = &mut self.session {
                    let _ = session.reset();
                    session.set_passthrough_mode(true, Some(10));
                }
                return Vec::new();
            };
            let ready = match &mut self.session {
                Some(session) => session.reinit(version, self.user, self.channel, None).map_err(|e| e.to_string()),
                None => DaveSession::new(version, self.user, self.channel, None)
                    .map(|s| self.session = Some(s))
                    .map_err(|e| e.to_string()),
            };
            if let Err(e) = ready {
                tracing::warn!(error = %e, "DAVE session could not start");
                return Vec::new();
            }
            match self.session.as_mut().map(DaveSession::create_key_package) {
                Some(Ok(package)) => vec![DaveOut::Binary(binary(op::DAVE_MLS_KEY_PACKAGE, &package))],
                Some(Err(e)) => {
                    tracing::warn!(error = %e, "DAVE key package failed");
                    Vec::new()
                }
                None => Vec::new(),
            }
        }

        /// Asks the others for a new transition after one failed, and starts over.
        fn recover(&mut self, transition: u16) -> Vec<DaveOut> {
            if self.reinitializing {
                return Vec::new();
            }
            tracing::debug!(transition, "DAVE transition invalid, reinitializing");
            self.reinitializing = true;
            self.failures = 0;
            let mut out: Vec<DaveOut> =
                json(op::DAVE_MLS_INVALID_COMMIT_WELCOME, TransitionId { transition_id: transition })
                    .into_iter()
                    .collect();
            out.extend(self.reinit());
            out
        }

        fn prepare(&mut self, transition: u16, version: u16) -> Vec<DaveOut> {
            if let Some(session) = &mut self.session
                && let Err(e) = session.prepare_transition(transition, version)
            {
                tracing::debug!(error = %e, transition, "DAVE prepare transition failed");
            }
            if transition == 0 {
                self.apply(0, version);
                return Vec::new();
            }
            self.pending.insert(transition, version);
            if version == 0
                && let Some(session) = &mut self.session
            {
                session.set_passthrough_mode(true, Some(24));
            }
            json(op::DAVE_TRANSITION_READY, TransitionId { transition_id: transition }).into_iter().collect()
        }

        fn apply(&mut self, transition: u16, version: u16) {
            let old = self.version;
            self.version = version;
            if old != version && version == 0 {
                self.downgraded = true;
            } else if transition > 0 && self.downgraded {
                self.downgraded = false;
                if let Some(session) = &mut self.session {
                    session.set_passthrough_mode(true, Some(10));
                }
            }
            self.reinitializing = false;
            self.last_transition = Some(transition);
        }

        /// A JSON frame for DAVE: 21, 22 or 24.
        pub fn on_json(&mut self, opcode: u8, d: &Value) -> Vec<DaveOut> {
            match opcode {
                op::DAVE_PREPARE_TRANSITION => match serde_json::from_value::<PrepareTransition>(d.clone()) {
                    Ok(p) => self.prepare(p.transition_id, p.protocol_version),
                    Err(_) => Vec::new(),
                },
                op::DAVE_EXECUTE_TRANSITION => {
                    let Ok(t) = serde_json::from_value::<TransitionId>(d.clone()) else { return Vec::new() };
                    let executed = self.session.as_mut().map(|s| s.execute_transition(t.transition_id));
                    if let Some(version) = self.pending.remove(&t.transition_id) {
                        self.apply(t.transition_id, version);
                    } else if !matches!(executed, Some(Ok(true))) {
                        tracing::debug!(transition = t.transition_id, "DAVE execute with nothing prepared");
                    }
                    Vec::new()
                }
                op::DAVE_PREPARE_EPOCH => match serde_json::from_value::<PrepareEpoch>(d.clone()) {
                    Ok(p) if p.epoch == 1 => {
                        self.version = p.protocol_version;
                        self.reinit()
                    }
                    _ => Vec::new(),
                },
                _ => Vec::new(),
            }
        }

        /// A binary frame for DAVE: 25, 27, 29 or 30. `members` are the users in the call, this
        /// bot included.
        pub fn on_binary(&mut self, opcode: u8, payload: &[u8], members: &[u64]) -> Vec<DaveOut> {
            let Some(session) = self.session.as_mut() else { return Vec::new() };
            match opcode {
                op::DAVE_MLS_EXTERNAL_SENDER => {
                    if let Err(e) = session.set_external_sender(payload) {
                        tracing::debug!(error = %e, "DAVE external sender refused");
                    }
                    Vec::new()
                }
                op::DAVE_MLS_PROPOSALS => {
                    let Some((&kind, proposals)) = payload.split_first() else { return Vec::new() };
                    let kind = if kind == 1 { ProposalsOperationType::REVOKE } else { ProposalsOperationType::APPEND };
                    match session.process_proposals(kind, proposals, Some(members)) {
                        Ok(Some(cw)) => {
                            let mut body = cw.commit;
                            if let Some(welcome) = cw.welcome {
                                body.extend_from_slice(&welcome);
                            }
                            vec![DaveOut::Binary(binary(op::DAVE_MLS_COMMIT_WELCOME, &body))]
                        }
                        Ok(None) => Vec::new(),
                        Err(e) => {
                            tracing::debug!(error = %e, "DAVE proposals failed");
                            Vec::new()
                        }
                    }
                }
                op::DAVE_MLS_ANNOUNCE_COMMIT_TRANSITION | op::DAVE_MLS_WELCOME => {
                    let Some((id, message)) = payload.split_at_checked(2) else { return Vec::new() };
                    let transition = u16::from_be_bytes([id[0], id[1]]);
                    let result = if opcode == op::DAVE_MLS_WELCOME {
                        session.process_welcome(message, Some(members)).map(|()| true).map_err(|e| e.to_string())
                    } else {
                        session.process_commit(message).map_err(|e| e.to_string())
                    };
                    match result {
                        Ok(true) => {
                            let version = self.version;
                            self.prepare(transition, version)
                        }
                        Ok(false) => Vec::new(),
                        Err(e) => {
                            tracing::debug!(error = %e, transition, "DAVE commit or welcome failed");
                            self.recover(transition)
                        }
                    }
                }
                _ => Vec::new(),
            }
        }

        /// End-to-end encrypts an Opus frame when the call needs it. Silence stays in the clear.
        pub fn encrypt(&mut self, frame: &[u8]) -> Option<Vec<u8>> {
            if self.version == 0 || frame == SILENCE_FRAME {
                return Some(frame.to_vec());
            }
            let session = self.session.as_mut()?;
            if !session.can_encrypt() {
                return None;
            }
            session.encrypt_opus(frame).ok().map(|c| c.into_owned())
        }

        /// Decrypts a frame from `user`. `None` for a frame to drop; the second value is what to
        /// send when too many failed in a row.
        pub fn decrypt(&mut self, user: u64, frame: &[u8]) -> (Option<Vec<u8>>, Vec<DaveOut>) {
            if frame == SILENCE_FRAME {
                return (Some(frame.to_vec()), Vec::new());
            }
            let Some(session) = self.session.as_mut() else { return (Some(frame.to_vec()), Vec::new()) };
            if !session.is_ready() || (self.version == 0 && !session.can_passthrough(user)) {
                return (Some(frame.to_vec()), Vec::new());
            }
            match session.decrypt(user, MediaType::AUDIO, frame) {
                Ok(clear) => {
                    self.failures = 0;
                    (Some(clear), Vec::new())
                }
                Err(_) if !self.reinitializing && self.pending.is_empty() => {
                    self.failures += 1;
                    if self.failures > FAILURE_TOLERANCE
                        && let Some(last) = self.last_transition
                    {
                        return (None, self.recover(last));
                    }
                    (None, Vec::new())
                }
                Err(_) => (None, Vec::new()),
            }
        }
    }
}

#[cfg(not(feature = "dave"))]
mod imp {
    use super::*;

    /// Without the `dave` feature: version 0, every packet in the clear.
    #[derive(Debug)]
    pub struct Dave;

    impl Dave {
        /// Always version 0.
        pub fn new(_version: u16, _user: u64, _channel: u64) -> (Self, Vec<DaveOut>) {
            (Self, Vec::new())
        }
        /// Always 0.
        pub fn version(&self) -> u16 {
            0
        }
        /// Never one.
        pub fn privacy_code(&self) -> Option<String> {
            None
        }
        /// Ignored.
        pub fn on_json(&mut self, _opcode: u8, _d: &Value) -> Vec<DaveOut> {
            let _ = json::<()>;
            Vec::new()
        }
        /// Ignored.
        pub fn on_binary(&mut self, _opcode: u8, _payload: &[u8], _members: &[u64]) -> Vec<DaveOut> {
            let _ = binary;
            Vec::new()
        }
        /// The frame as is.
        pub fn encrypt(&mut self, frame: &[u8]) -> Option<Vec<u8>> {
            Some(frame.to_vec())
        }
        /// The frame as is.
        pub fn decrypt(&mut self, _user: u64, frame: &[u8]) -> (Option<Vec<u8>>, Vec<DaveOut>) {
            (Some(frame.to_vec()), Vec::new())
        }
    }
}

pub use imp::Dave;

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn version_zero_passes_everything_through() {
        let (mut dave, out) = Dave::new(0, 1, 2);
        assert!(out.is_empty(), "no key package without end-to-end encryption");
        assert_eq!(dave.encrypt(b"opus").unwrap(), b"opus");
        assert_eq!(dave.decrypt(5, b"opus").0.unwrap(), b"opus");
        assert!(dave.privacy_code().is_none());
    }

    #[cfg(feature = "dave")]
    #[test]
    fn version_one_sends_a_key_package_and_waits_for_a_group() {
        use crate::protocol::op;
        let (mut dave, out) = Dave::new(1, 1_234_123_412_341_234, 9);
        assert!(matches!(&out[..], [DaveOut::Binary(b)] if b[0] == op::DAVE_MLS_KEY_PACKAGE && b.len() > 100));
        assert!(dave.encrypt(b"opus").is_none(), "no key before the group exists");
        assert_eq!(dave.encrypt(&SILENCE_FRAME).unwrap(), SILENCE_FRAME, "silence goes in the clear");
        let ready =
            dave.on_json(op::DAVE_PREPARE_TRANSITION, &serde_json::json!({"transition_id": 3, "protocol_version": 1}));
        assert_eq!(ready, [DaveOut::Json(r#"{"op":23,"d":{"transition_id":3}}"#.to_owned())]);
        assert_eq!(max_protocol_version(), 1);
    }
}
