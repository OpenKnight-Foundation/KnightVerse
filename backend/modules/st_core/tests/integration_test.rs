#[cfg(test)]
mod tests {
    use super::*;
    use st_core::{NFTService, AIMetadata, NFTMintRequest};

    #[tokio::test]
    async fn test_nft_mint_transaction_creation() {
        let ai_metadata = AIMetadata {
            name: "Test AI Agent".to_string(),
            description: "A test AI agent for chess".to_string(),
            url: "ipfs://QmTest123456".to_string(),
            issuer: "GAB35A2WLFSK64P6EWSGVFXZYU6E5K2INGTTLMDEDSIPYOH7NZVV6GIG".to_string(),
            code: "TESTAI".to_string(),
            attributes: None,
            external_url: Some("https://example.com/ai/test".to_string()),
            image: Some("https://example.com/images/test-ai.png".to_string()),
            animation_url: None,
            youtube_url: None,
        };

        let mint_request = NFTMintRequest {
            ai_metadata,
            destination_account: "GATTMQEODSDX45WZK2JFIYETXWYCU5GRJ5I3Z7P2UDYD6YFVONDM4CX4".to_string(),
            issuer_account: "GAB35A2WLFSK64P6EWSGVFXZYU6E5K2INGTTLMDEDSIPYOH7NZVV6GIG".to_string(),
            network: "testnet".to_string(),
        };

        let result = NFTService::create_nft_mint_transaction(mint_request).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(!response.xdr_transaction.is_empty());
        assert_eq!(response.network, "testnet");
        assert!(response.transaction_hash.is_some());
    }

    #[tokio::test]
    async fn test_ai_metadata_formatting() {
        let mut metadata = AIMetadata {
            name: "  Test AI  ".to_string(),
            description: "  Test Description  ".to_string(),
            url: "ipfs://QmTest123".to_string(),
            issuer: "GAB35A2WLFSK64P6EWSGVFXZYU6E5K2INGTTLMDEDSIPYOH7NZVV6GIG".to_string(),
            code: "testai".to_string(),
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

    #[tokio::test]
    async fn test_ipfs_metadata_creation() {
        let metadata = AIMetadata {
            name: "Chess AI".to_string(),
            description: "AI chess master".to_string(),
            url: "ipfs://QmChessAI".to_string(),
            issuer: "GAB35A2WLFSK64P6EWSGVFXZYU6E5K2INGTTLMDEDSIPYOH7NZVV6GIG".to_string(),
            code: "CHESSAI".to_string(),
            attributes: None,
            external_url: Some("https://knightverse.ai".to_string()),
            image: Some("https://example.com/chess-ai.png".to_string()),
            animation_url: None,
            youtube_url: None,
        };

        let result = NFTService::create_ipfs_metadata(&metadata);
        assert!(result.is_ok());

        let ipfs_metadata = result.unwrap();
        assert_eq!(ipfs_metadata["name"], "Chess AI");
        assert_eq!(ipfs_metadata["code"], "CHESSAI");
        assert!(ipfs_metadata["attributes"].is_object());
    }

    #[tokio::test]
    async fn test_stellar_toml_generation() {
        let metadata = AIMetadata {
            name: "Test NFT".to_string(),
            description: "Test NFT description".to_string(),
            url: "https://example.com/nft".to_string(),
            issuer: "GAB35A2WLFSK64P6EWSGVFXZYU6E5K2INGTTLMDEDSIPYOH7NZVV6GIG".to_string(),
            code: "TESTNFT".to_string(),
            attributes: None,
            external_url: Some("https://example.com".to_string()),
            image: Some("https://example.com/image.png".to_string()),
            animation_url: None,
            youtube_url: None,
        };

        let result = NFTService::generate_stellar_toml(&metadata);
        assert!(result.is_ok());

        let toml_content = result.unwrap();
        assert!(toml_content.contains("TESTNFT"));
        assert!(toml_content.contains("Test NFT"));
        assert!(toml_content.contains("fixed_number=1"));
    }
}
