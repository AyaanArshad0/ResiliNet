use serde::{Deserialize, Serialize};

/// The type of frame traveling through the tunnel.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum FrameType {
    /// Standard encrypted transport frame containing IP packet.
    Transport,
    /// Keep-alive packet (Chaff) to maintain NAT mappings.
    Heartbeat,
    /// Fake Handshake (Obfuscation) to look like TLS.
    Handshake,
}

/// The headers for our Ghost Protocol (Wire Format).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FrameHeader {
    /// Monotonic sequence number.
    /// Used for:
    /// 1. Reordering (if we implement a reorder buffer later).
    /// 2. Basic replay protection.
    /// 3. Jitter calcluation (diff between send/recv times).
    pub seq: u64,
    /// The type of payload.
    pub frame_type: FrameType,
}

/// The Atomic Unit of the Ghost Protocol.
/// This matches the MTU size + Overhead.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WireFrame {
    pub header: FrameHeader,
    /// Encrypted payload (Poly1305 tag included).
    pub payload: Vec<u8>,
}

impl WireFrame {
    /// Create a new data frame ready for the wire.
    pub fn new_data(seq: u64, payload: Vec<u8>) -> Self {
        Self {
            header: FrameHeader {
                seq,
                frame_type: FrameType::Transport,
            },
            payload,
        }
    }

    /// Create a heartbeat frame to keep middleboxes happy.
    pub fn new_heartbeat(seq: u64) -> Self {
        Self {
            header: FrameHeader {
                seq,
                frame_type: FrameType::Heartbeat,
            },
            payload: vec![],
        }
    }
}
