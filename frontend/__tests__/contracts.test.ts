/**
 * SC-91 (#1420) — the env-aware wrapper around the generated contract clients
 * in `frontend/contracts/`.
 *
 * The generated clients themselves are committed generator output, so they are
 * mocked here; these tests pin down the wiring the app relies on: contract ids
 * read from the environment, proof normalisation, typed arguments, and the
 * read-vs-write dispatch of `invokeBoundContract`.
 */
import { Buffer } from "buffer";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const generated = vi.hoisted(() => ({
  claimPuzzleReward: vi.fn(),
  stake: vi.fn(),
}));

vi.mock("@/contracts/game_contract/src/index", () => ({
  Client: class {
    options: unknown;
    claim_puzzle_reward = generated.claimPuzzleReward;
    constructor(options: unknown) {
      this.options = options;
    }
  },
}));

vi.mock("@/contracts/yield_staking/src/index", () => ({
  Client: class {
    options: unknown;
    stake = generated.stake;
    constructor(options: unknown) {
      this.options = options;
    }
  },
}));

import {
  claimPuzzleReward,
  createGameContractClient,
  createYieldStakingClient,
  getGameContractId,
  getYieldStakingConfig,
  invokeBoundContract,
  normalizePuzzleRewardProof,
  parsePuzzleRewardProof,
  resolveBoundContract,
  stakeInPool,
  toTokenAmount,
  type ContractSigner,
} from "@/contracts/client";

const GAME_CONTRACT_ID = "CGAMECONTRACTXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX";
const STAKING_CONTRACT_ID = "CSTAKINGCONTRACTXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX";
const POOL_ADDRESS = "CPOOLXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX";
const PLAYER = "GPLAYERXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX";

// BigInt literals need an ES2020 target and the app targets ES2017, so the
// expected values below are built explicitly.
const bn = (value: string): bigint => BigInt(value);

const signer: ContractSigner = {
  address: PLAYER,
  sign: vi.fn(async (xdr: string) => `signed:${xdr}`),
};

/** Fake `AssembledTransaction` + `SentTransaction` pair. */
function fakeCall({
  isReadCall = false,
  result,
  hash,
}: {
  isReadCall?: boolean;
  result?: unknown;
  hash?: string;
}) {
  return {
    isReadCall,
    result,
    signAndSend: vi.fn(async () => ({
      result,
      getTransactionResponse: hash ? { txHash: hash } : undefined,
      sendTransactionResponse: undefined,
    })),
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.stubEnv("NEXT_PUBLIC_GAME_CONTRACT_ID", GAME_CONTRACT_ID);
  vi.stubEnv("NEXT_PUBLIC_YIELD_STAKING_CONTRACT_ID", STAKING_CONTRACT_ID);
  vi.stubEnv("NEXT_PUBLIC_YIELD_STAKING_POOL_ADDRESS", POOL_ADDRESS);
});

afterEach(() => {
  vi.unstubAllEnvs();
  vi.restoreAllMocks();
});

describe("contract configuration", () => {
  it("keeps the app on the legacy path when no contract is configured", () => {
    vi.stubEnv("NEXT_PUBLIC_GAME_CONTRACT_ID", "");
    vi.stubEnv("NEXT_PUBLIC_YIELD_STAKING_CONTRACT_ID", "");
    vi.stubEnv("NEXT_PUBLIC_YIELD_STAKING_POOL_ADDRESS", "");

    expect(getGameContractId()).toBeUndefined();
    expect(getYieldStakingConfig()).toBeUndefined();
    expect(resolveBoundContract(GAME_CONTRACT_ID)).toBeUndefined();
  });

  it("requires both staking variables before enabling the contract path", () => {
    vi.stubEnv("NEXT_PUBLIC_YIELD_STAKING_POOL_ADDRESS", "");
    expect(getYieldStakingConfig()).toBeUndefined();
  });

  it("builds clients from the configured contract ids and network", () => {
    const game = createGameContractClient() as unknown as { options: unknown };
    expect(game.options).toMatchObject({
      contractId: GAME_CONTRACT_ID,
      networkPassphrase: expect.any(String),
      rpcUrl: expect.any(String),
    });

    expect(createYieldStakingClient()).toBeDefined();
  });

  it("refuses to build a client for an unconfigured contract", () => {
    vi.stubEnv("NEXT_PUBLIC_GAME_CONTRACT_ID", "");
    vi.stubEnv("NEXT_PUBLIC_YIELD_STAKING_CONTRACT_ID", "");
    vi.stubEnv("NEXT_PUBLIC_YIELD_STAKING_POOL_ADDRESS", "");
    expect(() => createGameContractClient()).toThrow(/not configured/);
    expect(() => createYieldStakingClient()).toThrow(/not configured/);
  });
});

describe("toTokenAmount", () => {
  it("converts whole and fractional units to 7-decimal bigints", () => {
    expect(toTokenAmount(1)).toBe(bn("10000000"));
    expect(toTokenAmount(1.5)).toBe(bn("15000000"));
    expect(toTokenAmount(0.0000001)).toBe(bn("1"));
  });

  it("is not fooled by float artifacts", () => {
    expect(toTokenAmount(0.1 + 0.2)).toBe(bn("3000000"));
  });

  it("rejects non-finite amounts", () => {
    expect(() => toTokenAmount(Number.NaN)).toThrow(TypeError);
  });
});

describe("puzzle reward proofs", () => {
  it("normalises backend fields into the contract argument types", () => {
    const proof = normalizePuzzleRewardProof({
      recipient: PLAYER,
      reward_amount: "50000000",
      nonce: "7",
      signature: Buffer.from([1, 2, 3]).toString("base64"),
    });

    expect(proof).toEqual({
      recipient: PLAYER,
      reward_amount: bn("50000000"),
      nonce: bn("7"),
      signature: Buffer.from([1, 2, 3]),
    });
  });

  it("accepts hex signatures and byte arrays", () => {
    expect(
      normalizePuzzleRewardProof({
        recipient: PLAYER,
        reward_amount: 1,
        nonce: 1,
        signature: "0x0a0b",
      }).signature,
    ).toEqual(Buffer.from([10, 11]));

    expect(
      normalizePuzzleRewardProof({
        recipient: PLAYER,
        reward_amount: 1,
        nonce: 1,
        signature: [9, 8],
      }).signature,
    ).toEqual(Buffer.from([9, 8]));
  });

  it("throws on malformed proofs and parses to null instead", () => {
    expect(() => normalizePuzzleRewardProof({ reward_amount: 1 })).toThrow(
      /recipient/,
    );

    vi.spyOn(console, "warn").mockImplementation(() => {});
    expect(parsePuzzleRewardProof(undefined)).toBeNull();
    expect(parsePuzzleRewardProof({ nonce: 1 })).toBeNull();
  });
});

describe("claimPuzzleReward", () => {
  it("calls the generated client with typed arguments and returns the hash", async () => {
    generated.claimPuzzleReward.mockResolvedValue(fakeCall({ hash: "abc123" }));

    const proof = normalizePuzzleRewardProof({
      recipient: PLAYER,
      reward_amount: "50000000",
      nonce: "7",
      signature: Buffer.from([4, 5, 6]).toString("base64"),
    });

    await expect(claimPuzzleReward({ proof, signer })).resolves.toEqual({
      hash: "abc123",
    });

    expect(generated.claimPuzzleReward).toHaveBeenCalledWith({
      recipient: PLAYER,
      reward_amount: bn("50000000"),
      nonce: bn("7"),
      signature: Buffer.from([4, 5, 6]),
    });
    expect(signer.sign).not.toHaveBeenCalled();
  });
});

describe("stakeInPool", () => {
  it("stakes into the configured pool through the generated client", async () => {
    generated.stake.mockResolvedValue(fakeCall({ hash: "stakehash" }));

    await expect(
      stakeInPool({
        caller: PLAYER,
        pool: POOL_ADDRESS,
        amount: toTokenAmount(10.35),
        signer,
      }),
    ).resolves.toEqual({ hash: "stakehash" });

    expect(generated.stake).toHaveBeenCalledWith({
      caller: PLAYER,
      pool: POOL_ADDRESS,
      amount: bn("103500000"),
    });
  });
});

describe("invokeBoundContract", () => {
  it("refuses to guess an ABI for a contract without bindings", async () => {
    await expect(
      invokeBoundContract({
        contractId: "CNODEPLOYEDCONTRACTXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
        method: "anything",
        signer,
      }),
    ).rejects.toThrow(/No TypeScript bindings/);
  });

  it("rejects unknown methods on a bound contract", async () => {
    await expect(
      invokeBoundContract({
        contractId: GAME_CONTRACT_ID,
        method: "not_a_method",
        signer,
      }),
    ).rejects.toThrow(/has no method/);
  });

  it("resolves read calls from simulation without signing", async () => {
    const readCall = fakeCall({ isReadCall: true, result: bn("42") });
    generated.claimPuzzleReward.mockResolvedValue(readCall);

    await expect(
      invokeBoundContract({
        contractId: GAME_CONTRACT_ID,
        method: "claim_puzzle_reward",
        args: { recipient: PLAYER, reward_amount: bn("1"), nonce: bn("1"), signature: Buffer.alloc(64) },
        signer,
      }),
    ).resolves.toBe(bn("42"));

    expect(readCall.signAndSend).not.toHaveBeenCalled();
  });

  it("signs and submits write calls", async () => {
    const writeCall = fakeCall({ result: "submitted" });
    generated.stake.mockResolvedValue(writeCall);

    await expect(
      invokeBoundContract({
        contractId: STAKING_CONTRACT_ID,
        method: "stake",
        args: { caller: PLAYER, pool: POOL_ADDRESS, amount: bn("1") },
        signer,
      }),
    ).resolves.toBe("submitted");

    expect(writeCall.signAndSend).toHaveBeenCalled();
  });
});
