//! The mock answers: it binds, the gateway sends Hello, and REST answers 404.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dr_mock::MockServer;
use futures::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::test]
async fn gateway_sends_hello() {
    let server = MockServer::start().await.unwrap();
    let url = server.endpoints().gateway_override.unwrap();
    let (mut socket, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    let first = socket.next().await.unwrap().unwrap();
    let hello: serde_json::Value = serde_json::from_str(first.to_text().unwrap()).unwrap();
    assert_eq!(hello["op"], 10);
    assert!(hello["d"]["heartbeat_interval"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn rest_answers_404_with_a_json_error() {
    let server = MockServer::start().await.unwrap();
    let api = server.endpoints().api;
    let authority = api.trim_start_matches("http://").trim_end_matches("/api").to_owned();
    let mut stream = TcpStream::connect(&authority).await.unwrap();
    let request = format!("GET /api/v10/nope HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    assert!(response.starts_with("HTTP/1.1 404"), "{response}");
    assert!(response.contains("10000"), "{response}");
}

#[tokio::test]
async fn world_gives_a_guild_payload_with_its_channels() {
    let server = MockServer::start().await.unwrap();
    let guild = server.world().add_guild("g");
    let channel = server.world().add_channel(guild, "general");
    let payload = server.world().guild_payload(guild).unwrap();
    assert_eq!(payload["channels"][0]["id"], channel.to_string());
    assert_eq!(server.world().guild_ids(), vec![guild]);
}
