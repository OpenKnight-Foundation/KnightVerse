//! Wallet linking (FE-97): proves the signed-in player controls a Stellar
//! account and stores it on their player record.
//!
//! Flow:
//! 1. `POST /v1/players/me/wallet/challenge {wallet_address}` returns a
//!    single-use message bound to the player, the address and a random nonce
//!    (kept in Redis for `CHALLENGE_TTL_SECS`).
//! 2. The browser asks Freighter to sign that message (`signMessage`).
//! 3. `POST /v1/players/me/wallet/verify {wallet_address, signature}` checks
//!    the ed25519 signature against the public key encoded in the address and
//!    links the wallet.
//!
//! Signatures are checked against the SEP-53 message digest
//! (`SHA-256("Stellar Signed Message:\n" || message)`), which is what current
//! Freighter versions sign. The plain `SHA-256(message)` digest produced by
//! older Freighter releases is also accepted; both are bound to the nonce, so
//! a signature can't be replayed for another player or challenge.

use actix_web::{delete, post, web, HttpMessage, HttpRequest, HttpResponse};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use db::DbPool;
use db_entity::player;
use dto::players::{
    WalletChallengeRequest, WalletChallengeResponse, WalletLinkResponse, WalletVerifyRequest,
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use rand::RngCore;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use security::jwt::Claims;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// How long a challenge can be signed before it expires.
pub const CHALLENGE_TTL_SECS: u64 = 300;

const SEP53_PREFIX: &[u8] = b"Stellar Signed Message:\n";
const ACCOUNT_ID_VERSION_BYTE: u8 = 6 << 3; // 'G…' addresses
const BASE32_ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

#[derive(Serialize, Deserialize)]
struct StoredChallenge {
    wallet_address: String,
    message: String,
}

fn challenge_key(player_id: Uuid) -> String {
    format!("wallet_link:challenge:{}", player_id)
}

fn crc16_xmodem(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

/// Decodes a Stellar account id (`G…` strkey) into its ed25519 public key,
/// validating the version byte and CRC16 checksum.
pub fn decode_account_id(address: &str) -> Option<[u8; 32]> {
    if address.len() != 56 {
        return None;
    }
    let mut bits: u32 = 0;
    let mut bit_count = 0;
    let mut bytes = Vec::with_capacity(35);
    for ch in address.bytes() {
        let value = BASE32_ALPHABET.iter().position(|&c| c == ch)? as u32;
        bits = (bits << 5) | value;
        bit_count += 5;
        if bit_count >= 8 {
            bit_count -= 8;
            bytes.push((bits >> bit_count) as u8);
            bits &= (1 << bit_count) - 1;
        }
    }
    if bytes.len() != 35 || bytes[0] != ACCOUNT_ID_VERSION_BYTE {
        return None;
    }
    let checksum = u16::from_le_bytes([bytes[33], bytes[34]]);
    if crc16_xmodem(&bytes[..33]) != checksum {
        return None;
    }
    bytes[1..33].try_into().ok()
}

/// Decodes a signature sent as base64 (what Freighter returns) or hex.
fn decode_signature(signature: &str) -> Option<Signature> {
    let raw = BASE64
        .decode(signature.trim())
        .ok()
        .filter(|b| b.len() == 64)
        .or_else(|| hex::decode(signature.trim()).ok())?;
    Signature::from_slice(&raw).ok()
}

/// True when `signature` is a valid signature of `message` by `address`.
pub fn verify_wallet_signature(address: &str, message: &str, signature: &str) -> bool {
    let Some(public_key) = decode_account_id(address) else {
        return false;
    };
    let Ok(key) = VerifyingKey::from_bytes(&public_key) else {
        return false;
    };
    let Some(signature) = decode_signature(signature) else {
        return false;
    };

    let sep53_digest = Sha256::new()
        .chain_update(SEP53_PREFIX)
        .chain_update(message)
        .finalize();
    let legacy_digest = Sha256::digest(message.as_bytes());
    key.verify(sep53_digest.as_slice(), &signature).is_ok()
        || key.verify(legacy_digest.as_slice(), &signature).is_ok()
}

fn build_challenge_message(player_id: Uuid, address: &str, nonce: &str) -> String {
    format!(
        "KnightVerse wants to link this Stellar account to your KnightVerse player.\n\n\
         Account: {address}\nPlayer: {player_id}\nNonce: {nonce}\nIssued At: {}",
        chrono::Utc::now().to_rfc3339()
    )
}

fn authenticated_player(req: &HttpRequest) -> Result<Uuid, HttpResponse> {
    req.extensions()
        .get::<Claims>()
        .map(|c| c.player_id)
        .ok_or_else(|| {
            HttpResponse::Unauthorized().json(json!({ "message": "Authentication required" }))
        })
}

/// POST /v1/players/me/wallet/challenge
#[post("/me/wallet/challenge")]
pub async fn wallet_challenge(
    req: HttpRequest,
    payload: web::Json<WalletChallengeRequest>,
    redis: web::Data<deadpool_redis::Pool>,
) -> HttpResponse {
    let player_id = match authenticated_player(&req) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let address = payload.wallet_address.trim().to_string();
    if decode_account_id(&address).is_none() {
        return HttpResponse::BadRequest()
            .json(json!({ "message": "Invalid Stellar account address" }));
    }

    let mut nonce = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut nonce);
    let message = build_challenge_message(player_id, &address, &hex::encode(nonce));
    let stored = StoredChallenge {
        wallet_address: address,
        message: message.clone(),
    };

    let mut conn = match redis.get().await {
        Ok(c) => c,
        Err(_) => {
            return HttpResponse::ServiceUnavailable()
                .json(json!({ "message": "Challenge store unavailable" }))
        }
    };
    let stored_json = serde_json::to_string(&stored).expect("challenge serialises");
    let result: redis::RedisResult<()> = redis::cmd("SET")
        .arg(challenge_key(player_id))
        .arg(stored_json)
        .arg("EX")
        .arg(CHALLENGE_TTL_SECS)
        .query_async(&mut conn)
        .await;
    if result.is_err() {
        return HttpResponse::ServiceUnavailable()
            .json(json!({ "message": "Challenge store unavailable" }));
    }

    HttpResponse::Ok().json(WalletChallengeResponse {
        message,
        expires_in: CHALLENGE_TTL_SECS,
    })
}

/// POST /v1/players/me/wallet/verify
#[post("/me/wallet/verify")]
pub async fn wallet_verify(
    req: HttpRequest,
    payload: web::Json<WalletVerifyRequest>,
    redis: web::Data<deadpool_redis::Pool>,
    pool: web::Data<DbPool>,
) -> HttpResponse {
    let player_id = match authenticated_player(&req) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let address = payload.wallet_address.trim();

    // GETDEL makes each challenge single-use, even if verification fails.
    let mut conn = match redis.get().await {
        Ok(c) => c,
        Err(_) => {
            return HttpResponse::ServiceUnavailable()
                .json(json!({ "message": "Challenge store unavailable" }))
        }
    };
    let stored: Option<String> = match redis::cmd("GETDEL")
        .arg(challenge_key(player_id))
        .query_async(&mut conn)
        .await
    {
        Ok(v) => v,
        Err(_) => {
            return HttpResponse::ServiceUnavailable()
                .json(json!({ "message": "Challenge store unavailable" }))
        }
    };
    let Some(stored) = stored.and_then(|s| serde_json::from_str::<StoredChallenge>(&s).ok()) else {
        return HttpResponse::BadRequest()
            .json(json!({ "message": "No pending challenge; request a new one" }));
    };
    if stored.wallet_address != address {
        return HttpResponse::BadRequest()
            .json(json!({ "message": "Address does not match the challenge" }));
    }
    if !verify_wallet_signature(address, &stored.message, &payload.signature) {
        return HttpResponse::Unauthorized()
            .json(json!({ "message": "Signature verification failed" }));
    }

    // One wallet per player; refuse addresses already linked to someone else.
    match player::Entity::find()
        .filter(player::Column::WalletAddress.eq(address))
        .filter(player::Column::Id.ne(player_id))
        .one(pool.primary())
        .await
    {
        Ok(Some(_)) => {
            return HttpResponse::Conflict()
                .json(json!({ "message": "This wallet is linked to another player" }))
        }
        Ok(None) => {}
        Err(_) => {
            return HttpResponse::InternalServerError()
                .json(json!({ "message": "Failed to link wallet" }))
        }
    }

    let model = match player::Entity::find_by_id(player_id)
        .one(pool.primary())
        .await
    {
        Ok(Some(m)) => m,
        Ok(None) => return HttpResponse::NotFound().json(json!({ "message": "Player not found" })),
        Err(_) => {
            return HttpResponse::InternalServerError()
                .json(json!({ "message": "Failed to link wallet" }))
        }
    };
    let mut active: player::ActiveModel = model.into();
    active.wallet_address = Set(Some(address.to_string()));
    if active.update(pool.primary()).await.is_err() {
        return HttpResponse::InternalServerError()
            .json(json!({ "message": "Failed to link wallet" }));
    }

    HttpResponse::Ok().json(WalletLinkResponse {
        wallet_address: Some(address.to_string()),
    })
}

/// DELETE /v1/players/me/wallet
#[delete("/me/wallet")]
pub async fn wallet_unlink(req: HttpRequest, pool: web::Data<DbPool>) -> HttpResponse {
    let player_id = match authenticated_player(&req) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let model = match player::Entity::find_by_id(player_id)
        .one(pool.primary())
        .await
    {
        Ok(Some(m)) => m,
        Ok(None) => return HttpResponse::NotFound().json(json!({ "message": "Player not found" })),
        Err(_) => {
            return HttpResponse::InternalServerError()
                .json(json!({ "message": "Failed to unlink wallet" }))
        }
    };
    let mut active: player::ActiveModel = model.into();
    active.wallet_address = Set(None);
    if active.update(pool.primary()).await.is_err() {
        return HttpResponse::InternalServerError()
            .json(json!({ "message": "Failed to unlink wallet" }));
    }
    HttpResponse::Ok().json(WalletLinkResponse {
        wallet_address: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn encode_account_id(public_key: &[u8; 32]) -> String {
        let mut payload = vec![ACCOUNT_ID_VERSION_BYTE];
        payload.extend_from_slice(public_key);
        let crc = crc16_xmodem(&payload);
        payload.extend_from_slice(&crc.to_le_bytes());
        let (mut bits, mut bit_count, mut out) = (0u32, 0, String::new());
        for byte in payload {
            bits = (bits << 8) | byte as u32;
            bit_count += 8;
            while bit_count >= 5 {
                bit_count -= 5;
                out.push(BASE32_ALPHABET[((bits >> bit_count) & 31) as usize] as char);
            }
        }
        if bit_count > 0 {
            out.push(BASE32_ALPHABET[((bits << (5 - bit_count)) & 31) as usize] as char);
        }
        out
    }

    fn keypair() -> (SigningKey, String) {
        let signing = SigningKey::from_bytes(&[7u8; 32]);
        let address = encode_account_id(signing.verifying_key().as_bytes());
        (signing, address)
    }

    #[test]
    fn decodes_a_known_account_id() {
        // Circle's mainnet USDC issuer.
        assert!(
            decode_account_id("GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN").is_some()
        );
    }

    #[test]
    fn rejects_bad_checksum_length_and_version() {
        assert!(
            decode_account_id("GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVM").is_none()
        );
        assert!(decode_account_id("GA5ZSEJY").is_none());
        // Secret seeds (S…) use a different version byte.
        assert!(
            decode_account_id("SA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN").is_none()
        );
    }

    #[test]
    fn accepts_a_sep53_signature() {
        let (signing, address) = keypair();
        let message = "KnightVerse challenge nonce 42";
        let digest = Sha256::new()
            .chain_update(SEP53_PREFIX)
            .chain_update(message)
            .finalize();
        let signature = BASE64.encode(signing.sign(digest.as_slice()).to_bytes());

        assert!(verify_wallet_signature(&address, message, &signature));
    }

    #[test]
    fn accepts_a_legacy_freighter_signature_in_hex() {
        let (signing, address) = keypair();
        let message = "KnightVerse challenge nonce 43";
        let signature = hex::encode(
            signing
                .sign(Sha256::digest(message.as_bytes()).as_slice())
                .to_bytes(),
        );

        assert!(verify_wallet_signature(&address, message, &signature));
    }

    #[test]
    fn rejects_a_signature_for_another_message_or_key() {
        let (signing, address) = keypair();
        let digest = Sha256::new()
            .chain_update(SEP53_PREFIX)
            .chain_update("original")
            .finalize();
        let signature = BASE64.encode(signing.sign(digest.as_slice()).to_bytes());
        assert!(!verify_wallet_signature(&address, "tampered", &signature));

        let other = SigningKey::from_bytes(&[9u8; 32]);
        let other_address = encode_account_id(other.verifying_key().as_bytes());
        assert!(!verify_wallet_signature(
            &other_address,
            "original",
            &signature
        ));
        assert!(!verify_wallet_signature(
            &address,
            "original",
            "not-a-signature"
        ));
    }
}
