"use client";
import React, { createContext, useContext, useEffect, useState } from "react";
import { Server, TransactionBuilder, Operation, Asset } from "stellar-sdk";

import { HORIZON_URL, SOROBAN_RPC, NETWORK_PASSPHRASE } from "@/lib/api";

type AppContextType = {
  address?: string;
  status: "connected" | "disconnected" | "connecting" | "error";
  balance?: string | React.ReactNode;
  connectWallet: () => Promise<void>;
  disconnectWallet: () => Promise<void>;
  sendXLM: (destination: string, amount: string | number) => Promise<any>;
  invokeSorobanContract: (
    contractId: string,
    functionName: string,
    args?: any[],
  ) => Promise<any>;
};

const AppContext = createContext<AppContextType | undefined>(undefined);

export function AppProvider({ children }: { children: React.ReactNode }) {
  const [address, setAddress] = useState<string | undefined>(undefined);
  const [status, setStatus] =
    useState<AppContextType["status"]>("disconnected");
  const server = new Server(HORIZON_URL);

  useEffect(() => {
    // Auto-detect previously connected Freighter account
    const stored = localStorage.getItem("freighter_address");
    if (stored) {
      setAddress(stored);
      setStatus("connected");
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
      localStorage.setItem("freighter_address", publicKey);
      setStatus("connected");
    } catch (err) {
      console.error("connectWallet", err);
      setStatus("error");
      throw err;
    }
  };

  const disconnectWallet = async () => {
    setAddress(undefined);
    localStorage.removeItem("freighter_address");
    setStatus("disconnected");
  };

  async function signWithFreighter(txXDR: string): Promise<string> {
    const freighter = (window as any).freighterApi || (window as any).freighter;
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

  const sendXLM = async (destination: string, amount: string | number) => {
    if (!address) throw new Error("No wallet connected");
    const acct = await server.loadAccount(address);
    const fee = await server.fetchBaseFee();
    const tx = new TransactionBuilder(acct, {
      fee: fee.toString(),
      networkPassphrase: NETWORK_PASSPHRASE,
    })
      .addOperation(
        Operation.payment({
          destination,
          asset: Asset.native(),
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

  const invokeSorobanContract = async (
    contractId: string,
    functionName: string,
    args: any[] = [],
  ) => {
    if (!address) throw new Error("No wallet connected");
    try {
      // dynamic import to avoid build-time hard dependency when not used
      const sc = await import("soroban-client");
      const SorobanClient = sc.default;
      // Modern Soroban SDK usage
      const rpc = new SorobanClient(SOROBAN_RPC);
      // TODO: Implement full hostfunction builder for specific contract/ABI
      // Example: await rpc.getContractData(contractId);
      // TODO: Implement full hostfunction builder for specific contract/ABI
      throw new Error(
        "invokeSorobanContract: implement contract-specific invocation (ABI required)",
      );
    } catch (err) {
      console.error("invokeSorobanContract", err);
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
        sendXLM,
        invokeSorobanContract,
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
