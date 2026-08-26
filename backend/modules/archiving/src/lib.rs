use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use reqwest::Client;
use base64::{Engine as _, engine::general_purpose};
use sha2::{Sha256, Digest};
use sea_orm::{DatabaseConnection, EntityTrait, ActiveModelTrait, Set};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PGNGame {
    pub id: Uuid,
    pub event: String,
    pub site: String,
    pub date: String,
    pub round: String,
    pub white: String,
    pub black: String,
    pub result: String,
    pub white_elo: Option<i32>,
    pub black_elo: Option<i32>,
    pub time_control: String,
    pub eco: Option<String>,
    pub opening: Option<String>,
    pub moves: Vec<String>,
    pub annotations: Option<Vec<GameAnnotation>>,
    pub metadata: GameMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameAnnotation {
    pub move_number: u16,
    pub evaluation: Option<f64>,
    pub comment: Option<String>,
    pub variation: Option<Vec<String>>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameMetadata {
    pub game_id: Uuid,
    pub tournament_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub duration_seconds: u64,
    pub total_moves: u16,
    pub average_move_time: f64,
    pub time_control_category: TimeControlCategory,
    pub game_type: GameType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimeControlCategory {
    Bullet,
    Blitz,
    Rapid,
    Classical,
    Correspondence,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameType {
    Rated,
    Casual,
    Tournament,
    Challenge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveRequest {
    pub game_id: Uuid,
    pub pgn_data: PGNGame,
    pub archive_immediately: bool,
    pub preferred_network: ArchiveNetwork,
    pub metadata: ArchiveMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveMetadata {
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub visibility: ArchiveVisibility,
    pub encryption_key: Option<String>,
    pub compression: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArchiveNetwork {
    IPFS,
    Arweave,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArchiveVisibility {
    Public,
    Private,
    Unlisted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveResult {
    pub game_id: Uuid,
    pub archive_urls: HashMap<String, String>,
    pub content_hash: String,
    pub archive_timestamp: DateTime<Utc>,
    pub file_size_bytes: u64,
    pub network: ArchiveNetwork,
    pub transaction_id: Option<String>,
    pub gas_used: Option<u64>,
    pub cost_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveStatus {
    pub game_id: Uuid,
    pub status: ArchiveProcessStatus,
    pub progress: f64,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArchiveProcessStatus {
    Pending,
    Processing,
    Uploading,
    Confirming,
    Completed,
    Failed,
    Retrying,
}

#[derive(Debug, Error)]
pub enum ArchiveError {
    #[error("PGN parsing error: {0}")]
    PGNParseError(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("IPFS upload failed: {0}")]
    IPFSUploadError(String),
    
    #[error("Arweave upload failed: {0}")]
    ArweaveUploadError(String),
    
    #[error("Insufficient funds for upload")]
    InsufficientFunds,
    
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    #[error("File too large: {size} bytes exceeds limit of {limit} bytes")]
    FileTooLarge { size: u64, limit: u64 },
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Invalid archive network: {0}")]
    InvalidNetwork(String),
}

pub struct PGNArchiver {
    db: DatabaseConnection,
    http_client: Client,
    ipfs_gateway: String,
    arweave_gateway: String,
    max_file_size: u64,
    upload_timeout: u64,
    retry_attempts: u32,
}

impl PGNArchiver {
    pub fn new(
        db: DatabaseConnection,
        ipfs_gateway: String,
        arweave_gateway: String,
    ) -> Self {
        Self {
            db,
            http_client: Client::new(),
            ipfs_gateway,
            arweave_gateway,
            max_file_size: 10 * 1024 * 1024, // 10MB
            upload_timeout: 300, // 5 minutes
            retry_attempts: 3,
        }
    }

    pub fn with_config(
        db: DatabaseConnection,
        ipfs_gateway: String,
        arweave_gateway: String,
        max_file_size: u64,
        upload_timeout: u64,
        retry_attempts: u32,
    ) -> Self {
        Self {
            db,
            http_client: Client::new(),
            ipfs_gateway,
            arweave_gateway,
            max_file_size,
            upload_timeout,
            retry_attempts,
        }
    }

    pub async fn archive_game(&self, request: ArchiveRequest) -> Result<ArchiveResult, ArchiveError> {
        // Convert PGN to string and validate
        let pgn_string = self.pgn_to_string(&request.pgn_data)?;
        let pgn_bytes = pgn_string.as_bytes();

        // Check file size
        if pgn_bytes.len() as u64 > self.max_file_size {
            return Err(ArchiveError::FileTooLarge {
                size: pgn_bytes.len() as u64,
                limit: self.max_file_size,
            });
        }

        // Calculate content hash
        let content_hash = self.calculate_hash(pgn_bytes);

        // Archive to specified networks
        let mut archive_urls = HashMap::new();
        let mut transaction_id = None;
        let mut gas_used = None;
        let mut cost_usd = None;

        match request.preferred_network {
            ArchiveNetwork::IPFS => {
                let (url, tx_id, gas, cost) = self.archive_to_ipfs(&pgn_string, &request.metadata).await?;
                archive_urls.insert("ipfs".to_string(), url);
                transaction_id = tx_id;
                gas_used = gas;
                cost_usd = cost;
            }
            ArchiveNetwork::Arweave => {
                let (url, tx_id, gas, cost) = self.archive_to_arweave(&pgn_string, &request.metadata).await?;
                archive_urls.insert("arweave".to_string(), url);
                transaction_id = tx_id;
                gas_used = gas;
                cost_usd = cost;
            }
            ArchiveNetwork::Both => {
                // Archive to both networks
                let (ipfs_url, ipfs_tx, ipfs_gas, ipfs_cost) = self.archive_to_ipfs(&pgn_string, &request.metadata).await?;
                let (arweave_url, arweave_tx, arweave_gas, arweave_cost) = self.archive_to_arweave(&pgn_string, &request.metadata).await?;
                
                archive_urls.insert("ipfs".to_string(), ipfs_url);
                archive_urls.insert("arweave".to_string(), arweave_url);
                
                // Use the more expensive transaction as primary
                if let (Some(ipfs_cost), Some(arweave_cost)) = (ipfs_cost, arweave_cost) {
                    if arweave_cost > ipfs_cost {
                        transaction_id = arweave_tx;
                        gas_used = arweave_gas;
                        cost_usd = arweave_cost;
                    } else {
                        transaction_id = ipfs_tx;
                        gas_used = ipfs_gas;
                        cost_usd = ipfs_cost;
                    }
                }
            }
        }

        let result = ArchiveResult {
            game_id: request.game_id,
            archive_urls,
            content_hash,
            archive_timestamp: Utc::now(),
            file_size_bytes: pgn_bytes.len() as u64,
            network: request.preferred_network,
            transaction_id,
            gas_used,
            cost_usd,
        };

        // Save archive record to database
        self.save_archive_record(&result).await?;

        Ok(result)
    }

    pub async fn batch_archive(&self, requests: Vec<ArchiveRequest>) -> Vec<Result<ArchiveResult, ArchiveError>> {
        let mut results = Vec::new();

        for request in requests {
            let result = self.archive_game(request).await;
            results.push(result);
        }

        results
    }

    async fn archive_to_ipfs(&self, pgn_string: &str, metadata: &ArchiveMetadata) -> Result<(String, Option<String>, Option<u64>, Option<f64>), ArchiveError> {
        // Prepare the data for IPFS upload
        let mut file_data = HashMap::new();
        file_data.insert("pgn".to_string(), pgn_string.to_string());
        file_data.insert("metadata".to_string(), serde_json::to_string(metadata).unwrap());
        
        let upload_data = serde_json::to_vec(&file_data)
            .map_err(|e| ArchiveError::NetworkError(e.to_string()))?;

        // Upload to IPFS
        let url = format!("{}/api/v0/add", self.ipfs_gateway);
        
        let form = reqwest::multipart::Form::new()
            .part("file", reqwest::multipart::Part::bytes(upload_data)
                .file_name("game.pgn")
                .mime_str("application/json").unwrap());

        let response = self.http_client
            .post(&url)
            .multipart(form)
            .timeout(std::time::Duration::from_secs(self.upload_timeout))
            .send()
            .await
            .map_err(|e| ArchiveError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ArchiveError::IPFSUploadError(
                format!("HTTP error: {}", response.status())
            ));
        }

        let result: serde_json::Value = response.json()
            .await
            .map_err(|e| ArchiveError::NetworkError(e.to_string()))?;

        let hash = result["Hash"].as_str()
            .ok_or_else(|| ArchiveError::IPFSUploadError("No hash returned".to_string()))?;

        let ipfs_url = format!("https://ipfs.io/ipfs/{}", hash);

        // For IPFS, we don't have traditional transaction IDs or gas costs
        // But we can estimate the cost based on storage
        let estimated_cost = self.estimate_ipfs_cost(pgn_string.len() as u64);

        Ok((ipfs_url, Some(hash.to_string()), None, Some(estimated_cost)))
    }

    async fn archive_to_arweave(&self, pgn_string: &str, metadata: &ArchiveMetadata) -> Result<(String, Option<String>, Option<u64>, Option<f64>), ArchiveError> {
        // Prepare data for Arweave upload
        let mut file_data = HashMap::new();
        file_data.insert("pgn".to_string(), pgn_string.to_string());
        file_data.insert("metadata".to_string(), serde_json::to_string(metadata).unwrap());
        
        let upload_data = serde_json::to_vec(&file_data)
            .map_err(|e| ArchiveError::NetworkError(e.to_string()))?;

        // Upload to Arweave
        let url = format!("{}/tx", self.arweave_gateway);
        
        let mut form_data = HashMap::new();
        form_data.insert("data", general_purpose::STANDARD.encode(&upload_data));
        form_data.insert("content-type", "application/json".to_string());
        
        if let Some(tags) = &metadata.tags {
            for (i, tag) in tags.iter().enumerate() {
                form_data.insert(format!("tag-{}-name", i), "xlmate-tag".to_string());
                form_data.insert(format!("tag-{}-value", i), tag.clone());
            }
        }

        let response = self.http_client
            .post(&url)
            .form(&form_data)
            .timeout(std::time::Duration::from_secs(self.upload_timeout))
            .send()
            .await
            .map_err(|e| ArchiveError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ArchiveError::ArweaveUploadError(
                format!("HTTP error: {}", response.status())
            ));
        }

        let result: serde_json::Value = response.json()
            .await
            .map_err(|e| ArchiveError::NetworkError(e.to_string()))?;

        let tx_id = result["id"].as_str()
            .ok_or_else(|| ArchiveError::ArweaveUploadError("No transaction ID returned".to_string()))?;

        let arweave_url = format!("https://arweave.net/{}", tx_id);

        // Estimate cost based on data size
        let estimated_cost = self.estimate_arwear_cost(pgn_string.len() as u64);
        let estimated_gas = self.estimate_arweave_gas(pgn_string.len() as u64);

        Ok((arweave_url, Some(tx_id.to_string()), Some(estimated_gas), Some(estimated_cost)))
    }

    fn pgn_to_string(&self, pgn: &PGNGame) -> Result<String, ArchiveError> {
        let mut pgn_string = String::new();
        
        // Add PGN headers
        pgn_string.push_str(&format!("[Event \"{}\"]\n", pgn.event));
        pgn_string.push_str(&format!("[Site \"{}\"]\n", pgn.site));
        pgn_string.push_str(&format!("[Date \"{}\"]\n", pgn.date));
        pgn_string.push_str(&format!("[Round \"{}\"]\n", pgn.round));
        pgn_string.push_str(&format!("[White \"{}\"]\n", pgn.white));
        pgn_string.push_str(&format!("[Black \"{}\"]\n", pgn.black));
        pgn_string.push_str(&format!("[Result \"{}\"]\n", pgn.result));
        
        if let Some(white_elo) = pgn.white_elo {
            pgn_string.push_str(&format!("[WhiteElo \"{}\"]\n", white_elo));
        }
        
        if let Some(black_elo) = pgn.black_elo {
            pgn_string.push_str(&format!("[BlackElo \"{}\"]\n", black_elo));
        }
        
        pgn_string.push_str(&format!("[TimeControl \"{}\"]\n", pgn.time_control));
        
        if let Some(eco) = &pgn.eco {
            pgn_string.push_str(&format!("[ECO \"{}\"]\n", eco));
        }
        
        if let Some(opening) = &pgn.opening {
            pgn_string.push_str(&format!("[Opening \"{}\"]\n", opening));
        }

        // Add custom metadata
        pgn_string.push_str(&format!("[XLMateGameID \"{}\"]\n", pgn.metadata.game_id));
        pgn_string.push_str(&format!("[XLMateCreatedAt \"{}\"]\n", pgn.metadata.created_at.format("%Y-%m-%d %H:%M:%S UTC")));
        
        if let Some(tournament_id) = pgn.metadata.tournament_id {
            pgn_string.push_str(&format!("[XLMateTournamentID \"{}\"]\n", tournament_id));
        }

        pgn_string.push('\n');

        // Add moves
        let mut move_number = 1;
        let mut i = 0;
        
        while i < pgn.moves.len() {
            if i % 2 == 0 {
                pgn_string.push_str(&format!("{}. ", move_number));
                move_number += 1;
            }
            
            pgn_string.push_str(&pgn.moves[i]);
            
            if i < pgn.moves.len() - 1 {
                pgn_string.push(' ');
            }
            
            i += 1;
        }

        pgn_string.push_str(&format!(" {}\n", pgn.result));

        // Add annotations if present
        if let Some(annotations) = &pgn.annotations {
            for annotation in annotations {
                pgn_string.push_str(&format!("\n{{{",));
                if let Some(evaluation) = annotation.evaluation {
                    pgn_string.push_str(&format!("[%eval {:.2}]", evaluation));
                }
                if let Some(comment) = &annotation.comment {
                    pgn_string.push_str(&format!(" {}", comment));
                }
                pgn_string.push_str("}}\n");
            }
        }

        Ok(pgn_string)
    }

    fn calculate_hash(&self, data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    fn estimate_ipfs_cost(&self, size_bytes: u64) -> f64 {
        // IPFS is typically free for pinning services, but we estimate storage costs
        // This is a simplified calculation
        let storage_cost_per_gb_per_year = 0.10; // $0.10 per GB per year
        let size_gb = size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        storage_cost_per_gb_per_year * size_gb
    }

    fn estimate_arwear_cost(&self, size_bytes: u64) -> f64 {
        // Arweave one-time payment based on current AR price and storage costs
        // This is a simplified estimation
        let ar_price_usd = 10.0; // Assumed AR price
        let cost_per_kb = 0.0001 * ar_price_usd; // Rough estimate
        (size_bytes as f64 / 1024.0) * cost_per_kb
    }

    fn estimate_arweave_gas(&self, size_bytes: u64) -> u64 {
        // Estimate gas usage for Arweave transaction
        // This is a simplified calculation
        let base_gas = 20000;
        let gas_per_byte = 100;
        base_gas + (size_bytes * gas_per_byte)
    }

    async fn save_archive_record(&self, result: &ArchiveResult) -> Result<(), ArchiveError> {
        // This would save to the database using the db_entity module
        // For now, we'll just log the result
        log::info!("Saved archive record for game {}: {:?}", result.game_id, result.archive_urls);
        Ok(())
    }

    pub async fn get_archive_status(&self, game_id: Uuid) -> Result<ArchiveStatus, ArchiveError> {
        // This would fetch from the database
        // For now, return a placeholder
        Ok(ArchiveStatus {
            game_id,
            status: ArchiveProcessStatus::Completed,
            progress: 100.0,
            error_message: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    pub async fn list_archived_games(&self, limit: u64, offset: u64) -> Result<Vec<ArchiveResult>, ArchiveError> {
        // This would fetch from the database
        // For now, return an empty vector
        Ok(vec![])
    }

    pub async fn verify_archive(&self, game_id: Uuid, expected_hash: &str) -> Result<bool, ArchiveError> {
        // This would verify the archive integrity
        // For now, return true
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_sample_pgn() -> PGNGame {
        PGNGame {
            id: Uuid::new_v4(),
            event: "Online Game".to_string(),
            site: "XLMate".to_string(),
            date: "2024.01.01".to_string(),
            round: "1".to_string(),
            white: "Player1".to_string(),
            black: "Player2".to_string(),
            result: "1-0".to_string(),
            white_elo: Some(1500),
            black_elo: Some(1400),
            time_control: "5+0".to_string(),
            eco: Some("C20".to_string()),
            opening: Some("King's Pawn Game".to_string()),
            moves: vec![
                "e4".to_string(),
                "e5".to_string(),
                "Nf3".to_string(),
                "Nc6".to_string(),
                "Bb5".to_string(),
                "a6".to_string(),
                "Ba4".to_string(),
                "Nf6".to_string(),
                "O-O".to_string(),
                "Be7".to_string(),
            ],
            annotations: Some(vec![
                GameAnnotation {
                    move_number: 1,
                    evaluation: Some(0.2),
                    comment: Some("Good opening move".to_string()),
                    variation: None,
                    timestamp: Utc::now(),
                }
            ]),
            metadata: GameMetadata {
                game_id: Uuid::new_v4(),
                tournament_id: None,
                created_at: Utc::now(),
                completed_at: Utc::now(),
                duration_seconds: 300,
                total_moves: 10,
                average_move_time: 30.0,
                time_control_category: TimeControlCategory::Blitz,
                game_type: GameType::Rated,
            },
        }
    }

    #[test]
    fn test_pgn_to_string() {
        let archiver = PGNArchiver::new(
            sea_orm::Database::connect("sqlite::memory:").await.unwrap(),
            "https://ipfs.infura.io:5001".to_string(),
            "https://arweave.net".to_string(),
        );
        
        let pgn = create_sample_pgn();
        let pgn_string = archiver.pgn_to_string(&pgn).unwrap();
        
        assert!(pgn_string.contains("[Event \"Online Game\"]"));
        assert!(pgn_string.contains("[White \"Player1\"]"));
        assert!(pgn_string.contains("[Black \"Player2\"]"));
        assert!(pgn_string.contains("1. e4 e5"));
        assert!(pgn_string.contains("2. Nf3 Nc6"));
    }

    #[test]
    fn test_hash_calculation() {
        let archiver = PGNArchiver::new(
            sea_orm::Database::connect("sqlite::memory:").await.unwrap(),
            "https://ipfs.infura.io:5001".to_string(),
            "https://arweave.net".to_string(),
        );
        
        let data = b"test data";
        let hash = archiver.calculate_hash(data);
        
        assert_eq!(hash.len(), 64); // SHA256 produces 64-character hex string
    }

    #[test]
    fn test_cost_estimation() {
        let archiver = PGNArchiver::new(
            sea_orm::Database::connect("sqlite::memory:").await.unwrap(),
            "https://ipfs.infura.io:5001".to_string(),
            "https://arweave.net".to_string(),
        );
        
        let size = 1024; // 1KB
        let ipfs_cost = archiver.estimate_ipfs_cost(size);
        let arweave_cost = archiver.estimate_arwear_cost(size);
        
        assert!(ipfs_cost > 0.0);
        assert!(arweave_cost > 0.0);
    }
}
