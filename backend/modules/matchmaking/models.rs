use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum QueueType {
    RatedStaked { token: String, amount: u64 },
    RatedFree,
    CasualUnrated,
    Private,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StakeInfo {
    pub token: String,
    pub amount: u64,
    pub escrow_signature: Option<String>,
}

impl QueueType {
    pub fn redis_key(&self) -> String {
        match self {
            QueueType::RatedStaked { token, amount } => {
                format!("matchmaking:queue:rated_staked:{}:{}", token, amount)
            }
            QueueType::RatedFree => "matchmaking:queue:rated_free".to_string(),
            QueueType::CasualUnrated => "matchmaking:queue:casual_unrated".to_string(),
            QueueType::Private => "matchmaking:invites".to_string(),
        }
    }
}

/// Time control selected by the player (maps to frontend variant IDs).
/// Serialized as a simple string, e.g. "10+0", "5+0", "3+0", "1+0".
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimeControl(pub String);

impl TimeControl {
    /// Return the initial time in seconds for this time control.
    pub fn initial_seconds(&self) -> u64 {
        match self.0.as_str() {
            "bullet" => 60,
            "blitz" => 180,
            "rapid" => 480,
            _ => 600,
        }
    }

    pub fn increment_seconds(&self) -> u64 {
        0
    }
}

impl Default for TimeControl {
    fn default() -> Self {
        TimeControl("standard".to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Player {
    pub wallet_address: String,
    pub elo: u32,
    pub join_time: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatchRequest {
    pub id: Uuid,
    pub player: Player,
    pub queue_type: QueueType,
    pub invite_address: Option<String>,
    pub max_elo_diff: Option<u32>,
    pub stake_info: Option<StakeInfo>,
    #[serde(default)]
    pub time_control: TimeControl,
}

impl MatchRequest {
    pub fn to_redis_value(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_redis_value(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Match {
    pub id: Uuid,
    pub player1: Player,
    pub player2: Player,
    pub queue_type: QueueType,
    pub created_at: DateTime<Utc>,
    pub stake_info: Option<StakeInfo>,
    #[serde(default)]
    pub time_control: TimeControl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStatus {
    pub request_id: Uuid,
    pub position: usize,
    pub estimated_wait_time: Duration,
    pub queue_type: QueueType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchmakingResponse {
    pub status: String,
    pub match_id: Option<Uuid>,
    pub request_id: Uuid,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_type_redis_key() {
        assert_eq!(QueueType::RatedFree.redis_key(), "matchmaking:queue:rated_free");
        assert_eq!(QueueType::CasualUnrated.redis_key(), "matchmaking:queue:casual_unrated");
        assert_eq!(QueueType::Private.redis_key(), "matchmaking:invites");
        assert_eq!(
            QueueType::RatedStaked { token: "USDC".to_string(), amount: 100 }.redis_key(),
            "matchmaking:queue:rated_staked:USDC:100"
        );
    }

    #[test]
    fn test_match_request_round_trip() {
        let join_time = Utc::now();
        let player = Player {
            wallet_address: "GABC1234567890ABCDEF".to_string(),
            elo: 1500,
            join_time,
        };
        let req = MatchRequest {
            id: Uuid::new_v4(),
            player,
            queue_type: QueueType::RatedFree,
            invite_address: None,
            max_elo_diff: Some(100),
            stake_info: None,
            time_control: TimeControl::default(),
        };

        let json = req.to_redis_value().expect("Should serialize");
        let deserialized = MatchRequest::from_redis_value(&json).expect("Should deserialize");

        // Full structural equality — verifies every field survives the round-trip
        assert_eq!(req, deserialized);
    }

    #[test]
    fn test_match_request_with_invite_address_round_trip() {
        let req = MatchRequest {
            id: Uuid::new_v4(),
            player: Player {
                wallet_address: "GXYZ987".to_string(),
                elo: 1200,
                join_time: Utc::now(),
            },
            queue_type: QueueType::Private,
            invite_address: Some("GINVITEE123".to_string()),
            max_elo_diff: None,
            stake_info: None,
            time_control: TimeControl::default(),
        };

        let json = req.to_redis_value().expect("Should serialize");
        let deserialized = MatchRequest::from_redis_value(&json).expect("Should deserialize");

        assert_eq!(req, deserialized);
    }

    #[test]
    fn test_casual_match_request_round_trip() {
        let req = MatchRequest {
            id: Uuid::new_v4(),
            player: Player {
                wallet_address: "GCASUAL999".to_string(),
                elo: 800,
                join_time: Utc::now(),
            },
            queue_type: QueueType::CasualUnrated,
            invite_address: None,
            max_elo_diff: None,
            stake_info: None,
            time_control: TimeControl::default(),
        };

        let json = req.to_redis_value().expect("Should serialize");
        let deserialized = MatchRequest::from_redis_value(&json).expect("Should deserialize");

        assert_eq!(req, deserialized);
    }

    #[test]
    fn test_staked_match_request_round_trip() {
        let stake_info = StakeInfo {
            token: "USDC".to_string(),
            amount: 100,
            escrow_signature: Some("valid_signature".to_string()),
        };
        let req = MatchRequest {
            id: Uuid::new_v4(),
            player: Player {
                wallet_address: "GSTAKED123".to_string(),
                elo: 1600,
                join_time: Utc::now(),
            },
            queue_type: QueueType::RatedStaked { token: "USDC".to_string(), amount: 100 },
            invite_address: None,
            max_elo_diff: Some(150),
            stake_info: Some(stake_info),
            time_control: TimeControl::default(),
        };

        let json = req.to_redis_value().expect("Should serialize");
        let deserialized = MatchRequest::from_redis_value(&json).expect("Should deserialize");

        assert_eq!(req, deserialized);
    }
}