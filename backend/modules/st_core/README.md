# Stellar Core (st_core)

A Rust module for creating NFT minting transactions on the Stellar network following SEP-0039 standards.

## Features

- **AI Metadata Formatting**: Format AI agent metadata according to Stellar SEP-0039 and EIP-721 standards
- **XDR Transaction Construction**: Build signable XDR transaction envelopes for NFT minting
- **IPFS Integration**: Support for IPFS hash storage in Stellar manage data operations
- **SEP-0039 Compliance**: Full compliance with Stellar NFT interoperability standards
- **API Integration**: Ready-to-use HTTP endpoints for integration with existing APIs

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
st_core = { path = "../st_core" }
```

For API integration:
```toml
st_core = { path = "../st_core", features = ["api"] }
```

## Usage

### Creating an NFT Minting Transaction

```rust
use st_core::{NFTService, AIMetadata, NFTMintRequest};

let ai_metadata = AIMetadata {
    name: "Chess AI Master".to_string(),
    description: "An AI agent that excels at chess strategy".to_string(),
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

let response = NFTService::create_nft_mint_transaction(mint_request).await?;
println!("XDR Transaction: {}", response.xdr_transaction);
```

### Formatting AI Metadata

```rust
let formatted_metadata = NFTService::format_ai_metadata(metadata)?;
let ipfs_metadata = NFTService::create_ipfs_metadata(&formatted_metadata)?;
```

### Generating stellar.toml

```rust
let stellar_toml = NFTService::generate_stellar_toml(&metadata)?;
```

## API Endpoints

When the `api` feature is enabled, the following endpoints are available:

### POST /api/v1/nft/mint
Creates a signable XDR transaction for NFT minting.

**Request Body:**
```json
{
  "ai_metadata": {
    "name": "Chess AI Master",
    "description": "An AI agent that excels at chess strategy",
    "url": "ipfs://QmChessAI123",
    "issuer": "GAB35A2WLFSK64P6EWSGVFXZYU6E5K2INGTTLMDEDSIPYOH7NZVV6GIG",
    "code": "CHESSAI",
    "external_url": "https://knightverse.ai/agents/chess-master",
    "image": "https://example.com/chess-ai.png"
  },
  "destination_account": "GATTMQEODSDX45WZK2JFIYETXWYCU5GRJ5I3Z7P2UDYD6YFVONDM4CX4",
  "network": "testnet"
}
```

**Response:**
```json
{
  "success": true,
  "xdr_transaction": "AAAAAgAAAAA...",
  "network": "testnet",
  "transaction_hash": "tx_1234567890"
}
```

### POST /api/v1/nft/metadata/format
Formats AI metadata according to SEP-0039 standards.

### GET /api/v1/nft/stellar-toml
Generates stellar.toml content for the NFT.

## Stellar SEP-0039 Compliance

This implementation follows the [Stellar Ecosystem Proposal 0039](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0039.md) for NFT interoperability:

- **Non-divisible Assets**: NFTs are issued with minimum precision (1 stroop = 0.0000001)
- **IPFS Hash Storage**: Uses `ipfshash` manage data entry for IPFS content identifiers
- **Metadata Standards**: Follows EIP-721 style JSON metadata format
- **stellar.toml Integration**: Generates proper stellar.toml files for ecosystem compatibility

## Environment Variables

- `STELLAR_ISSUER_ACCOUNT`: Default issuer account for NFTs (optional)
- `STELLAR_NETWORK`: Default network ("testnet" or "public")

## Testing

Run tests with:
```bash
cargo test -p st_core
```

## Dependencies

- `stellar-base`: Core Stellar SDK for transaction building
- `stellar_sdk`: Horizon API client (optional)
- `serde`: JSON serialization/deserialization
- `anyhow`: Error handling
- `chrono`: Date/time handling
- `uuid`: Unique identifier generation

## License

MIT
