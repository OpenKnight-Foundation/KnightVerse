use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AIMetadata {
    pub name: String,
    pub description: String,
    pub url: String,
    pub issuer: String,
    pub code: String,
    pub attributes: Option<HashMap<String, serde_json::Value>>,
    pub external_url: Option<String>,
    pub image: Option<String>,
    pub animation_url: Option<String>,
    pub youtube_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NFTMintRequest {
    pub ai_metadata: AIMetadata,
    pub destination_account: String,
    pub issuer_account: String,
    pub network: String, // "testnet" or "public"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NFTMintResponse {
    pub xdr_transaction: String,
    pub network: String,
    pub transaction_hash: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StellarAssetInfo {
    pub code: String,
    pub issuer: String,
    pub name: String,
    pub desc: String,
    pub image: Option<String>,
    pub url: Option<String>,
    pub url_sha256: Option<String>,
    pub fixed_number: u32,
    pub display_decimals: u8,
}
