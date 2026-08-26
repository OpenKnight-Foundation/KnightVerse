/**
 * puzzleRewardVerifier.ts
 *
 * SC-07: Backend utility for generating and verifying ed25519 puzzle-reward
 * signatures that match the on-chain `claim_puzzle_reward` contract function.
 *
 * Replaces the previous HMAC-based approach (which used a plaintext shared
 * secret) with a proper ed25519 keypair so the Soroban contract's
 * `env.crypto().ed25519_verify()` call succeeds.
 *
 * Canonical payload (must be identical to the contract):
 *   SHA-256( address_string_bytes || reward_amount_le8 || nonce_le8 )
 *
 * Usage
 * -----
 *   // Generate a fresh keypair (do once; store private key securely)
 *   const { privateKeyHex, publicKeyHex } = generateAdminKeypair();
 *
 *   // Sign a reward claim
 *   const sig = await signPuzzleReward({
 *     recipientAddress: "GABC...XYZ",
 *     rewardAmount:     BigInt(500),
 *     nonce:            BigInt(1),
 *     privateKeyHex,
 *   });
 *
 *   // Verify before broadcasting to the chain (optional sanity-check)
 *   const ok = await verifyPuzzleReward({
 *     recipientAddress: "GABC...XYZ",
 *     rewardAmount:     BigInt(500),
 *     nonce:            BigInt(1),
 *     signatureHex:     sig,
 *     publicKeyHex,
 *   });
 */

import {
  createHash,
  createPrivateKey,
  createPublicKey,
  generateKeyPairSync,
  KeyObject,
  sign,
  verify,
} from "crypto";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface PuzzleRewardParams {
  /** Stellar address string (e.g. "GABC…XYZ") */
  recipientAddress: string;
  /** Token reward amount as BigInt (i128 on-chain, fits in i64 for practical amounts) */
  rewardAmount: bigint;
  /** Replay-protection nonce (u64 on-chain) */
  nonce: bigint;
}

export interface SignParams extends PuzzleRewardParams {
  /** 32-byte ed25519 private key, hex-encoded */
  privateKeyHex: string;
}

export interface VerifyParams extends PuzzleRewardParams {
  /** 64-byte ed25519 signature, hex-encoded */
  signatureHex: string;
  /** 32-byte ed25519 public key, hex-encoded */
  publicKeyHex: string;
}

export interface AdminKeypair {
  /** 32-byte raw ed25519 private key (seed), hex-encoded */
  privateKeyHex: string;
  /** 32-byte ed25519 public key (verifying key), hex-encoded */
  publicKeyHex: string;
}

// ---------------------------------------------------------------------------
// Keypair generation
// ---------------------------------------------------------------------------

/**
 * Generates a fresh ed25519 keypair for use as the admin signing key.
 * Store `privateKeyHex` in a secrets manager (e.g. AWS Secrets Manager).
 * Register `publicKeyHex` with the Soroban contract's `initialize_puzzle_rewards`.
 */
export function generateAdminKeypair(): AdminKeypair {
  const { privateKey, publicKey } = generateKeyPairSync("ed25519");
  return {
    privateKeyHex: privateKeyToRawHex(privateKey),
    publicKeyHex: publicKeyToRawHex(publicKey),
  };
}

// ---------------------------------------------------------------------------
// Payload builder
// ---------------------------------------------------------------------------

/**
 * Builds the canonical 56-byte pre-image and returns its SHA-256 digest.
 *
 * Layout (matches contracts/game_contract/src/lib.rs, claim_puzzle_reward):
 *   address_string_bytes          (variable length)
 *   reward_amount_le8             (8 bytes, little-endian i64)
 *   nonce_le8                     (8 bytes, little-endian u64)
 */
function buildDigest(params: PuzzleRewardParams): Buffer {
  const addrBytes = Buffer.from(params.recipientAddress, "utf8");

  // amount as little-endian i64 (matches Rust's i64::to_le_bytes)
  const amountLE = writeBigInt64LE(params.rewardAmount);

  // nonce as little-endian u64 (matches Rust's u64::to_le_bytes)
  const nonceLE = writeBigUInt64LE(params.nonce);

  const preimage = Buffer.concat([addrBytes, amountLE, nonceLE]);
  return createHash("sha256").update(preimage).digest();
}

// ---------------------------------------------------------------------------
// Signing
// ---------------------------------------------------------------------------

/**
 * Signs a puzzle-reward claim with the admin ed25519 private key.
 *
 * @returns 64-byte ed25519 signature, hex-encoded.
 */
export function signPuzzleReward(params: SignParams): string {
  const digest = buildDigest(params);
  const privateKey = rawHexToPrivateKeyObject(params.privateKeyHex);

  // Node's `crypto.sign` with ed25519 KeyObject signs the raw bytes directly
  // (no additional hash — the digest is already SHA-256).
  const sigBuf = sign(null, digest, privateKey);
  return sigBuf.toString("hex");
}

// ---------------------------------------------------------------------------
// Verification (server-side sanity-check before broadcasting)
// ---------------------------------------------------------------------------

/**
 * Verifies a puzzle-reward signature using the admin ed25519 public key.
 * Returns `true` if valid, `false` otherwise.
 *
 * This mirrors the on-chain check so you can validate off-chain first.
 */
export function verifyPuzzleReward(params: VerifyParams): boolean {
  const digest = buildDigest(params);
  const publicKey = rawHexToPublicKeyObject(params.publicKeyHex);
  const sigBuf = Buffer.from(params.signatureHex, "hex");

  return verify(null, digest, publicKey, sigBuf);
}

// ---------------------------------------------------------------------------
// Internal helpers — key encoding/decoding
// ---------------------------------------------------------------------------

/**
 * Converts a Node KeyObject (ed25519 private key) to its 32-byte raw seed,
 * hex-encoded.  Node exports ed25519 private keys in PKCS#8 DER format;
 * the raw 32-byte seed occupies the last 32 bytes.
 */
function privateKeyToRawHex(key: KeyObject): string {
  // Export as DER (PKCS8); raw seed is the last 32 bytes.
  const der = key.export({ type: "pkcs8", format: "der" }) as Buffer;
  return der.subarray(der.length - 32).toString("hex");
}

/**
 * Converts a Node KeyObject (ed25519 public key) to its 32-byte raw bytes,
 * hex-encoded.  Node exports the SubjectPublicKeyInfo (SPKI) DER; the raw
 * 32-byte key occupies the last 32 bytes.
 */
function publicKeyToRawHex(key: KeyObject): string {
  const der = key.export({ type: "spki", format: "der" }) as Buffer;
  return der.subarray(der.length - 32).toString("hex");
}

/**
 * Reconstructs a Node ed25519 private KeyObject from a 32-byte hex seed.
 * Wraps the raw seed in the standard PKCS#8 DER envelope for ed25519.
 */
function rawHexToPrivateKeyObject(hex: string): KeyObject {
  const seed = Buffer.from(hex, "hex");
  if (seed.length !== 32) {
    throw new Error(`ed25519 private key must be 32 bytes; got ${seed.length}`);
  }
  // PKCS8 DER wrapper for an ed25519 private key (RFC 8410)
  // Header: 30 2e 02 01 00 30 05 06 03 2b 65 70 04 22 04 20
  const header = Buffer.from("302e020100300506032b657004220420", "hex");
  const der = Buffer.concat([header, seed]);
  return createPrivateKey({ key: der, format: "der", type: "pkcs8" });
}

/**
 * Reconstructs a Node ed25519 public KeyObject from a 32-byte hex verifying key.
 * Wraps the raw bytes in the standard SPKI DER envelope for ed25519.
 */
function rawHexToPublicKeyObject(hex: string): KeyObject {
  const rawKey = Buffer.from(hex, "hex");
  if (rawKey.length !== 32) {
    throw new Error(`ed25519 public key must be 32 bytes; got ${rawKey.length}`);
  }
  // SPKI DER wrapper for an ed25519 public key (RFC 8410)
  // Header: 30 2a 30 05 06 03 2b 65 70 03 21 00
  const header = Buffer.from("302a300506032b6570032100", "hex");
  const der = Buffer.concat([header, rawKey]);
  return createPublicKey({ key: der, format: "der", type: "spki" });
}

// ---------------------------------------------------------------------------
// BigInt little-endian serialization (matches Rust's to_le_bytes)
// ---------------------------------------------------------------------------

function writeBigInt64LE(value: bigint): Buffer {
  const buf = Buffer.alloc(8);
  buf.writeBigInt64LE(value);
  return buf;
}

function writeBigUInt64LE(value: bigint): Buffer {
  const buf = Buffer.alloc(8);
  buf.writeBigUInt64LE(value);
  return buf;
}
