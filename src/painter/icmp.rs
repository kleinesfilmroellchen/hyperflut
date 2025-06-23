//! ICMP send and receive tooling.
//!
//! This module is only available in std environments.

use socket2::{Domain, Protocol, Socket, Type};
use std::{io, net::SocketAddr};

/// Includes both the real header (4 bytes) as well as the echo standard data (4 bytes).
pub const ICMP_HEADER_SIZE: usize = 8;
pub const IPV4_HEADER_SIZE: usize = 20;
pub const ECHO_REQUEST_V4: u8 = 8;
pub const ECHO_REQUEST_V6: u8 = 128;
pub const ECHO_REPLY_V4: u8 = 0;
pub const ECHO_REPLY_V6: u8 = 129;

/// The two kinds of echo packets, request and reply.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EchoDirection {
    Request,
    Reply,
}

/// An ICMP v4/v6 Echo Request packet.
/// Provides functionality to send out Echo Request messages (pings) and capture their response.
// TODO: ICMPv6 is not implemented yet.
pub struct Icmp {
    direction: EchoDirection,
    /// Ping identifier, part of the standard payload.
    identifier: u16,
    /// Target address.
    target: SocketAddr,
    // Raw packet data, reused across subsequent packet sends for performance.
    packet: Vec<u8>,
    /// Non-standard payload to be sent.
    payload: Vec<u8>,
    /// Ping sequence number, part of the standard payload.
    current_sequence_number: u16,
}

impl Icmp {
    /// Create a new ICMP packet.
    ///
    /// - `target`: The target address of the ping.
    /// - `identifier`: The identifier of the ping.
    /// - `direction`: The echo direction, i.e. an echo request or reply.
    pub fn new(target: SocketAddr, identifier: u16, direction: EchoDirection) -> Self {
        Icmp {
            direction,
            identifier,
            target,
            packet: [0; ICMP_HEADER_SIZE].to_vec(),
            payload: Vec::new(),
            current_sequence_number: 0,
        }
    }

    /// Set this ICMP packet’s custom payload.
    /// The first four bytes of the Echo Request packet are semi-standard and not affected by this payload.
    pub fn set_payload(&mut self, payload: Vec<u8>) {
        self.payload = payload;
    }

    /// lowest priority DSCP
    const DSCP_LOW_PRIORITY: u32 = 8 << 2;

    /// Send this ICMP packet.
    /// Apart from the send action this has the additional effect of incrementing the sequence number of this packet.
    ///
    /// Returns the socket used for sending so that responses can be received.
    pub fn send(&mut self) -> Result<Socket, io::Error> {
        self.encode();
        let socket = if self.target.is_ipv4() {
            Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4))?
        } else {
            Socket::new(Domain::IPV6, Type::RAW, Some(Protocol::ICMPV6))?
        };
        if self.target.is_ipv4() {
            socket.set_tos(Self::DSCP_LOW_PRIORITY)?;
        } else {
            socket.set_tclass_v6(Self::DSCP_LOW_PRIORITY)?;
        }

        socket.send_to(&self.packet, &self.target.into())?;

        self.current_sequence_number = self.current_sequence_number.wrapping_add(1);
        self.update_seq(self.current_sequence_number);

        Ok(socket)
    }

    /// Encode this packet’s data.
    fn encode(&mut self) {
        self.packet.truncate(ICMP_HEADER_SIZE);
        self.packet[0] = match (self.target.is_ipv4(), self.direction) {
            (true, EchoDirection::Request) => ECHO_REQUEST_V4,
            (true, EchoDirection::Reply) => ECHO_REPLY_V4,
            (false, EchoDirection::Request) => ECHO_REQUEST_V6,
            (false, EchoDirection::Reply) => ECHO_REPLY_V6,
        };

        self.packet[1] = 0;
        self.packet[4] = (self.identifier >> 8) as u8;
        self.packet[5] = self.identifier as u8;
        self.packet[6] = 0;
        self.packet[7] = 0;
        self.packet.append(&mut self.payload.clone());
        self.checksum();
    }

    /// Update this packet’s sequence number.
    fn update_seq(&mut self, seq: u16) {
        self.packet[2] = 0;
        self.packet[3] = 0;
        self.packet[6] = (seq >> 8) as u8;
        self.packet[7] = seq as u8;
        self.checksum();
    }

    /// Update this packet’s checksum.
    fn checksum(&mut self) {
        let mut sum = 0u32;
        for word in self.packet.chunks(2) {
            let mut part = u16::from(word[0]) << 8;
            if word.len() > 1 {
                part += u16::from(word[1]);
            }
            sum = sum.wrapping_add(u32::from(part));
        }
        while (sum >> 16) > 0 {
            sum = (sum & 0xffff) + (sum >> 16);
        }
        let sum = !sum as u16;
        self.packet[2] = (sum >> 8) as u8;
        self.packet[3] = (sum & 0xff) as u8;
    }
}
