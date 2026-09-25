use crate::models::{AIMetadata, NFTMintRequest, NFTMintResponse};
use anyhow::{anyhow, Result};
use std::str::FromStr;
use std::time::Duration;
use stellar_base::{
    account::DataValue,
    amount::Amount,
    asset::Asset,
    crypto::PublicKey,
    memo::Memo,
    operations::Operation,
    transaction::{Transaction, MIN_BASE_FEE},
    xdr::XDRSerialize,
};
use thiserror::Error;

/// Public Horizon endpoints, used unless `HORIZON_URL` overrides them.
const TESTNET_HORIZON_URL: &str = "https://horizon-testnet.stellar.org";
const PUBLIC_HORIZON_URL: &str = "https://horizon.stellar.org";

/// How long to wait for Horizon to report the signing account.
const HORIZON_TIMEOUT: Duration = Duration::from_secs(10);

/// Everything that can go wrong while resolving the sequence number of the
/// account that signs the mint transaction.
#[derive(Debug, Error)]
pub enum SequenceNumberError {
    #[error("could not build the Horizon HTTP client: {0}")]
    ClientBuild(#[source] reqwest::Error),

    #[error("Horizon request for account {account_id} failed: {source}")]
    Request {
        account_id: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("Horizon answered {status} for account {account_id}")]
    Status { account_id: String, status: u16 },

    #[error("Horizon sent an unusable account payload for {account_id}: {reason}")]
    Payload { account_id: String, reason: String },

    #[error("account {account_id} has no sequence number left to spend")]
    Exhausted { account_id: String },
}

/// Resolves the Horizon base URL to use for a network.
///
/// `HORIZON_URL` takes precedence when it holds a non-empty value, which lets a
/// deployment point the backend at a private Horizon. The network is validated
/// first so an unknown network is always rejected.
pub fn horizon_url_for_network(network: &str) -> Result<String> {
    let default = match network {
        "testnet" => TESTNET_HORIZON_URL,
        "public" => PUBLIC_HORIZON_URL,
        _ => return Err(anyhow!("Invalid network. Use 'testnet' or 'public'")),
    };

    Ok(std::env::var("HORIZON_URL")
        .ok()
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| default.to_string()))
}

/// Builds the Horizon account endpoint URL for `account_id`.
fn account_url(horizon_url: &str, account_id: &str) -> String {
    format!(
        "{}/accounts/{}",
        horizon_url.trim_end_matches('/'),
        account_id
    )
}

/// Reads a Horizon account payload and returns the next sequence number that may
/// be used by a transaction from that account.
///
/// Stellar rejects a transaction whose sequence number is not exactly
/// `account.sequence + 1` (`tx_bad_seq`), so the increment happens here rather
/// than at every call site. Sequence numbers are signed 64-bit (an account starts
/// at `creation_ledger << 32`), and Horizon encodes them as strings to keep them
/// intact in JavaScript clients, so both shapes are accepted.
pub fn next_sequence_number(
    account_id: &str,
    payload: &serde_json::Value,
) -> Result<i64, SequenceNumberError> {
    let unusable = |reason: String| SequenceNumberError::Payload {
        account_id: account_id.to_string(),
        reason,
    };

    let sequence = payload
        .get("sequence")
        .ok_or_else(|| unusable("no `sequence` field".to_string()))?;

    let current: i64 = match sequence {
        serde_json::Value::String(value) => value
            .parse::<i64>()
            .map_err(|_| unusable(format!("`sequence` is not a 64-bit integer: {value}")))?,
        serde_json::Value::Number(value) => value
            .as_i64()
            .ok_or_else(|| unusable(format!("`sequence` is not a 64-bit integer: {value}")))?,
        _ => {
            return Err(unusable(format!(
                "`sequence` is not a string or a number: {sequence}"
            )))
        }
    };

    if current < 0 {
        return Err(unusable(format!("`sequence` is negative: {current}")));
    }

    current.checked_add(1).ok_or(SequenceNumberError::Exhausted {
            account_id: account_id.to_string(),
        })
}

/// Loads the next usable sequence number for `account_id` from a Horizon server.
///
/// Every failure is reported as a typed [`SequenceNumberError`]; there is no
/// fallback sequence number, because guessing one produces a transaction the
/// network will reject with `tx_bad_seq`.
pub async fn fetch_next_sequence_number(
    horizon_url: &str,
    account_id: &str,
) -> Result<i64, SequenceNumberError> {
    let url = account_url(horizon_url, account_id);

    let client = reqwest::Client::builder()
        .timeout(HORIZON_TIMEOUT)
        .build()
        .map_err(SequenceNumberError::ClientBuild)?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|source| SequenceNumberError::Request {
            account_id: account_id.to_string(),
            source,
        })?;

    let status = response.status();
    if !status.is_success() {
        return Err(SequenceNumberError::Status {
            account_id: account_id.to_string(),
            status: status.as_u16(),
        });
    }

    let payload: serde_json::Value = response.json().await.map_err(|source| {
        SequenceNumberError::Payload {
            account_id: account_id.to_string(),
            reason: format!("response body is not valid JSON: {source}"),
        }
    })?;

    next_sequence_number(account_id, &payload)
}

pub struct StellarTransactionBuilder;

impl StellarTransactionBuilder {
    /// Creates an unsigned SEP-0039 NFT mint transaction.
    ///
    /// The sequence number is loaded from Horizon for the account that signs the
    /// transaction, because Stellar rejects any transaction whose sequence number
    /// is not exactly `source_account.sequence + 1` (`tx_bad_seq`). If the lookup
    /// fails the call returns an error rather than falling back to a value the
    /// network would reject.
    pub async fn create_nft_mint_transaction(request: &NFTMintRequest) -> Result<NFTMintResponse> {
        let horizon_url = horizon_url_for_network(&request.network)?;

        Self::create_nft_mint_transaction_on(&horizon_url, request).await
    }

    /// Creates an unsigned NFT mint transaction against an explicit Horizon
    /// server, resolving the signing account's sequence number from it.
    pub async fn create_nft_mint_transaction_on(
        horizon_url: &str,
        request: &NFTMintRequest,
    ) -> Result<NFTMintResponse> {
        // The envelope is built with `ai_metadata.issuer` as the source account,
        // so that is the account whose sequence number has to be spent.
        let sequence_number =
            fetch_next_sequence_number(horizon_url, &request.ai_metadata.issuer).await?;

        Self::build_nft_mint_transaction_with_sequence(request, sequence_number)
    }

    /// Creates an unsigned NFT mint transaction around an already resolved
    /// sequence number.
    pub fn build_nft_mint_transaction_with_sequence(
        request: &NFTMintRequest,
        sequence_number: i64,
    ) -> Result<NFTMintResponse> {
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
        let mut operations = vec![mint_operation];

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

        // Create transaction
        let mut transaction =
            Transaction::builder(issuer_publickey, sequence_number, MIN_BASE_FEE)
                .with_memo(Memo::new_text(format!(
                    "NFT Mint: {}",
                    request.ai_metadata.name
                ))?);

        for operation in operations {
            transaction = transaction.add_operation(operation);
        }

        let transaction = transaction.into_transaction()?;

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
    use stellar_base::xdr::XDRDeserialize;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    const ISSUER: &str = "GAB35A2WLFSK64P6EWSGVFXZYU6E5K2INGTTLMDEDSIPYOH7NZVV6GIG";
    const DESTINATION: &str = "GATTMQEODSDX45WZK2JFIYETXWYCU5GRJ5I3Z7P2UDYD6YFVONDM4CX4";

    fn sample_request() -> NFTMintRequest {
        NFTMintRequest {
            ai_metadata: AIMetadata {
                name: "Test AI NFT".to_string(),
                description: "A test NFT representing an AI".to_string(),
                url: "ipfs://QmTest123".to_string(),
                issuer: ISSUER.to_string(),
                code: "TESTAI".to_string(),
                attributes: None,
                external_url: None,
                image: None,
                animation_url: None,
                youtube_url: None,
            },
            destination_account: DESTINATION.to_string(),
            issuer_account: ISSUER.to_string(),
            network: "testnet".to_string(),
        }
    }

    /// Decodes the envelope the builder returns and reads back the sequence
    /// number that will actually be submitted to the network.
    fn sequence_in_xdr(xdr_transaction: &str) -> i64 {
        let envelope = stellar_base::xdr::TransactionEnvelope::from_xdr_base64(xdr_transaction)
            .expect("builder returns valid base64 XDR");
        let envelope = stellar_base::transaction::TransactionEnvelope::from_xdr(&envelope)
            .expect("envelope decodes into a v1 transaction");

        *envelope
            .as_transaction()
            .expect("envelope holds a transaction")
            .sequence()
    }

    /// Serves a single canned Horizon account response on an ephemeral local port.
    async fn spawn_horizon_stub(status_line: &'static str, body: &'static str) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind stub Horizon listener");
        let address = listener.local_addr().expect("read stub Horizon address");

        tokio::spawn(async move {
            let (mut socket, _) = match listener.accept().await {
                Ok(connection) => connection,
                Err(_) => return,
            };

            // Only an account is ever requested, so the request carries no body:
            // reading up to the end of the headers is enough.
            let mut head = Vec::new();
            let mut chunk = [0u8; 1024];
            while !head.windows(4).any(|window| window == b"\r\n\r\n") {
                match socket.read(&mut chunk).await {
                    Ok(0) => break,
                    Ok(read) => head.extend_from_slice(&chunk[..read]),
                    Err(_) => return,
                }
            }

            let response = format!(
                "HTTP/1.1 {status_line}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.flush().await;
        });

        format!("http://{address}")
    }

    #[test]
    fn test_account_url_targets_the_account_endpoint() {
        assert_eq!(
            account_url(TESTNET_HORIZON_URL, ISSUER),
            format!("{TESTNET_HORIZON_URL}/accounts/{ISSUER}")
        );
        assert_eq!(
            account_url("http://127.0.0.1:8080/", ISSUER),
            format!("http://127.0.0.1:8080/accounts/{ISSUER}")
        );
    }

    #[test]
    fn test_mint_transaction_uses_the_resolved_sequence_number() {
        let response =
            StellarTransactionBuilder::build_nft_mint_transaction_with_sequence(&sample_request(), 43)
                .expect("build mint transaction");

        assert_eq!(sequence_in_xdr(&response.xdr_transaction), 43);
        assert_eq!(response.network, "testnet");
        assert!(!response.xdr_transaction.is_empty());
    }

    #[tokio::test]
    async fn test_sequence_number_is_fetched_from_horizon_and_incremented() {
        let base_url = spawn_horizon_stub(
            "200 OK",
            r#"{"account_id":"GAB35A2WLFSK64P6EWSGVFXZYU6E5K2INGTTLMDEDSIPYOH7NZVV6GIG","sequence":"41","subentry_count":0}"#,
        )
        .await;

        let next_sequence = fetch_next_sequence_number(&base_url, ISSUER)
            .await
            .expect("Horizon account is reachable");

        assert_eq!(next_sequence, 42, "sequence must be fetched_sequence + 1");

        let response = StellarTransactionBuilder::build_nft_mint_transaction_with_sequence(
            &sample_request(),
            next_sequence,
        )
        .expect("build mint transaction");

        assert_eq!(
            sequence_in_xdr(&response.xdr_transaction),
            42,
            "the signed envelope must carry the fetched sequence plus one"
        );
    }

    #[tokio::test]
    async fn test_mint_transaction_carries_the_horizon_sequence_number() {
        let base_url = spawn_horizon_stub(
            "200 OK",
            r#"{"account_id":"GAB35A2WLFSK64P6EWSGVFXZYU6E5K2INGTTLMDEDSIPYOH7NZVV6GIG","sequence":"41","subentry_count":0}"#,
        )
        .await;

        let response =
            StellarTransactionBuilder::create_nft_mint_transaction_on(&base_url, &sample_request())
                .await
                .expect("mint transaction is built from the fetched sequence");

        assert_eq!(
            sequence_in_xdr(&response.xdr_transaction),
            42,
            "the envelope must not reuse a hardcoded sequence number"
        );
        assert_eq!(response.network, "testnet");
    }

    #[tokio::test]
    async fn test_horizon_error_status_becomes_a_typed_error() {
        let base_url = spawn_horizon_stub("404 Not Found", "{}").await;

        let error = fetch_next_sequence_number(&base_url, ISSUER)
            .await
            .expect_err("a missing Horizon account must not be papered over");

        assert!(
            matches!(
                &error,
                SequenceNumberError::Status { status, .. } if *status == 404
            ),
            "expected a typed status error, got {error:?}"
        );
    }

    #[test]
    fn test_unusable_sequence_payloads_are_rejected() {
        let missing = serde_json::json!({ "account_id": ISSUER });
        assert!(matches!(
            next_sequence_number(ISSUER, &missing),
            Err(SequenceNumberError::Payload { .. })
        ));

        let not_a_number = serde_json::json!({ "sequence": "not-a-number" });
        assert!(matches!(
            next_sequence_number(ISSUER, &not_a_number),
            Err(SequenceNumberError::Payload { .. })
        ));

        let wrong_shape = serde_json::json!({ "sequence": { "value": 41 } });
        assert!(matches!(
            next_sequence_number(ISSUER, &wrong_shape),
            Err(SequenceNumberError::Payload { .. })
        ));

        let too_large = serde_json::json!({ "sequence": "9223372036854775808" });
        assert!(matches!(
            next_sequence_number(ISSUER, &too_large),
            Err(SequenceNumberError::Payload { .. })
        ));

        let negative = serde_json::json!({ "sequence": "-1" });
        assert!(matches!(
            next_sequence_number(ISSUER, &negative),
            Err(SequenceNumberError::Payload { .. })
        ));

        let exhausted = serde_json::json!({ "sequence": "9223372036854775807" });
        assert!(matches!(
            next_sequence_number(ISSUER, &exhausted),
            Err(SequenceNumberError::Exhausted { .. })
        ));
    }

    #[test]
    fn test_numeric_sequence_payloads_are_accepted() {
        let payload = serde_json::json!({ "sequence": 41 });
        assert_eq!(
            next_sequence_number(ISSUER, &payload).expect("numeric sequence is valid"),
            42
        );
    }

    #[test]
    fn test_real_account_sequence_numbers_are_accepted() {
        // A real account starts at `creation_ledger << 32`, far beyond 32 bits.
        let payload = serde_json::json!({ "sequence": "103420918407103888" });
        assert_eq!(
            next_sequence_number(ISSUER, &payload).expect("64-bit sequence is valid"),
            103_420_918_407_103_889
        );
    }

    #[test]
    fn test_unknown_network_is_rejected() {
        let error = horizon_url_for_network("mainnet").expect_err("unknown network must fail");
        assert!(
            error.to_string().contains("Invalid network"),
            "got {error}"
        );
    }
}
