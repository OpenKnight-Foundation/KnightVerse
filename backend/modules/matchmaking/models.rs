use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;
use chrono::{DateTime, Utc};


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MatchType {
    Rated,
    Casual,
    Private,
}

impl MatchType {
    pub fn redis_key(&self) -> String {
        match self {
            MatchType::Rated => "matchmaking:queue:rated".to_string(),
            MatchType::Casual => "matchmaking:queue:casual".to_string(),
            MatchType::Private => "matchmaking:invites".to_string(),
        }
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
    pub match_type: MatchType,
    pub invite_address: Option<String>,
    pub max_elo_diff: Option<u32>,
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
    pub match_type: MatchType,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStatus {
    pub request_id: Uuid,
    pub position: usize,
    pub estimated_wait_time: Duration,
    pub match_type: MatchType,
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
    fn test_match_type_redis_key() {
        assert_eq!(MatchType::Rated.redis_key(), "matchmaking:queue:rated");
        assert_eq!(MatchType::Casual.redis_key(), "matchmaking:queue:casual");
        assert_eq!(MatchType::Private.redis_key(), "matchmaking:invites");
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
            match_type: MatchType::Rated,
            invite_address: None,
            max_elo_diff: Some(100),
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
            match_type: MatchType::Private,
            invite_address: Some("GINVITEE123".to_string()),
            max_elo_diff: None,
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
            match_type: MatchType::Casual,
            invite_address: None,
            max_elo_diff: None,
        };

        let json = req.to_redis_value().expect("Should serialize");
        let deserialized = MatchRequest::from_redis_value(&json).expect("Should deserialize");

        assert_eq!(req, deserialized);
    }
}
