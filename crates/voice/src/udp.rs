//! The UDP side: IP discovery and keepalives.
//!
//! IP discovery is a 74 byte packet: type 1 (request) or 2 (response), length 70, the SSRC, a 64
//! byte NUL-padded address, and the port, all big endian.

use std::net::Ipv4Addr;

/// The length of an IP discovery packet.
pub const DISCOVERY_LEN: usize = 74;

/// The request for this client's public address.
pub fn discovery_request(ssrc: u32) -> [u8; DISCOVERY_LEN] {
    let mut p = [0u8; DISCOVERY_LEN];
    p[0..2].copy_from_slice(&1u16.to_be_bytes());
    p[2..4].copy_from_slice(&70u16.to_be_bytes());
    p[4..8].copy_from_slice(&ssrc.to_be_bytes());
    p
}

/// The answer to a discovery request, for the server side of a test.
pub fn discovery_response(ssrc: u32, ip: Ipv4Addr, port: u16) -> [u8; DISCOVERY_LEN] {
    let mut p = [0u8; DISCOVERY_LEN];
    p[0..2].copy_from_slice(&2u16.to_be_bytes());
    p[2..4].copy_from_slice(&70u16.to_be_bytes());
    p[4..8].copy_from_slice(&ssrc.to_be_bytes());
    let text = ip.to_string();
    p[8..8 + text.len()].copy_from_slice(text.as_bytes());
    p[72..74].copy_from_slice(&port.to_be_bytes());
    p
}

/// The public address and port in a discovery response, or `None` for any other packet.
pub fn parse_discovery(packet: &[u8]) -> Option<(Ipv4Addr, u16)> {
    if packet.len() != DISCOVERY_LEN || packet[0..2] != 2u16.to_be_bytes() {
        return None;
    }
    let address = &packet[8..72];
    let end = address.iter().position(|b| *b == 0).unwrap_or(address.len());
    let ip = std::str::from_utf8(&address[..end]).ok()?.parse().ok()?;
    Some((ip, u16::from_be_bytes([packet[72], packet[73]])))
}

/// An 8 byte keepalive carrying a little endian counter, as upstream sends every 5 s.
pub fn keepalive(counter: u32) -> [u8; 8] {
    let mut p = [0u8; 8];
    p[0..4].copy_from_slice(&counter.to_le_bytes());
    p
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn discovery_round_trips() {
        let request = discovery_request(0x0102_0304);
        assert_eq!(&request[..8], &[0, 1, 0, 70, 1, 2, 3, 4]);
        assert!(parse_discovery(&request).is_none(), "a request is not an answer");
        let answer = discovery_response(5, Ipv4Addr::new(203, 0, 113, 9), 50_000);
        assert_eq!(parse_discovery(&answer), Some((Ipv4Addr::new(203, 0, 113, 9), 50_000)));
        assert!(parse_discovery(&answer[..10]).is_none());
        assert_eq!(keepalive(1), [1, 0, 0, 0, 0, 0, 0, 0]);
    }
}
