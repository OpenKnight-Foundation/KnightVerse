import { renderHook, act } from "@testing-library/react";
import React from "react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  rpc,
  Horizon,
  nativeToScVal,
  Account,
  TransactionBuilder,
  Networks,
  Operation,
  Asset,
  xdr,
} from "@stellar/stellar-sdk";

// Mock @stellar/freighter-api
const mockSignTransaction = vi.fn();
const mockGetPublicKey = vi.fn();
vi.mock("@stellar/freighter-api", () => ({
  signTransaction: (...args: unknown[]) => mockSignTransaction(...args),
  getPublicKey: (...args: unknown[]) => mockGetPublicKey(...args),
  isConnected: vi.fn().mockResolvedValue(true),
  isAllowed: vi.fn().mockResolvedValue(true),
}));

import { AppProvider, useAppContext } from "@/context/walletContext";
import {
  YieldStakingContractClient,
  PuzzleRewardsContractClient,
  AiNftContractClient,
  ArbiterEscrowContractClient,
  createTypedContractClient,
  findRegisteredClient,
} from "@/lib/contracts";

const TEST_ADDRESS = "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H";
const TEST_CONTRACT_ID = "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC";

function createMockSimSuccess(retval: xdr.ScVal) {
  const td = new xdr.SorobanTransactionData({
    resources: new xdr.SorobanResources({
      footprint: new xdr.LedgerFootprint({ readOnly: [], readWrite: [] }),
      instructions: 100,
      diskReadBytes: 0,
      readBytes: 0,
      writeBytes: 0,
    }),
    resourceFee: 100n,
    ext: xdr.ExtensionPoint.v0(),
  });
  return rpc.parseRawSimulation({
    transactionData: td.toXDR("base64"),
    minResourceFee: "100",
    results: [{ auth: [], xdr: retval.toXDR("base64") }],
  });
}

describe("Wallet Context - Soroban Contract Invocation", () => {
  let spySimulate: ReturnType<typeof vi.spyOn>;
  let spySend: ReturnType<typeof vi.spyOn>;
  let spyPoll: ReturnType<typeof vi.spyOn>;
  let spyGetAccount: ReturnType<typeof vi.spyOn>;
  let spyLoadAccount: ReturnType<typeof vi.spyOn>;
  let spyFetchBaseFee: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    vi.clearAllMocks();

    spyFetchBaseFee = vi
      .spyOn(Horizon.Server.prototype, "fetchBaseFee")
      .mockResolvedValue(100);
    spyLoadAccount = vi
      .spyOn(Horizon.Server.prototype, "loadAccount")
      .mockResolvedValue({
        sequence: "100",
        balances: [],
      } as any);
    spyGetAccount = vi
      .spyOn(rpc.Server.prototype, "getAccount")
      .mockResolvedValue(new Account(TEST_ADDRESS, "100"));

    spySimulate = vi.spyOn(rpc.Server.prototype, "simulateTransaction");
    spySend = vi.spyOn(rpc.Server.prototype, "sendTransaction");
    spyPoll = vi.spyOn(rpc.Server.prototype, "pollTransaction");

    mockSignTransaction.mockResolvedValue("AAAA_SIGNED_MOCK_ENVELOPE_XDR");

    // Provide window.localStorage mock
    if (typeof window !== "undefined") {
      const storage: Record<string, string> = {
        freighter_address: TEST_ADDRESS,
      };
      vi.stubGlobal("localStorage", {
        getItem: (key: string) => storage[key] ?? null,
        setItem: (key: string, val: string) => {
          storage[key] = val;
        },
        removeItem: (key: string) => {
          delete storage[key];
        },
        clear: () => {
          for (const k in storage) delete storage[k];
        },
      });
    }
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("throws when invoking a contract with no wallet connected", async () => {
    vi.stubGlobal("localStorage", {
      getItem: () => null,
      setItem: () => {},
      removeItem: () => {},
      clear: () => {},
    });

    const wrapper = ({ children }: { children: React.ReactNode }) =>
      React.createElement(AppProvider, null, children);

    const { result } = renderHook(() => useAppContext(), { wrapper });

    await expect(
      result.current.invokeSorobanContract(TEST_CONTRACT_ID, "test_fn"),
    ).rejects.toThrow(/No wallet connected/);
  });

  it("executes simulate -> sign -> submit -> poll successfully", async () => {
    // Mock simulate response (success)
    spySimulate.mockResolvedValue(createMockSimSuccess(nativeToScVal(42)));

    // Valid signed transaction for TransactionBuilder.fromXDR
    const acct = new Account(TEST_ADDRESS, "100");
    const validTx = new TransactionBuilder(acct, {
      fee: "100",
      networkPassphrase: Networks.TESTNET,
    })
      .addOperation(
        Operation.payment({
          destination: TEST_ADDRESS,
          amount: "1",
          asset: Asset.native(),
        }),
      )
      .setTimeout(30)
      .build();
    mockSignTransaction.mockResolvedValue(validTx.toXDR());

    // Mock sendTransaction (success)
    spySend.mockResolvedValue({
      status: "PENDING",
      hash: "mock_tx_hash_1234567890",
    } as any);

    // Mock pollTransaction (success)
    spyPoll.mockResolvedValue({
      status: rpc.Api.GetTransactionStatus.SUCCESS,
      returnValue: nativeToScVal(42),
    } as any);

    const wrapper = ({ children }: { children: React.ReactNode }) =>
      React.createElement(AppProvider, null, children);

    const { result } = renderHook(() => useAppContext(), { wrapper });

    const response = await act(async () => {
      return await result.current.invokeSorobanContract(
        TEST_CONTRACT_ID,
        "calculate_reward",
        [10],
      );
    });

    expect(spySimulate).toHaveBeenCalledTimes(1);
    expect(mockSignTransaction).toHaveBeenCalled();
    expect(spySend).toHaveBeenCalledTimes(1);
    expect(spyPoll).toHaveBeenCalledWith(
      "mock_tx_hash_1234567890",
      expect.objectContaining({ attempts: 30 }),
    );
    expect(response.status).toBe("SUCCESS");
    expect(response.hash).toBe("mock_tx_hash_1234567890");
    expect(response.returnValue).toBe(42n);
  });

  it("handles simulateOnly queries without signing or submitting", async () => {
    spySimulate.mockResolvedValue(createMockSimSuccess(nativeToScVal("query_result")));

    const wrapper = ({ children }: { children: React.ReactNode }) =>
      React.createElement(AppProvider, null, children);

    const { result } = renderHook(() => useAppContext(), { wrapper });

    const data = await act(async () => {
      return await result.current.invokeSorobanContract(
        TEST_CONTRACT_ID,
        "get_value",
        [],
        { simulateOnly: true },
      );
    });

    expect(spySimulate).toHaveBeenCalledTimes(1);
    expect(mockSignTransaction).not.toHaveBeenCalled();
    expect(spySend).not.toHaveBeenCalled();
    expect(spyPoll).not.toHaveBeenCalled();
    expect(data).toBe("query_result");
  });

  it("throws when simulation fails with isSimulationError", async () => {
    spySimulate.mockResolvedValue({
      error: "HostError: Error(Contract, #101)",
    } as any);

    const wrapper = ({ children }: { children: React.ReactNode }) =>
      React.createElement(AppProvider, null, children);

    const { result } = renderHook(() => useAppContext(), { wrapper });

    await expect(
      result.current.invokeSorobanContract(TEST_CONTRACT_ID, "failing_method"),
    ).rejects.toThrow(/Soroban simulation failed for failing_method/);

    expect(mockSignTransaction).not.toHaveBeenCalled();
    expect(spySend).not.toHaveBeenCalled();
  });

  it("throws when transaction submission returns status ERROR", async () => {
    spySimulate.mockResolvedValue(createMockSimSuccess(nativeToScVal(1)));

    const acct = new Account(TEST_ADDRESS, "100");
    const validTx = new TransactionBuilder(acct, {
      fee: "100",
      networkPassphrase: Networks.TESTNET,
    })
      .addOperation(
        Operation.payment({
          destination: TEST_ADDRESS,
          amount: "1",
          asset: Asset.native(),
        }),
      )
      .setTimeout(30)
      .build();
    mockSignTransaction.mockResolvedValue(validTx.toXDR());

    spySend.mockResolvedValue({
      status: "ERROR",
      errorResult: { toXDR: () => "BASE64_TX_ERROR" },
    } as any);

    const wrapper = ({ children }: { children: React.ReactNode }) =>
      React.createElement(AppProvider, null, children);

    const { result } = renderHook(() => useAppContext(), { wrapper });

    await expect(
      result.current.invokeSorobanContract(TEST_CONTRACT_ID, "some_action"),
    ).rejects.toThrow(/Soroban transaction submission failed/);
  });

  it("throws when pollTransaction returns status FAILED", async () => {
    spySimulate.mockResolvedValue(createMockSimSuccess(nativeToScVal(1)));

    const acct = new Account(TEST_ADDRESS, "100");
    const validTx = new TransactionBuilder(acct, {
      fee: "100",
      networkPassphrase: Networks.TESTNET,
    })
      .addOperation(
        Operation.payment({
          destination: TEST_ADDRESS,
          amount: "1",
          asset: Asset.native(),
        }),
      )
      .setTimeout(30)
      .build();
    mockSignTransaction.mockResolvedValue(validTx.toXDR());

    spySend.mockResolvedValue({
      status: "PENDING",
      hash: "mock_hash_failed",
    } as any);

    spyPoll.mockResolvedValue({
      status: rpc.Api.GetTransactionStatus.FAILED,
      resultXdr: "FAILED_RESULT_XDR",
    } as any);

    const wrapper = ({ children }: { children: React.ReactNode }) =>
      React.createElement(AppProvider, null, children);

    const { result } = renderHook(() => useAppContext(), { wrapper });

    await expect(
      result.current.invokeSorobanContract(TEST_CONTRACT_ID, "failing_action"),
    ).rejects.toThrow(/Soroban transaction failed/);
  });

  it("throws when pollTransaction times out (status NOT_FOUND)", async () => {
    spySimulate.mockResolvedValue(createMockSimSuccess(nativeToScVal(1)));

    const acct = new Account(TEST_ADDRESS, "100");
    const validTx = new TransactionBuilder(acct, {
      fee: "100",
      networkPassphrase: Networks.TESTNET,
    })
      .addOperation(
        Operation.payment({
          destination: TEST_ADDRESS,
          amount: "1",
          asset: Asset.native(),
        }),
      )
      .setTimeout(30)
      .build();
    mockSignTransaction.mockResolvedValue(validTx.toXDR());

    spySend.mockResolvedValue({
      status: "PENDING",
      hash: "mock_hash_timeout",
    } as any);

    spyPoll.mockResolvedValue({
      status: rpc.Api.GetTransactionStatus.NOT_FOUND,
    } as any);

    const wrapper = ({ children }: { children: React.ReactNode }) =>
      React.createElement(AppProvider, null, children);

    const { result } = renderHook(() => useAppContext(), { wrapper });

    await expect(
      result.current.invokeSorobanContract(TEST_CONTRACT_ID, "timeout_action"),
    ).rejects.toThrow(/Soroban transaction timed out waiting for confirmation/);
  });
});

describe("Typed Contract Clients & Registry", () => {
  it("creates and retrieves typed contract clients via getContractClient", () => {
    const wrapper = ({ children }: { children: React.ReactNode }) =>
      React.createElement(AppProvider, null, children);

    const { result } = renderHook(() => useAppContext(), { wrapper });

    const stakingClient = result.current.getContractClient<YieldStakingContractClient>(
      "staking",
      TEST_CONTRACT_ID,
    );
    expect(stakingClient).toBeInstanceOf(YieldStakingContractClient);
    expect(stakingClient.contractId).toBe(TEST_CONTRACT_ID);

    const puzzleClient = result.current.getContractClient<PuzzleRewardsContractClient>(
      "puzzle",
      TEST_CONTRACT_ID,
    );
    expect(puzzleClient).toBeInstanceOf(PuzzleRewardsContractClient);

    const nftClient = result.current.getContractClient<AiNftContractClient>(
      "nft",
      TEST_CONTRACT_ID,
    );
    expect(nftClient).toBeInstanceOf(AiNftContractClient);

    const escrowClient = result.current.getContractClient<ArbiterEscrowContractClient>(
      "escrow",
      TEST_CONTRACT_ID,
    );
    expect(escrowClient).toBeInstanceOf(ArbiterEscrowContractClient);
  });

  it("invokes YieldStakingContractClient methods properly", async () => {
    const mockInvoker = vi.fn().mockResolvedValue({
      status: "SUCCESS",
      hash: "hash123",
      returnValue: 500n,
    });

    const client = new YieldStakingContractClient(TEST_CONTRACT_ID, mockInvoker);

    await client.stake("user1", "pool1", 1000n);
    expect(mockInvoker).toHaveBeenCalledWith(
      TEST_CONTRACT_ID,
      "stake",
      ["user1", "pool1", 1000n],
      expect.objectContaining({ simulateOnly: false }),
    );

    await client.unstake("user1", "pool1", 500n);
    expect(mockInvoker).toHaveBeenCalledWith(
      TEST_CONTRACT_ID,
      "unstake",
      ["user1", "pool1", 500n],
      expect.objectContaining({ simulateOnly: false }),
    );

    await client.harvestYield("user1", 1);
    expect(mockInvoker).toHaveBeenCalledWith(
      TEST_CONTRACT_ID,
      "harvest_yield",
      ["user1", 1n],
      expect.objectContaining({ simulateOnly: false }),
    );

    const escrowBal = await client.getEscrowBalance("user1");
    expect(mockInvoker).toHaveBeenCalledWith(
      TEST_CONTRACT_ID,
      "escrow_balance",
      ["user1"],
      expect.objectContaining({ simulateOnly: true }),
    );
    expect(escrowBal).toBe(500n);
  });

  it("invokes PuzzleRewardsContractClient methods properly", async () => {
    const mockInvoker = vi.fn().mockResolvedValue({
      status: "SUCCESS",
      hash: "hash_puzzle",
      returnValue: 100n,
    });

    const client = new PuzzleRewardsContractClient(TEST_CONTRACT_ID, mockInvoker);

    await client.claimPuzzleReward("player1", 5, 100n);
    expect(mockInvoker).toHaveBeenCalledWith(
      TEST_CONTRACT_ID,
      "claim_puzzle_reward",
      ["player1", 5n, 100n],
      expect.objectContaining({ simulateOnly: false }),
    );

    const balance = await client.getRewardBalance("player1");
    expect(balance).toBe(100n);
  });

  it("invokes AiNftContractClient methods properly", async () => {
    const mockInvoker = vi.fn().mockResolvedValue({
      status: "SUCCESS",
      hash: "hash_nft",
      returnValue: "owner_address",
    });

    const client = new AiNftContractClient(TEST_CONTRACT_ID, mockInvoker);

    await client.mint("recipient", "ipfs://uri", "Knight #1", "KNIGHT");
    expect(mockInvoker).toHaveBeenCalledWith(
      TEST_CONTRACT_ID,
      "mint",
      ["recipient", "ipfs://uri", "Knight #1", "KNIGHT"],
      expect.objectContaining({ simulateOnly: false }),
    );

    const owner = await client.ownerOf(1);
    expect(owner).toBe("owner_address");
  });

  it("invokes ArbiterEscrowContractClient methods properly", async () => {
    const mockInvoker = vi.fn().mockResolvedValue({
      status: "SUCCESS",
      hash: "hash_escrow",
      returnValue: null,
    });

    const client = new ArbiterEscrowContractClient(TEST_CONTRACT_ID, mockInvoker);

    await client.createEscrow(10, "playerA", "playerB", "arbiter1", 5000n, "tokenX");
    expect(mockInvoker).toHaveBeenCalledWith(
      TEST_CONTRACT_ID,
      "create_escrow",
      [10n, "playerA", "playerB", "arbiter1", 5000n, "tokenX"],
      expect.objectContaining({ simulateOnly: false }),
    );

    await client.settleMatch(10, "admin1", "playerA");
    expect(mockInvoker).toHaveBeenCalledWith(
      TEST_CONTRACT_ID,
      "settle_match",
      [10n, "admin1", "playerA"],
      expect.objectContaining({ simulateOnly: false }),
    );
  });

  it("supports registry lookup and registration", () => {
    const mockInvoker = vi.fn();
    const client = createTypedContractClient("staking", "C_REGISTRY_TEST", mockInvoker);
    expect(client).toBeInstanceOf(YieldStakingContractClient);

    const found = findRegisteredClient("C_REGISTRY_TEST");
    expect(found).toBe(client);
  });
});
