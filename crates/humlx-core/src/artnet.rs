//! Art-Net 4 packet encoding and decoding.
//!
//! Only the packets HumLX needs are implemented: `ArtDmx` for output, and
//! `ArtPoll` / `ArtPollReply` for discovering nodes on the network.

use std::net::Ipv4Addr;

use serde::{Deserialize, Serialize};

use crate::dmx::UNIVERSE_SIZE;

/// UDP port used by all Art-Net traffic.
pub const PORT: u16 = 6454;

const ID: &[u8; 8] = b"Art-Net\0";
const PROTOCOL_VERSION: u16 = 14;

const OP_POLL: u16 = 0x2000;
const OP_POLL_REPLY: u16 = 0x2100;
const OP_DMX: u16 = 0x5000;

/// Highest valid 15-bit Art-Net port-address (net, sub-net and universe combined).
pub const MAX_PORT_ADDRESS: u16 = 0x7FFF;

/// Builds an `ArtDmx` packet carrying a full universe.
///
/// `port_address` is the 15-bit Art-Net address: bits 14-8 are the net,
/// bits 7-4 the sub-net and bits 3-0 the universe. `sequence` should
/// increment with each packet and wrap from 255 to 1; 0 disables
/// re-ordering on the receiving node.
pub fn dmx_packet(port_address: u16, sequence: u8, data: &[u8; UNIVERSE_SIZE]) -> Vec<u8> {
    let mut packet = Vec::with_capacity(18 + UNIVERSE_SIZE);
    packet.extend_from_slice(ID);
    packet.extend_from_slice(&OP_DMX.to_le_bytes());
    packet.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    packet.push(sequence);
    packet.push(0); // physical input port, informational only
    packet.push((port_address & 0xFF) as u8); // SubUni
    packet.push(((port_address >> 8) & 0x7F) as u8); // Net
    packet.extend_from_slice(&(UNIVERSE_SIZE as u16).to_be_bytes());
    packet.extend_from_slice(data);
    packet
}

/// Builds an `ArtPoll` packet asking every node to reply with its details.
pub fn poll_packet() -> Vec<u8> {
    let mut packet = Vec::with_capacity(14);
    packet.extend_from_slice(ID);
    packet.extend_from_slice(&OP_POLL.to_le_bytes());
    packet.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    packet.push(0); // flags: no diagnostics, reply only when polled
    packet.push(0); // diagnostics priority
    packet
}

/// A node found on the network through `ArtPoll`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub ip: Ipv4Addr,
    pub short_name: String,
    pub long_name: String,
    /// Port-addresses of the node's DMX outputs.
    pub outputs: Vec<u16>,
}

/// Parses an `ArtPollReply` packet. Returns `None` for any other packet.
pub fn parse_poll_reply(packet: &[u8]) -> Option<Node> {
    // The fields we read end at the SwOut array (offset 190..194).
    if packet.len() < 194 || &packet[0..8] != ID {
        return None;
    }
    if u16::from_le_bytes([packet[8], packet[9]]) != OP_POLL_REPLY {
        return None;
    }
    let ip = Ipv4Addr::new(packet[10], packet[11], packet[12], packet[13]);
    let net = u16::from(packet[18] & 0x7F);
    let sub_net = u16::from(packet[19] & 0x0F);
    let port_count = usize::from(packet[173].min(4));
    let port_types = &packet[174..178];
    let outputs = (0..port_count)
        .filter(|&i| port_types[i] & 0x80 != 0) // port can output DMX
        .map(|i| (net << 8) | (sub_net << 4) | u16::from(packet[190 + i] & 0x0F))
        .collect();
    Some(Node {
        ip,
        short_name: c_string(&packet[26..44]),
        long_name: c_string(&packet[44..108]),
        outputs,
    })
}

fn c_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dmx_packet_layout() {
        let mut data = [0u8; UNIVERSE_SIZE];
        data[0] = 255;
        data[511] = 7;
        let packet = dmx_packet(0x0123, 9, &data);

        assert_eq!(packet.len(), 18 + 512);
        assert_eq!(&packet[0..8], b"Art-Net\0");
        assert_eq!(&packet[8..10], &[0x00, 0x50]); // OpDmx, little endian
        assert_eq!(&packet[10..12], &[0, 14]); // protocol version, big endian
        assert_eq!(packet[12], 9); // sequence
        assert_eq!(packet[14], 0x23); // SubUni
        assert_eq!(packet[15], 0x01); // Net
        assert_eq!(&packet[16..18], &[0x02, 0x00]); // length 512, big endian
        assert_eq!(packet[18], 255);
        assert_eq!(packet[18 + 511], 7);
    }

    #[test]
    fn poll_packet_layout() {
        let packet = poll_packet();
        assert_eq!(packet.len(), 14);
        assert_eq!(&packet[8..10], &[0x00, 0x20]);
    }

    fn sample_reply() -> Vec<u8> {
        let mut packet = vec![0u8; 239];
        packet[0..8].copy_from_slice(ID);
        packet[8..10].copy_from_slice(&OP_POLL_REPLY.to_le_bytes());
        packet[10..14].copy_from_slice(&[192, 168, 1, 50]);
        packet[18] = 0; // net
        packet[19] = 1; // sub-net
        packet[26..32].copy_from_slice(b"Node 1");
        packet[44..55].copy_from_slice(b"Studio node");
        packet[173] = 2; // two ports
        packet[174] = 0x80; // port 1 outputs DMX
        packet[175] = 0x40; // port 2 is input only
        packet[190] = 3;
        packet
    }

    #[test]
    fn parses_poll_reply() {
        let node = parse_poll_reply(&sample_reply()).expect("valid reply");
        assert_eq!(node.ip, Ipv4Addr::new(192, 168, 1, 50));
        assert_eq!(node.short_name, "Node 1");
        assert_eq!(node.long_name, "Studio node");
        assert_eq!(node.outputs, vec![0x13]);
    }

    #[test]
    fn ignores_other_packets() {
        assert_eq!(parse_poll_reply(&poll_packet()), None);
        let mut short = sample_reply();
        short.truncate(100);
        assert_eq!(parse_poll_reply(&short), None);
    }
}
