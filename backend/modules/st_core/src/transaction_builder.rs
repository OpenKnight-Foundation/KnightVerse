use crate::models::{AIMetadata, NFTMintRequest, NFTMintResponse};
use anyhow::{anyhow, Result};
use std::str::FromStr;
use stellar_base::{
    account::DataValue,
    amount::Amount,
    asset::Asset,
    crypto::PublicKey,
    memo::Memo,
    network::Network,
    operations::Operation,
    transaction::{Transaction, MIN_BASE_FEE},
    xdr::XDRSerialize,
};

pub struct StellarTransactionBuilder;

impl StellarTransactionBuilder {
    pub fn create_nft_mint_transaction(request: &NFTMintRequest) -> Result<NFTMintResponse> {
        // Parse accounts
        let destination_publickey = PublicKey::from_account_id(&request.destination_account)?;

        // Create NFT asset (non-divisible)
        let issuer_publickey = PublicKey::from_account_id(&request.ai_metadata.issuer)?;
        let nft_asset = Asset::new_credit(&request.ai_metadata.code, issuer_publickey)?;

        // Create mint operation with minimum amount (1 stroop = 0.0000001)
        let mint_amount = Amount::from_str("0.0000001")?;
        let mint_operation = Operation::new_payment()
            .with_destination(destination_publickey)
            .with_amount(mint_amount)?
            .with_asset(nft_asset)
            .build()?;

        // Create manage data operation for IPFS hash (if URL is provided)
        let mut operations = Vec::new();

        // Add mint operation
        operations.push(mint_operation);

        if !request.ai_metadata.url.is_empty() {
            // Extract IPFS hash or use full URL as data entry
            let data_entry_name = "ipfshash";
            let data_entry_value = request.ai_metadata.url.clone();

            let manage_data_op = Operation::new_manage_data()
                .with_data_name(data_entry_name.to_string())
                .with_data_value(Some(DataValue::from_slice(data_entry_value.as_bytes())?))
                .build()?;

            operations.push(manage_data_op);
        }

        // Get sequence number (in production, this should be fetched from Horizon)
        let sequence_number = 1u64; // Placeholder - should be fetched from Horizon API

        // Create transaction
        let mut transaction =
            Transaction::builder(issuer_publickey, sequence_number as i64, MIN_BASE_FEE)
                .with_memo(Memo::new_text(format!(
                    "NFT Mint: {}",
                    request.ai_metadata.name
                ))?)
                .add_operation(operations[0].clone());

        // Add manage data operation if present
        if operations.len() > 1 {
            transaction = transaction.add_operation(operations[1].clone());
        }

        let transaction = transaction.into_transaction()?;

        // Set network and sign
        let _network = match request.network.as_str() {
            "testnet" => Network::new_test(),
            "public" => Network::new_public(),
            _ => return Err(anyhow!("Invalid network. Use 'testnet' or 'public'")),
        };

        // Generate XDR (unsigned transaction envelope)
        let xdr_envelope = transaction.into_envelope();
        let xdr_base64 = xdr_envelope.xdr_base64()?;

        // Generate transaction hash for reference
        let transaction_hash = format!("tx_{}", uuid::Uuid::new_v4());

        Ok(NFTMintResponse {
            xdr_transaction: xdr_base64,
            network: request.network.clone(),
            transaction_hash: Some(transaction_hash),
            created_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    pub fn format_ai_metadata_for_stellar(metadata: &AIMetadata) -> Result<serde_json::Value> {
        let mut stellar_metadata = serde_json::json!({
            "name": metadata.name,
            "description": metadata.description,
            "url": metadata.url,
            "issuer": metadata.issuer,
            "code": metadata.code
        });

        // Add optional fields if present
        if let Some(image) = &metadata.image {
            stellar_metadata["image"] = serde_json::Value::String(image.clone());
        }

        if let Some(external_url) = &metadata.external_url {
            stellar_metadata["external_url"] = serde_json::Value::String(external_url.clone());
        }

        if let Some(animation_url) = &metadata.animation_url {
            stellar_metadata["animation_url"] = serde_json::Value::String(animation_url.clone());
        }

        if let Some(youtube_url) = &metadata.youtube_url {
            stellar_metadata["youtube_url"] = serde_json::Value::String(youtube_url.clone());
        }

        if let Some(attributes) = &metadata.attributes {
            stellar_metadata["attributes"] = serde_json::Value::Object(
                attributes
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            );
        }

        Ok(stellar_metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_nft_mint_transaction() {
        let request = NFTMintRequest {
            ai_metadata: AIMetadata {
                name: "Test AI NFT".to_string(),
                description: "A test NFT representing an AI".to_string(),
                url: "ipfs://QmTest123".to_string(),
                issuer: "GAB35A2WLFSK64P6EWSGVFXZYU6E5K2INGTTLMDEDSIPYOH7NZVV6GIG".to_string(),
                code: "TESTAI".to_string(),
                attributes: None,
                external_url: None,
                image: None,
                animation_url: None,
                youtube_url: None,
            },
            destination_account: "GATTMQEODSDX45WZK2JFIYETXWYCU5GRJ5I3Z7P2UDYD6YFVONDM4CX4"
                .to_string(),
            issuer_account: "GAB35A2WLFSK64P6EWSGVFXZYU6E5K2INGTTLMDEDSIPYOH7NZVV6GIG".to_string(),
            network: "testnet".to_string(),
        };

        let result = StellarTransactionBuilder::create_nft_mint_transaction(&request);
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(!response.xdr_transaction.is_empty());
        assert_eq!(response.network, "testnet");
    }
}
