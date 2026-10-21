//! Completed-game archiving (BE-135).
//!
//! The `archiving` crate has been a workspace member since BE-95 and has
//! never been called: completed games produced a rating update and then were
//! never written anywhere durable.
//!
//! This wires it to `PUT /v1/games/{id}/complete`. Archiving is opt-in and
//! off by default — it needs IPFS and/or Arweave gateways configured, and a
//! deployment that has not set them up must not start shelling out to the
//! network. When it is off, `complete_game` behaves exactly as before.
//!
//! When it is on, the work happens on a detached task. Archiving uploads bytes
//! to third-party networks and can take seconds; the player waiting on their
//! new rating should not wait for an IPFS round trip. Failures land in the
//! crate's dead-letter queue rather than failing the player's request.

use archiving::{
    ArchiveMetadata, ArchiveNetwork, ArchiveRequest, ArchiveVisibility, GameMetadata, GameType,
    PGNArchiver, PGNGame, TimeControlCategory,
};
use chrono::Utc;
use serde::Serialize;
use std::env;
use tracing::{info, warn};
use uuid::Uuid;

/// Where completed-game PGNs get mirrored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArchiveSettings {
    pub ipfs_gateway: Option<String>,
    pub arweave_gateway: Option<String>,
}

impl ArchiveSettings {
    /// Read from `IPFS_GATEWAY` / `ARWEAVE_GATEWAY`. Absent both, archiving is
    /// disabled.
    pub fn from_env() -> Self {
        let clean = |key: &str| env::var(key).ok().filter(|v| !v.trim().is_empty());
        Self {
            ipfs_gateway: clean("IPFS_GATEWAY"),
            arweave_gateway: clean("ARWEAVE_GATEWAY"),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.ipfs_gateway.is_some() || self.arweave_gateway.is_some()
    }

    pub fn preferred_network(&self) -> ArchiveNetwork {
        match (self.ipfs_gateway.is_some(), self.arweave_gateway.is_some()) {
            (true, true) => ArchiveNetwork::Both,
            (true, false) => ArchiveNetwork::IPFS,
            _ => ArchiveNetwork::Arweave,
        }
    }
}

/// Everything the archiver needs that is not in the request body.
#[derive(Debug, Clone)]
pub struct ArchiveJob {
    pub game_id: Uuid,
    pub white: String,
    pub black: String,
    pub result: String,
    pub time_control: String,
    pub moves: Vec<String>,
    pub rated: bool,
    pub started_at: chrono::DateTime<Utc>,
    pub completed_at: chrono::DateTime<Utc>,
}

impl ArchiveJob {
    fn to_request(&self, network: ArchiveNetwork) -> ArchiveRequest {
        let duration_seconds = (self.completed_at - self.started_at)
            .num_seconds()
            .max(0) as u64;

        ArchiveRequest {
            game_id: self.game_id,
            pgn_data: PGNGame {
                id: self.game_id,
                event: "KnightVerse Match".to_string(),
                site: "knightverse.io".to_string(),
                date: self.started_at.format("%Y.%m.%d").to_string(),
                round: "-".to_string(),
                white: self.white.clone(),
                black: self.black.clone(),
                result: self.result.clone(),
                white_elo: None,
                black_elo: None,
                time_control: self.time_control.clone(),
                eco: None,
                opening: None,
                moves: self.moves.clone(),
                annotations: None,
                metadata: GameMetadata {
                    game_id: self.game_id,
                    tournament_id: None,
                    created_at: self.started_at,
                    completed_at: self.completed_at,
                    duration_seconds,
                    total_moves: (self.moves.len() / 2) as u16,
                    average_move_time: if self.moves.is_empty() {
                        0.0
                    } else {
                        duration_seconds as f64 / self.moves.len() as f64
                    },
                    time_control_category: TimeControlCategory::Rapid,
                    game_type: if self.rated {
                        GameType::Rated
                    } else {
                        GameType::Casual
                    },
                },
            },
            archive_immediately: true,
            preferred_network: network,
            metadata: ArchiveMetadata {
                tags: vec![
                    "knightverse".to_string(),
                    format!("game:{}", self.game_id),
                ],
                description: Some(format!("Archived KnightVerse game {}", self.game_id)),
                visibility: ArchiveVisibility::Public,
                encryption_key: None,
                compression: true,
            },
        }
    }
}

/// Spawn the archive task. Returns immediately.
///
/// No-ops (with a debug log) when archiving is not configured, so callers do
/// not need to branch.
pub fn spawn_archive(settings: ArchiveSettings, job: ArchiveJob) {
    if !settings.is_enabled() {
        return;
    }

    let ipfs_gateway = settings.ipfs_gateway.clone();
    let arweave_gateway = settings.arweave_gateway.clone();
    let network = settings.preferred_network();
    let game_id = job.game_id;

    actix::spawn(async move {
        // `PGNArchiver::new` takes both gateways; fall back to a placeholder so
        // the constructor signature stays meaningful for a single-network setup.
        let archiver = PGNArchiver::new(
            db::db::db::get_db().await,
            ipfs_gateway.unwrap_or_default(),
            arweave_gateway.unwrap_or_default(),
        );

        match archiver.archive_game(job.to_request(network)).await {
            Ok(result) => info!(
                "Archived game {} to {:?} ({} bytes, hash {})",
                game_id,
                result.network,
                result.file_size_bytes,
                result.content_hash
            ),
            // Already recorded in the crate's dead-letter queue by
            // `archive_game`; log so it is visible in the API's output.
            Err(e) => warn!("Archiving game {} failed: {}", game_id, e),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job() -> ArchiveJob {
        let started = Utc::now();
        ArchiveJob {
            game_id: Uuid::new_v4(),
            white: "GWHITE".to_string(),
            black: "GBLACK".to_string(),
            result: "1-0".to_string(),
            time_control: "600+5".to_string(),
            moves: vec!["e4".into(), "e5".into(), "Nf3".into()],
            rated: true,
            started_at: started,
            completed_at: started,
        }
    }

    #[test]
    fn disabled_when_no_gateways_are_configured() {
        let settings = ArchiveSettings {
            ipfs_gateway: None,
            arweave_gateway: None,
        };
        assert!(!settings.is_enabled());
        // `preferred_network` must not panic even when disabled.
        let _ = settings.preferred_network();
    }

    #[test]
    fn ipfs_only_selects_ipfs() {
        let settings = ArchiveSettings {
            ipfs_gateway: Some("http://ipfs:5001".to_string()),
            arweave_gateway: None,
        };
        assert!(settings.is_enabled());
        assert_eq!(settings.preferred_network(), ArchiveNetwork::IPFS);
    }

    #[test]
    fn arweave_only_selects_arweave() {
        let settings = ArchiveSettings {
            ipfs_gateway: None,
            arweave_gateway: Some("https://arweave.net".to_string()),
        };
        assert_eq!(settings.preferred_network(), ArchiveNetwork::Arweave);
    }

    #[test]
    fn both_gateways_select_both() {
        let settings = ArchiveSettings {
            ipfs_gateway: Some("http://ipfs:5001".to_string()),
            arweave_gateway: Some("https://arweave.net".to_string()),
        };
        assert_eq!(settings.preferred_network(), ArchiveNetwork::Both);
    }

    #[test]
    fn job_maps_onto_a_well_formed_archive_request() {
        let job = job();
        let request = job.to_request(ArchiveNetwork::IPFS);

        assert_eq!(request.game_id, job.game_id);
        assert_eq!(request.preferred_network, ArchiveNetwork::IPFS);
        assert_eq!(request.pgn_data.white, "GWHITE");
        assert_eq!(request.pgn_data.black, "GBLACK");
        assert_eq!(request.pgn_data.result, "1-0");
        assert_eq!(request.pgn_data.moves.len(), 3);
        assert!(matches!(request.pgn_data.metadata.game_type, GameType::Rated));
        // 3 plies is one full move plus one, rounded down.
        assert_eq!(request.pgn_data.metadata.total_moves, 1);
        assert!(request.metadata.tags.iter().any(|t| t == "knightverse"));
    }

    #[test]
    fn casual_games_are_tagged_as_casual() {
        let mut job = job();
        job.rated = false;
        let request = job.to_request(ArchiveNetwork::Arweave);
        assert!(matches!(request.pgn_data.metadata.game_type, GameType::Casual));
    }
}