"use client";
import React, { createContext, useContext, useEffect, useState } from "react";
import {
  Horizon,
  TransactionBuilder,
  Operation,
  Asset,
  Contract,
  Account,
  rpc,
  nativeToScVal,
  scValToNative,
  Address,
  StrKey,
  xdr,
} from "@stellar/stellar-sdk";

import {
  HORIZON_URL,
  SOROBAN_RPC,
  NETWORK_PASSPHRASE,
  CONTRACT_STAKING,
  CONTRACT_GAME_ESCROW,
  CONTRACT_AI_NFT,
} from "@/lib/api";
import { invokeBoundContract as invokeGeneratedBinding } from "@/contracts/client";
import {
  InvokeContractOptions,
  TypedContractClient,
  KnownContractType,
  createTypedContractClient,
} from "@/lib/contracts";

type AppContextType = {
  address?: string;
  status: "connected" | "disconnected" | "connecting" | "error";
  balance?: string | React.ReactNode;
  connectWallet: () => Promise<void>;
  disconnectWallet: () => Promise<void>;
  /**
   * Send a classic payment. Omit `asset` for native XLM; pass `{ code, issuer }`
   * for an issued asset such as Circle's USDC/EURC (FE-133).
   */
  sendPayment: (
    destination: string,
    amount: string | number,
    asset?: { code: string; issuer: string },
  ) => Promise<any>;
  /** Convenience wrapper over {@link sendPayment} for native XLM. */
  sendXLM: (destination: string, amount: string | number) => Promise<any>;
  /**
   * Sign an XDR envelope with the connected Freighter wallet. Returns the
   * signed envelope XDR — the shape the generated contract clients in
   * `@/contracts` expect from their signer.
   */
  signTransaction: (txXdr: string) => Promise<string>;
  /**
   * Invoke a Soroban contract method through the generated TypeScript
   * bindings (`@/contracts`, SC-91). `params` is keyed by the contract's
   * parameter names. The contract id must have bindings registered via a
   * `NEXT_PUBLIC_*_CONTRACT_ID` variable; otherwise this throws instead of
   * guessing an ABI.
   */
  invokeBoundContract: (
    contractId: string,
    functionName: string,
    params?: Record<string, unknown>,
  ) => Promise<unknown>;
  /**
   * Invokes a Soroban smart contract using @stellar/stellar-sdk's rpc module:
   * simulate -> sign (Freighter) -> submit -> poll.
   * If `options.simulateOnly` is true, simulates and returns the value without submitting.
   */
  invokeSorobanContract: (
    contractId: string,
    functionName: string,
    args?: any[],
    options?: InvokeContractOptions,
  ) => Promise<any>;
  /**
   * Returns a typed contract client for Staking, Puzzle Rewards, AI NFTs, or Escrow.
   */
  getContractClient: <T extends TypedContractClient = TypedContractClient>(
    contractType: KnownContractType | string,
    contractId?: string,
  ) => T;
};

const AppContext = createContext<AppContextType | undefined>(undefined);

function toScVal(arg: any): xdr.ScVal {
  if (
    arg &&
    typeof arg === "object" &&
    ("_arm" in arg || "_value" in arg || arg instanceof xdr.ScVal)
  ) {
    return arg as xdr.ScVal;
  }
  if (
    typeof arg === "string" &&
    (StrKey.isValidEd25519PublicKey(arg) || StrKey.isValidContract(arg))
  ) {
    try {
      return new Address(arg).toScVal();
    } catch {
      // fallback
    }
  }
  return nativeToScVal(arg);
}

function resolveContractId(type: KnownContractType | string): string {
  switch (type.toLowerCase()) {
    case "staking":
    case "yield_staking":
      return CONTRACT_STAKING;
    case "puzzle":
    case "puzzles":
    case "puzzle_rewards":
    case "game":
      return CONTRACT_GAME_ESCROW;
    case "nft":
    case "ai_nft":
      return CONTRACT_AI_NFT;
    case "escrow":
    case "arbiter_escrow":
      return CONTRACT_GAME_ESCROW;
    default:
      return "";
  }
}

export function AppProvider({ children }: { children: React.ReactNode }) {
  const [address, setAddress] = useState<string | undefined>(undefined);
  const [status, setStatus] =
    useState<AppContextType["status"]>("disconnected");
  const server = new Horizon.Server(HORIZON_URL);

  useEffect(() => {
    // Auto-detect previously connected Freighter account safely across environments
    try {
      if (typeof window !== "undefined" && window.localStorage) {
        const stored = window.localStorage.getItem("freighter_address");
        if (stored) {
          setAddress(stored);
          setStatus("connected");
        }
      }
    } catch {
      // Ignore storage errors in non-browser/restricted environments
    }
  }, []);

  const isFreighterAvailable = (): boolean => {
    // freighter v2 exposes `window.freighterApi`, older versions add `window.freighter`
    return (
      typeof window !== "undefined" &&
      (!!(window as any).freighterApi || !!(window as any).freighter)
    );
  };

  const connectWallet = async () => {
    setStatus("connecting");
    try {
      if (!isFreighterAvailable())
        throw new Error("Freighter not found. Please install Freighter.");

      // Prefer the freighter-api package if available on window
      const freighter =
        (window as any).freighterApi || (window as any).freighter;
      // try to get public key
      const publicKey = await (freighter.getPublicKey
        ? freighter.getPublicKey()
        : freighter.getPublicKey());
      if (!publicKey)
        throw new Error("Unable to read public key from Freighter");
      setAddress(publicKey);
      try {
        if (typeof window !== "undefined" && window.localStorage) {
          window.localStorage.setItem("freighter_address", publicKey);
        }
      } catch {
        // Ignore storage errors
      }
      setStatus("connected");
    } catch (err) {
      console.error("connectWallet", err);
      setStatus("error");
      throw err;
    }
  };

  const disconnectWallet = async () => {
    setAddress(undefined);
    try {
      if (typeof window !== "undefined" && window.localStorage) {
        window.localStorage.removeItem("freighter_address");
      }
    } catch {
      // Ignore storage errors
    }
    setStatus("disconnected");
  };

  async function signWithFreighter(txXDR: string): Promise<string> {
    // 1) Try official @stellar/freighter-api package if available
    try {
      const freighterApi = await import("@stellar/freighter-api");
      // Only use the package when the extension is actually present: without
      // it, signTransaction() waits for an extension reply that never comes.
      if (
        freighterApi &&
        typeof freighterApi.signTransaction === "function" &&
        (await freighterApi.isConnected())
      ) {
        const signed = await freighterApi.signTransaction(txXDR, {
          networkPassphrase: NETWORK_PASSPHRASE,
        });
        if (signed) return signed;
      }
    } catch {
      // Fall through to window injection
    }

    const freighter =
      typeof window !== "undefined"
        ? (window as any).freighterApi || (window as any).freighter
        : undefined;
    if (!freighter) throw new Error("Freighter not available");

    // freighter API variants differ; try common shapes
    // 1) freighterApi.signTransaction({ network: NETWORK_PASSPHRASE, transaction: txXDR }) -> returns signedXDR
    // 2) freighter.signTransaction(txXDR) -> returns signedXDR
    try {
      if (freighter.signTransaction) {
        const res = await freighter.signTransaction(txXDR, NETWORK_PASSPHRASE);
        // some versions return { signed_envelope_xdr }
        if (res && typeof res === "object" && res.signed_envelope_xdr)
          return res.signed_envelope_xdr;
        if (typeof res === "string") return res;
      }
      if (freighter.sign) {
        const res = await freighter.sign({
          transaction: txXDR,
          network: NETWORK_PASSPHRASE,
        });
        if (res && res.signed_envelope_xdr) return res.signed_envelope_xdr;
      }
      // last resort: try freighterApi.signTransaction named export
      if (freighter.requestSignTransaction) {
        const res = await freighter.requestSignTransaction(txXDR);
        if (res && res.signed_envelope_xdr) return res.signed_envelope_xdr;
      }
    } catch (e) {
      console.error("Freighter signing failed:", e);
      throw e;
    }
    throw new Error(
      "Freighter signing interface not supported in this environment",
    );
  }

  const sendPayment = async (
    destination: string,
    amount: string | number,
    asset?: { code: string; issuer: string },
  ) => {
    if (!address) throw new Error("No wallet connected");
    // Issued assets must be addressed by code *and* issuer — the code alone is
    // not unique on Stellar (FE-133).
    const paymentAsset = asset
      ? new Asset(asset.code, asset.issuer)
      : Asset.native();
    const acct = await server.loadAccount(address);
    const fee = await server.fetchBaseFee();
    const tx = new TransactionBuilder(acct, {
      fee: fee.toString(),
      networkPassphrase: NETWORK_PASSPHRASE,
    })
      .addOperation(
        Operation.payment({
          destination,
          asset: paymentAsset,
          amount: String(amount),
        }),
      )
      .setTimeout(30)
      .build();

    const txXDR = tx.toXDR();
    const signedEnvelopeXDR = await signWithFreighter(txXDR);

    // submit signed XDR
    try {
      const txObj = TransactionBuilder.fromXDR(
        signedEnvelopeXDR,
        NETWORK_PASSPHRASE,
      );
      return await server.submitTransaction(txObj);
    } catch (e) {
      // Report the real failure: submitting the raw XDR string as a fallback
      // always threw "Cannot read properties of undefined (reading 'type')"
      // and hid the original error.
      console.error("submitTransaction failed", e);
      throw e;
    }
  };

  const sendXLM = (destination: string, amount: string | number) =>
    sendPayment(destination, amount);

  const invokeSorobanContract = async (
    contractId: string,
    functionName: string,
    args: any[] = [],
    options: InvokeContractOptions = {},
  ): Promise<any> => {
    if (!address) {
      throw new Error("No wallet connected. Please connect Freighter first.");
    }

    try {
      // Modern @stellar/stellar-sdk rpc module
      const rpcServer = new rpc.Server(SOROBAN_RPC, {
        allowHttp: SOROBAN_RPC.startsWith("http://"),
      });

      // 1. Convert arguments to ScVals
      const scValArgs = args.map(toScVal);

      // 2. Fetch source account (try Soroban RPC first, fallback to Horizon)
      let sourceAccount: Account;
      try {
        sourceAccount = await rpcServer.getAccount(address);
      } catch {
        const horizonAcct = await server.loadAccount(address);
        sourceAccount = new Account(address, horizonAcct.sequence);
      }

      // 3. Build contract invocation operation and transaction
      const baseFee =
        options.fee?.toString() ?? (await server.fetchBaseFee()).toString();
      const contract = new Contract(contractId);
      const callOp = contract.call(functionName, ...scValArgs);

      const tx = new TransactionBuilder(sourceAccount, {
        fee: baseFee,
        networkPassphrase: NETWORK_PASSPHRASE,
      })
        .addOperation(callOp)
        .setTimeout(options.timeoutInSeconds ?? 60)
        .build();

      // 4. SIMULATE transaction using rpc.Server
      const sim = await rpcServer.simulateTransaction(tx);
      if (rpc.Api.isSimulationError(sim)) {
        const errorMsg = sim.error || "Simulation failed";
        console.error("Soroban simulation error:", errorMsg, sim);
        throw new Error(
          `Soroban simulation failed for ${functionName}: ${errorMsg}`,
        );
      }

      // If simulateOnly is requested (read-only query), return decoded retval
      if (options.simulateOnly) {
        if (sim.result?.retval) {
          try {
            return scValToNative(sim.result.retval);
          } catch {
            return sim.result.retval;
          }
        }
        return sim.result;
      }

      // 5. ASSEMBLE transaction with footprint, auth entries, and resource fees
      const preparedTx = rpc.assembleTransaction(tx, sim).build();

      // 6. SIGN with Freighter
      const txXDR = preparedTx.toXDR();
      const signedEnvelopeXDR = await signWithFreighter(txXDR);
      const signedTx = TransactionBuilder.fromXDR(
        signedEnvelopeXDR,
        NETWORK_PASSPHRASE,
      );

      // 7. SUBMIT to Soroban RPC
      const sendResponse = await rpcServer.sendTransaction(signedTx);
      if (sendResponse.status === "ERROR") {
        const errDetail =
          sendResponse.errorResult?.toXDR("base64") ||
          JSON.stringify(sendResponse);
        throw new Error(`Soroban transaction submission failed: ${errDetail}`);
      }

      // 8. POLL for transaction confirmation
      const pollInterval = options.pollIntervalMs ?? 1000;
      const maxAttempts = options.maxPollAttempts ?? 30;

      const txResult = await rpcServer.pollTransaction(sendResponse.hash, {
        attempts: maxAttempts,
        sleepStrategy: () => pollInterval,
      });

      if (txResult.status === rpc.Api.GetTransactionStatus.FAILED) {
        const failDetail =
          (txResult as any).resultXdr?.toString() ||
          (txResult as any).errorResult?.toXDR?.("base64") ||
          "Transaction execution failed on-chain";
        throw new Error(`Soroban transaction failed: ${failDetail}`);
      }

      if (txResult.status === rpc.Api.GetTransactionStatus.NOT_FOUND) {
        throw new Error(
          `Soroban transaction timed out waiting for confirmation (hash: ${sendResponse.hash})`,
        );
      }

      // Parse return value if present
      let returnValue: any = undefined;
      if ((txResult as any).returnValue) {
        try {
          returnValue = scValToNative((txResult as any).returnValue);
        } catch {
          returnValue = (txResult as any).returnValue;
        }
      }

      return {
        status: "SUCCESS",
        hash: sendResponse.hash,
        returnValue,
        raw: txResult,
      };
    } catch (err) {
      console.error(`invokeSorobanContract [${functionName}]`, err);
      throw err;
    }
  };

  const getContractClient = <
    T extends TypedContractClient = TypedContractClient,
  >(
    contractType: KnownContractType | string,
    contractId?: string,
  ): T => {
    const id = contractId || resolveContractId(contractType);
    if (!id) {
      throw new Error(
        `No contract ID provided or configured for type: ${contractType}. ` +
          `Please provide a contractId or set the environment variable.`,
      );
    }
    return createTypedContractClient(
      contractType,
      id,
      invokeSorobanContract,
    ) as T;
  };

  const invokeBoundContract = async (
    contractId: string,
    functionName: string,
    params: Record<string, unknown> = {},
  ) => {
    if (!address) throw new Error("No wallet connected");
    try {
      // Dispatch through the generated bindings (SC-91). Read calls are
      // simulated and resolved without a signature; write calls are signed
      // with Freighter and submitted.
      return await invokeGeneratedBinding({
        contractId,
        method: functionName,
        args: params,
        signer: { address, sign: signWithFreighter },
      });
    } catch (err) {
      console.error("invokeBoundContract", err);
      throw err;
    }
  };

  return (
    <AppContext.Provider
      value={{
        address,
        status,
        balance: undefined,
        connectWallet,
        disconnectWallet,
        sendPayment,
        sendXLM,
        signTransaction: signWithFreighter,
        invokeBoundContract,
        invokeSorobanContract,
        getContractClient,
      }}
    >
      {children}
    </AppContext.Provider>
  );
}

export const useAppContext = () => {
  const ctx = useContext(AppContext);
  if (!ctx) throw new Error("useAppContext must be used within AppProvider");
  return ctx;
};
