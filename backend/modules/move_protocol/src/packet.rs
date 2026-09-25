//! Wire types exchanged between a move client and the server.

use serde::{Deserialize, Serialize};

/// A single client move, tagged with the client's monotonically increasing
/// sequence id and the client's own wall-clock send time.
///
/// The sequence id — not the timestamp — is what defines move order: clients
/// that share one game agree on a single counter, so a packet's `seq` is
/// meaningful even when its `client_ts_ms` is skewed by clock drift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MovePacket {
    /// Identifies the player (and therefore the game this move belongs to).
    pub player_id: String,
    /// Strictly increasing per-player sequence id, starting at 1.
    pub seq: u64,
    /// The move in SAN/UCI notation.
    pub move_notation: String,
    /// Client wall-clock time the move was sent, in milliseconds.
    pub client_ts_ms: u64,
}

impl MovePacket {
    /// Build a move packet from its parts.
    pub fn new(
        player_id: impl Into<String>,
        seq: u64,
        move_notation: impl Into<String>,
        client_ts_ms: u64,
    ) -> Self {
        Self {
            player_id: player_id.into(),
            seq,
            move_notation: move_notation.into(),
            client_ts_ms,
        }
    }
}

/// Server → client clock probe. The server records `server_sent_ms` locally
/// as well, so the payload only needs to carry the probe id and its send time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ping {
    /// Correlates this probe with the client's [`Pong`].
    pub id: u64,
    /// Server wall-clock time the ping was sent, in milliseconds.
    pub server_sent_ms: u64,
}

/// Client → server reply to a [`Ping`], carrying the client's receive and
/// send timestamps so the server can solve for the clock offset (NTP style).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pong {
    /// The [`Ping::id`] being answered.
    pub id: u64,
    /// Client wall-clock time the ping arrived, in milliseconds.
    pub client_recv_ms: u64,
    /// Client wall-clock time the pong was sent, in milliseconds.
    pub client_send_ms: u64,
}

impl Pong {
    /// Build a pong for `ping_id`, echoing the client's receive/send times.
    pub fn new(ping_id: u64, client_recv_ms: u64, client_send_ms: u64) -> Self {
        Self {
            id: ping_id,
            client_recv_ms,
            client_send_ms,
        }
    }
}
