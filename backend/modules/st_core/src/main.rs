use st_core::{NFTService, AIMetadata, NFTMintRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Stellar Core NFT Service");
    
    // Example usage
    let ai_metadata = AIMetadata {
        name: "Chess AI Master".to_string(),
        description: "An AI agent that excels at chess strategy and analysis".to_string(),
        url: "ipfs://QmChessAI123".to_string(),
        issuer: "GAB35A2WLFSK64P6EWSGVFXZYU6E5K2INGTTLMDEDSIPYOH7NZVV6GIG".to_string(),
        code: "CHESSAI".to_string(),
        attributes: None,
        external_url: Some("https://knightverse.ai/agents/chess-master".to_string()),
        image: Some("https://example.com/chess-ai.png".to_string()),
        animation_url: None,
        youtube_url: None,
    };
    
    let mint_request = NFTMintRequest {
        ai_metadata,
        destination_account: "GATTMQEODSDX45WZK2JFIYETXWYCU5GRJ5I3Z7P2UDYD6YFVONDM4CX4".to_string(),
        issuer_account: "GAB35A2WLFSK64P6EWSGVFXZYU6E5K2INGTTLMDEDSIPYOH7NZVV6GIG".to_string(),
        network: "testnet".to_string(),
    };
    
    match NFTService::create_nft_mint_transaction(mint_request).await {
        Ok(response) => {
            println!("✅ NFT Mint Transaction Created Successfully!");
            println!("Network: {}", response.network);
            println!("XDR Transaction: {}", response.xdr_transaction);
            println!("Transaction Hash: {:?}", response.transaction_hash);
            println!("Created At: {}", response.created_at);
        }
        Err(e) => {
            eprintln!("❌ Error creating NFT mint transaction: {}", e);
        }
    }
    
    Ok(())
}
