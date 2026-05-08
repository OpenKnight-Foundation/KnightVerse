use actix_web::{web, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use crate::{NFTService, AIMetadata, NFTMintRequest};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MintNFTRequest {
    pub ai_metadata: AIMetadata,
    pub destination_account: String,
    pub network: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MintNFTResponse {
    pub success: bool,
    pub xdr_transaction: Option<String>,
    pub network: Option<String>,
    pub transaction_hash: Option<String>,
    pub error: Option<String>,
}

#[utoipa::path(
    post,
    path = "/api/v1/nft/mint",
    tag = "NFT",
    request_body = MintNFTRequest,
    responses(
        (status = 200, description = "Transaction created successfully", body = MintNFTResponse),
        (status = 400, description = "Invalid request", body = MintNFTResponse),
        (status = 500, description = "Internal server error", body = MintNFTResponse)
    )
)]
pub async fn mint_nft(
    request: web::Json<MintNFTRequest>,
) -> Result<HttpResponse> {
    // Get issuer account from environment or configuration
    let issuer_account = std::env::var("STELLAR_ISSUER_ACCOUNT")
        .unwrap_or_else(|_| "GAB35A2WLFSK64P6EWSGVFXZYU6E5K2INGTTLMDEDSIPYOH7NZVV6GIG".to_string());

    let mint_request = NFTMintRequest {
        ai_metadata: request.ai_metadata.clone(),
        destination_account: request.destination_account.clone(),
        issuer_account,
        network: request.network.clone(),
    };

    match NFTService::create_nft_mint_transaction(mint_request).await {
        Ok(response) => Ok(HttpResponse::Ok().json(MintNFTResponse {
            success: true,
            xdr_transaction: Some(response.xdr_transaction),
            network: Some(response.network),
            transaction_hash: response.transaction_hash,
            error: None,
        })),
        Err(e) => Ok(HttpResponse::BadRequest().json(MintNFTResponse {
            success: false,
            xdr_transaction: None,
            network: None,
            transaction_hash: None,
            error: Some(e.to_string()),
        })),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/nft/metadata/format",
    tag = "NFT",
    request_body = AIMetadata,
    responses(
        (status = 200, description = "Metadata formatted successfully", body = serde_json::Value),
        (status = 400, description = "Invalid metadata", body = serde_json::Value)
    )
)]
pub async fn format_ai_metadata(
    request: web::Json<AIMetadata>,
) -> Result<HttpResponse> {
    match NFTService::format_ai_metadata(request.into_inner()) {
        Ok(formatted_metadata) => {
            match NFTService::create_ipfs_metadata(&formatted_metadata) {
                Ok(ipfs_metadata) => Ok(HttpResponse::Ok().json(ipfs_metadata)),
                Err(e) => Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "success": false,
                    "error": e.to_string()
                }))),
            }
        },
        Err(e) => Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "error": e.to_string()
        }))),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/nft/stellar-toml",
    tag = "NFT",
    params(
        ("code" = String, Query, description = "Asset code"),
        ("issuer" = String, Query, description = "Asset issuer"),
        ("name" = String, Query, description = "NFT name"),
        ("description" = String, Query, description = "NFT description"),
        ("url" = Option<String>, Query, description = "NFT URL"),
        ("image" = Option<String>, Query, description = "NFT image URL")
    ),
    responses(
        (status = 200, description = "stellar.toml generated successfully", body = serde_json::Value),
        (status = 400, description = "Invalid parameters", body = serde_json::Value)
    )
)]
pub async fn generate_stellar_toml(
    query: web::Query<serde_json::Value>,
) -> Result<HttpResponse> {
    let metadata = AIMetadata {
        name: query.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        description: query.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        url: query.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        issuer: query.get("issuer").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        code: query.get("code").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        attributes: None,
        external_url: query.get("external_url").and_then(|v| v.as_str()).map(|s| s.to_string()),
        image: query.get("image").and_then(|v| v.as_str()).map(|s| s.to_string()),
        animation_url: query.get("animation_url").and_then(|v| v.as_str()).map(|s| s.to_string()),
        youtube_url: query.get("youtube_url").and_then(|v| v.as_str()).map(|s| s.to_string()),
    };

    match NFTService::generate_stellar_toml(&metadata) {
        Ok(toml_content) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "stellar_toml": toml_content
        }))),
        Err(e) => Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "error": e.to_string()
        }))),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/nft")
            .service(
                web::resource("/mint")
                    .route(web::post().to(mint_nft))
            )
            .service(
                web::resource("/metadata/format")
                    .route(web::post().to(format_ai_metadata))
            )
            .service(
                web::resource("/stellar-toml")
                    .route(web::get().to(generate_stellar_toml))
            )
    );
}
