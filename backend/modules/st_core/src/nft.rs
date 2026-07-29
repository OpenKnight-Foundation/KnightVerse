use crate::models::{AIMetadata, NFTMintRequest, NFTMintResponse};
use crate::transaction_builder::StellarTransactionBuilder;
use anyhow::{anyhow, Result};
use serde_json::json;
use std::collections::HashMap;

pub struct NFTService;

impl NFTService {
    /// Creates an NFT minting transaction following Stellar SEP-0039 standards
    pub async fn create_nft_mint_transaction(request: NFTMintRequest) -> Result<NFTMintResponse> {
        // Validate request
        Self::validate_mint_request(&request)?;

        // Create the transaction
        let response = StellarTransactionBuilder::create_nft_mint_transaction(&request)?;

        Ok(response)
    }

    /// Validates the NFT mint request according to SEP-0039 standards
    fn validate_mint_request(request: &NFTMintRequest) -> Result<()> {
        // Validate Stellar account format
        if request.issuer_account.len() != 56 || !request.issuer_account.starts_with('G') {
            return Err(anyhow!("Invalid issuer account format"));
        }

        if request.destination_account.len() != 56 || !request.destination_account.starts_with('G')
        {
            return Err(anyhow!("Invalid destination account format"));
        }

        // Validate asset code (1-12 characters, alphanumeric)
        if request.ai_metadata.code.is_empty() || request.ai_metadata.code.len() > 12 {
            return Err(anyhow!("Asset code must be 1-12 characters"));
        }

        // Validate required fields
        if request.ai_metadata.name.is_empty() {
            return Err(anyhow!("AI name is required"));
        }

        if request.ai_metadata.description.is_empty() {
            return Err(anyhow!("AI description is required"));
        }

        if request.ai_metadata.issuer.is_empty() {
            return Err(anyhow!("AI issuer is required"));
        }

        // Validate network
        if request.network != "testnet" && request.network != "public" {
            return Err(anyhow!("Network must be 'testnet' or 'public'"));
        }

        Ok(())
    }

    /// Generates stellar.toml content for the NFT following SEP-0039
    pub fn generate_stellar_toml(metadata: &AIMetadata) -> Result<String> {
        let toml_content = format!(
            r#"[DOCUMENTATION]
ORG_URL="{}"

[[CURRENCIES]]
issuer="{}"
code="{}"
name="{}"
desc="{}"
image="{}"
url="{}"
fixed_number=1
display_decimals=7
"#,
            metadata
                .external_url
                .as_ref()
                .unwrap_or(&"https://example.com".to_string()),
            metadata.issuer,
            metadata.code,
            metadata.name,
            metadata.description,
            metadata.image.as_ref().unwrap_or(&"".to_string()),
            metadata.url
        );

        Ok(toml_content)
    }

    /// Creates IPFS-compatible metadata JSON following EIP-721 style
    pub fn create_ipfs_metadata(metadata: &AIMetadata) -> Result<serde_json::Value> {
        let mut ipfs_metadata = json!({
            "name": metadata.name,
            "description": metadata.description,
            "url": metadata.url,
            "issuer": metadata.issuer,
            "code": metadata.code
        });

        // Add optional fields
        if let Some(image) = &metadata.image {
            ipfs_metadata["image"] = json!(image);
        }

        if let Some(external_url) = &metadata.external_url {
            ipfs_metadata["external_url"] = json!(external_url);
        }

        if let Some(animation_url) = &metadata.animation_url {
            ipfs_metadata["animation_url"] = json!(animation_url);
        }

        if let Some(youtube_url) = &metadata.youtube_url {
            ipfs_metadata["youtube_url"] = json!(youtube_url);
        }

        if let Some(attributes) = &metadata.attributes {
            ipfs_metadata["attributes"] = json!(attributes);
        }

        // Add AI-specific metadata
        let mut ai_attributes = HashMap::new();
        ai_attributes.insert("type".to_string(), json!("AI Agent"));
        ai_attributes.insert("standard".to_string(), json!("SEP-0039"));
        ai_attributes.insert(
            "created_at".to_string(),
            json!(chrono::Utc::now().to_rfc3339()),
        );

        if let Some(ref mut attrs) = ipfs_metadata["attributes"].as_object_mut() {
            for (key, value) in ai_attributes {
                attrs.insert(key, value);
            }
        } else {
            ipfs_metadata["attributes"] = json!(ai_attributes);
        }

        Ok(ipfs_metadata)
    }

    /// Validates and formats AI metadata for NFT minting
    pub fn format_ai_metadata(mut metadata: AIMetadata) -> Result<AIMetadata> {
        // Clean and validate name
        metadata.name = metadata.name.trim().to_string();
        if metadata.name.is_empty() {
            return Err(anyhow!("AI name cannot be empty"));
        }

        // Clean and validate description
        metadata.description = metadata.description.trim().to_string();
        if metadata.description.is_empty() {
            return Err(anyhow!("AI description cannot be empty"));
        }

        // Validate asset code format
        metadata.code = metadata.code.to_uppercase().trim().to_string();
        if metadata.code.is_empty() || metadata.code.len() > 12 {
            return Err(anyhow!("Asset code must be 1-12 characters"));
        }

        // Validate URL format if provided
        if !metadata.url.is_empty()
            && !metadata.url.starts_with("http://")
                && !metadata.url.starts_with("https://")
                && !metadata.url.starts_with("ipfs://")
            {
                return Err(anyhow!("URL must start with http://, https://, or ipfs://"));
            }

        Ok(metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_ai_metadata() {
        let metadata = AIMetadata {
            name: "  Test AI  ".to_string(),
            description: "  Test Description  ".to_string(),
            code: "testai".to_string(),
            url: "ipfs://QmTest123".to_string(),
            issuer: "GAB35A2WLFSK64P6EWSGVFXZYU6E5K2INGTTLMDEDSIPYOH7NZVV6GIG".to_string(),
            attributes: None,
            external_url: None,
            image: None,
            animation_url: None,
            youtube_url: None,
        };

        let result = NFTService::format_ai_metadata(metadata);
        assert!(result.is_ok());

        let formatted = result.unwrap();
        assert_eq!(formatted.name, "Test AI");
        assert_eq!(formatted.description, "Test Description");
        assert_eq!(formatted.code, "TESTAI");
    }

    #[test]
    fn test_create_ipfs_metadata() {
        let metadata = AIMetadata {
            name: "Test AI".to_string(),
            description: "Test Description".to_string(),
            code: "TESTAI".to_string(),
            url: "ipfs://QmTest123".to_string(),
            issuer: "GAB35A2WLFSK64P6EWSGVFXZYU6E5K2INGTTLMDEDSIPYOH7NZVV6GIG".to_string(),
            attributes: None,
            external_url: Some("https://example.com".to_string()),
            image: Some("https://example.com/image.png".to_string()),
            animation_url: None,
            youtube_url: None,
        };

        let result = NFTService::create_ipfs_metadata(&metadata);
        assert!(result.is_ok());

        let ipfs_metadata = result.unwrap();
        assert_eq!(ipfs_metadata["name"], "Test AI");
        assert_eq!(ipfs_metadata["code"], "TESTAI");
        assert!(ipfs_metadata["attributes"].is_object());
    }
}
