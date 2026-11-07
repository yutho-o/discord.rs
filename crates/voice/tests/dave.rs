//! DAVE end to end: a voice server that plays the MLS external sender, as Discord's does, and a
//! second member of the call with its own `davey` session. The bot must form the group with it,
//! switch keys on the transition, and then each side must decrypt what the other sends.
//!
//! The external sender helper follows davey's `tests/unit/session.rs` (MIT, (c) Snazzah), itself a
//! port of libdave's `external_sender.cpp`.
#![cfg(feature = "dave")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use davey::{DaveSession, MediaType};
use dr_core::Secret;
use dr_model::{ChannelId, GuildId, UserId};
use dr_voice::crypto::{EncryptionMode, RtpHeader, Transport};
use dr_voice::protocol::op;
use dr_voice::{ConnectionInfo, OpusFrames, PlaybackState, VoiceConnection, VoiceOptions, udp};
use futures::{SinkExt, StreamExt};
use openmls::prelude::tls_codec::{Deserialize as _, Serialize as _};
use openmls::prelude::*;
use openmls_basic_credential::SignatureKeyPair;
use openmls_rust_crypto::OpenMlsRustCrypto;
use serde_json::{Value, json};
use std::num::NonZeroU16;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

const BOT: u64 = 1_234_123_412_341_234;
const OTHER: u64 = 5_678_567_856_785_678;
const CHANNEL: u64 = 1_234_567_890;
const KEY: [u8; 32] = [9; 32];

struct ExternalSender {
    provider: OpenMlsRustCrypto,
    group_id: GroupId,
    signer: SignatureKeyPair,
    sender: openmls::prelude::ExternalSender,
}

impl ExternalSender {
    fn new(group: u64) -> Self {
        let ciphersuite = Ciphersuite::MLS_128_DHKEMP256_AES128GCM_SHA256_P256;
        let signer = SignatureKeyPair::new(ciphersuite.signature_algorithm()).unwrap();
        let sender = openmls::prelude::ExternalSender::new(
            signer.public().into(),
            BasicCredential::new(vec![0x00, 0x01, 0x01, 0x00]).into(),
        );
        Self {
            provider: OpenMlsRustCrypto::default(),
            group_id: GroupId::from_slice(&group.to_be_bytes()),
            signer,
            sender,
        }
    }

    fn marshalled(&self) -> Vec<u8> {
        self.sender.tls_serialize_detached().unwrap()
    }

    /// The payload of an append proposal (op 27, after its type byte) adding `key_package`'s owner.
    fn propose_add(&self, key_package: &[u8]) -> Vec<u8> {
        let key_package = KeyPackageIn::tls_deserialize_exact_bytes(key_package)
            .unwrap()
            .validate(self.provider.crypto(), ProtocolVersion::Mls10)
            .unwrap();
        let message = ExternalProposal::new_add::<OpenMlsRustCrypto>(
            key_package,
            self.group_id.clone(),
            GroupEpoch::from(0),
            &self.signer,
            SenderExtensionIndex::new(0),
        )
        .unwrap();
        VLBytes::new(message.tls_serialize_detached().unwrap()).tls_serialize_detached().unwrap()
    }
}

type Ws = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

fn server_binary(seq: u16, opcode: u8, payload: &[u8]) -> Message {
    let mut out = seq.to_be_bytes().to_vec();
    out.push(opcode);
    out.extend_from_slice(payload);
    Message::binary(out)
}

fn text(opcode: u8, d: Value) -> Message {
    Message::text(json!({"op": opcode, "d": d}).to_string())
}

/// The next JSON frame with this opcode, skipping the others.
async fn next_text(ws: &mut Ws, opcode: u8) -> Value {
    loop {
        if let Message::Text(t) = ws.next().await.unwrap().unwrap() {
            let v: Value = serde_json::from_str(t.as_str()).unwrap();
            if v["op"] == opcode {
                return v["d"].clone();
            }
        }
    }
}

/// The payload of the next binary frame with this opcode, skipping the others.
async fn next_binary(ws: &mut Ws, opcode: u8) -> Vec<u8> {
    loop {
        if let Message::Binary(b) = ws.next().await.unwrap().unwrap()
            && b.first() == Some(&opcode)
        {
            return b[1..].to_vec();
        }
    }
}

#[tokio::test]
async fn the_bot_forms_a_group_and_both_sides_decrypt() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let socket = Arc::new(UdpSocket::bind(("127.0.0.1", 0)).await.unwrap());
    let endpoint = format!("ws://{}", listener.local_addr().unwrap());
    let udp_port = socket.local_addr().unwrap().port();
    let transport = Arc::new(Transport::new(EncryptionMode::Aes256Gcm, &KEY).unwrap());

    // UDP: answer IP discovery, and hand over every voice packet.
    let (packets_tx, mut packets) = mpsc::channel::<Vec<u8>>(64);
    let client_addr = Arc::new(parking_lot::Mutex::new(None));
    {
        let (socket, client_addr) = (socket.clone(), client_addr.clone());
        tokio::spawn(async move {
            let mut buf = [0u8; 2048];
            loop {
                let (n, peer) = socket.recv_from(&mut buf).await.unwrap();
                if n == udp::DISCOVERY_LEN {
                    *client_addr.lock() = Some(peer);
                    let answer = udp::discovery_response(1, std::net::Ipv4Addr::LOCALHOST, peer.port());
                    socket.send_to(&answer, peer).await.unwrap();
                } else if n > 12 {
                    let _ = packets_tx.send(buf[..n].to_vec()).await;
                }
            }
        });
    }

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        ws.send(text(op::HELLO, json!({"heartbeat_interval": 41_250.0}))).await.unwrap();
        let identify = next_text(&mut ws, op::IDENTIFY).await;
        assert_eq!(identify["max_dave_protocol_version"], 1);
        ws.send(text(
            op::READY,
            json!({"ssrc": 1, "ip": "127.0.0.1", "port": udp_port, "modes": ["aead_aes256_gcm_rtpsize"]}),
        ))
        .await
        .unwrap();
        next_text(&mut ws, op::SELECT_PROTOCOL).await;
        let description = json!({"mode": "aead_aes256_gcm_rtpsize", "secret_key": KEY, "dave_protocol_version": 1});
        ws.send(text(op::SESSION_DESCRIPTION, description)).await.unwrap();
        ws.send(text(op::CLIENTS_CONNECT, json!({"user_ids": [OTHER.to_string()]}))).await.unwrap();

        let external = ExternalSender::new(CHANNEL);
        ws.send(server_binary(1, op::DAVE_MLS_EXTERNAL_SENDER, &external.marshalled())).await.unwrap();
        let _bot_package = next_binary(&mut ws, op::DAVE_MLS_KEY_PACKAGE).await;

        // The other member, already in the call, joins the bot's group.
        let version = NonZeroU16::new(1).unwrap();
        let mut other = DaveSession::new(version, OTHER, CHANNEL, None).unwrap();
        other.set_external_sender(&external.marshalled()).unwrap();
        let mut proposals = vec![0u8];
        proposals.extend(external.propose_add(&other.create_key_package().unwrap()));
        ws.send(server_binary(2, op::DAVE_MLS_PROPOSALS, &proposals)).await.unwrap();

        let commit_welcome = next_binary(&mut ws, op::DAVE_MLS_COMMIT_WELCOME).await;
        let mut rest = commit_welcome.as_slice();
        MlsMessageIn::tls_deserialize(&mut rest).unwrap();
        let commit = &commit_welcome[..commit_welcome.len() - rest.len()];
        assert!(!rest.is_empty(), "a welcome follows the commit");
        other.process_welcome(rest, Some(&[BOT, OTHER])).unwrap();
        other.prepare_transition(1, 1).unwrap();

        let mut announce = 1u16.to_be_bytes().to_vec();
        announce.extend_from_slice(commit);
        ws.send(server_binary(3, op::DAVE_MLS_ANNOUNCE_COMMIT_TRANSITION, &announce)).await.unwrap();
        let ready = next_text(&mut ws, op::DAVE_TRANSITION_READY).await;
        assert_eq!(ready["transition_id"], 1);
        ws.send(text(op::DAVE_EXECUTE_TRANSITION, json!({"transition_id": 1}))).await.unwrap();
        assert!(other.execute_transition(1).unwrap());
        (ws, other)
    });

    let info = ConnectionInfo {
        endpoint,
        guild_id: GuildId::new(10),
        channel_id: ChannelId::new(CHANNEL),
        user_id: UserId::new(BOT),
        session_id: Secret::new("sess".to_owned()),
        token: Secret::new("tok".to_owned()),
    };
    let connection = VoiceConnection::connect(info, VoiceOptions::default()).await.unwrap();
    assert_eq!(connection.dave_version(), 1);
    let (mut ws, mut other) = tokio::time::timeout(Duration::from_secs(10), server).await.unwrap().unwrap();
    // The execute frame is in flight: give the bot a moment to switch keys.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(connection.privacy_code().is_some_and(|c| !c.is_empty()), "the group is established");

    // The bot speaks; the other member decrypts.
    let frame = vec![0xfc, 0x11, 0x22, 0x33, 0x44];
    let playback = connection.play(OpusFrames::new([frame.clone()]));
    assert_eq!(playback.finished().await, PlaybackState::Ended);
    let packet = tokio::time::timeout(Duration::from_secs(5), packets.recv()).await.unwrap().unwrap();
    let rtp = transport.open(&packet).unwrap();
    assert_ne!(rtp.opus, frame, "end-to-end encrypted on the wire");
    assert_eq!(other.decrypt(BOT, MediaType::AUDIO, &rtp.opus).unwrap(), frame);

    // The other member speaks; the bot decrypts.
    let mut incoming = connection.receive();
    ws.send(text(op::SPEAKING, json!({"speaking": 1, "ssrc": 9, "user_id": OTHER.to_string(), "delay": 0})))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    let theirs = vec![0xfc, 0x55, 0x66, 0x77];
    let sealed = other.encrypt_opus(&theirs).unwrap().into_owned();
    let packet = transport.seal(&RtpHeader::build(1, 960, 9), 1, &sealed).unwrap();
    let client = client_addr.lock().unwrap();
    socket.send_to(&packet, client).await.unwrap();
    let heard = tokio::time::timeout(Duration::from_secs(5), incoming.recv()).await.unwrap().unwrap();
    assert_eq!(heard.user_id, UserId::new(OTHER));
    assert_eq!(heard.opus.as_ref(), theirs);
    connection.destroy();
}
