//! BE-153: signs the payloads the on-chain `game_contract` expects for
//! `claim_win`, `claim_draw` and `claim_puzzle_reward`. The signing key is a
//! secret loaded from the environment; it is never committed to the repo.
//!
//! Callers must only construct [`SignedResult`] after the server has verified
//! the outcome through `GameService` (BE-146/BE-147) — this module deliberately
//! has no public constructor from raw client input.

use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Environment variable holding the base64-encoded 32-byte Ed25519 seed.
pub const RESULT_SIGNING_KEY_ENV: &str = "GAME_RESULT_SIGNING_KEY";

#[derive(Debug, Error)]
pub enum ResultSignerError {
    #[error("game result signing key is not configured")]
    MissingKey,
    #[error("invalid game result signing key: {0}")]
    InvalidKey(String),
}

/// A result the server has already verified and is willing to sign.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignedResult<'a> {
    Win {
        game_id: u64,
        winner_address: &'a str,
    },
    Draw {
        game_id: u64,
    },
    PuzzleReward {
        recipient: &'a str,
        amount: i64,
        nonce: u64,
    },
}

/// Build the exact byte payload the contract hashes before verifying.
fn win_payload(game_id: u64, winner_address: &str) -> Vec<u8> {
    let mut payload = game_id.to_le_bytes().to_vec();
    payload.extend_from_slice(winner_address.as_bytes());
    payload
}

fn draw_payload(game_id: u64) -> Vec<u8> {
    let mut payload = game_id.to_le_bytes().to_vec();
    payload.extend_from_slice(b"DRAW");
    payload
}

fn puzzle_reward_payload(recipient: &str, amount: i64, nonce: u64) -> Vec<u8> {
    let mut payload = recipient.as_bytes().to_vec();
    payload.extend_from_slice(&amount.to_le_bytes());
    payload.extend_from_slice(&nonce.to_le_bytes());
    payload
}

pub struct ResultSigner {
    signing_key: SigningKey,
}

impl ResultSigner {
    /// Load the signing key from `GAME_RESULT_SIGNING_KEY` (base64 of the
    /// 32-byte Ed25519 signing seed).
    pub fn from_env() -> Result<Self, ResultSignerError> {
        let encoded = std::env::var(RESULT_SIGNING_KEY_ENV)
            .map_err(|_| ResultSignerError::MissingKey)?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .map_err(|e| ResultSignerError::InvalidKey(e.to_string()))?;
        let seed: [u8; 32] = bytes
            .try_into()
            .map_err(|_| ResultSignerError::InvalidKey("expected 32 bytes".to_string()))?;
        Ok(Self {
            signing_key: SigningKey::from_bytes(&seed),
        })
    }

    /// Construct from an already-loaded seed (used in tests and by key
    /// management code that sources the secret from a vault).
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self {
            signing_key: SigningKey::from_bytes(&seed),
        }
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Sign a verified result, returning the 64-byte Ed25519 signature the
    /// contract's `BytesN<64>` argument expects.
    pub fn sign(&self, result: &SignedResult<'_>) -> [u8; 64] {
        let payload = match result {
            SignedResult::Win {
                game_id,
                winner_address,
            } => win_payload(*game_id, winner_address),
            SignedResult::Draw { game_id } => draw_payload(*game_id),
            SignedResult::PuzzleReward {
                recipient,
                amount,
                nonce,
            } => puzzle_reward_payload(recipient, *amount, *nonce),
        };
        let digest = Sha256::digest(&payload);
        self.signing_key.sign(&digest).to_bytes()
    }
}

/// Verify a produced signature against the contract's payload format. Useful
/// to the signer service itself and to contract-interop tests.
pub fn verify_signature(
    verifying_key: &VerifyingKey,
    result: &SignedResult<'_>,
    signature: &[u8; 64],
) -> bool {
    let payload = match result {
        SignedResult::Win {
            game_id,
            winner_address,
        } => win_payload(*game_id, winner_address),
        SignedResult::Draw { game_id } => draw_payload(*game_id),
        SignedResult::PuzzleReward {
            recipient,
            amount,
            nonce,
        } => puzzle_reward_payload(recipient, *amount, *nonce),
    };
    let digest = Sha256::digest(&payload);
    let signature = match Signature::from_bytes(signature) {
        Ok(sig) => sig,
        Err(_) => return false,
    };
    verifying_key.verify(&digest, &signature).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signer() -> ResultSigner {
        ResultSigner::from_seed([7u8; 32])
    }

    #[test]
    fn win_signature_verifies_against_contract_payload() {
        let signer = signer();
        let result = SignedResult::Win {
            game_id: 42,
            winner_address: "GABCDEFGHIJKLMNOPQRSTUVWXYZ",
        };
        let sig = signer.sign(&result);
        assert!(verify_signature(&signer.verifying_key(), &result, &sig));
    }

    #[test]
    fn draw_signature_verifies_against_contract_payload() {
        let signer = signer();
        let result = SignedResult::Draw { game_id: 42 };
        let sig = signer.sign(&result);
        assert!(verify_signature(&signer.verifying_key(), &result, &sig));
    }

    #[test]
    fn puzzle_reward_signature_verifies_against_contract_payload() {
        let signer = signer();
        let result = SignedResult::PuzzleReward {
            recipient: "GRECIPIENT",
            amount: 1000,
            nonce: 9,
        };
        let sig = signer.sign(&result);
        assert!(verify_signature(&signer.verifying_key(), &result, &sig));
    }

    #[test]
    fn signature_does_not_verify_for_a_different_result() {
        let signer = signer();
        let original = SignedResult::Draw { game_id: 42 };
        let sig = signer.sign(&original);
        let tampered = SignedResult::Draw { game_id: 43 };
        assert!(!verify_signature(
            &signer.verifying_key(),
            &tampered,
            &sig
        ));
    }
}
