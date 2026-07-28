// "use client";
// import { createContext, useContext } from "react";
// import { Toast } from "primereact/toast";
// import { useRef } from "react";
// import { useRouter } from "next/navigation";
// // import {
// //   useConnect,
// //   useDisconnect,
// //   useAccount,
// //   useBalance,
// } from blockchain provider;
// // import type { Connector } from blockchain provider;
// import DotPulseLoader from "../components/ui/DotPulseLoader";
// import { STRK_TOKEN_ADDRESS } from "@/constants/tokens";

// interface AppContextType {
//   showToast: (
//     severity: "success" | "error" | "info",
//     summary: string,
//     detail: string
//   ) => void;
//   connectWallet: (connector: Connector) => Promise<void>;
//   disconnectWallet: () => Promise<void>;
//   address?: string;
//   status: string;
//   balance?: string | React.ReactNode;
//   contactAddress?: string;
// }

// const AppContext = createContext<AppContextType | undefined>(undefined);

// export function AppProvider({ children }: { children: React.ReactNode }) {
//   const router = useRouter();
//   const toast = useRef<Toast>(null);
//   const { connectAsync } = useConnect();
//   const { disconnectAsync } = useDisconnect();
//   const { address, status } = useAccount();
//   const { data, isLoading } = useBalance({
//     token: STRK_TOKEN_ADDRESS,
//     address: address as "0x",
//   });

//   const balance =
//     isLoading || !data ? (
//       <DotPulseLoader />
//     ) : (
//       `${parseFloat(data.formatted).toFixed(2)} STRK`
//     );

//   const showToast = (
//     severity: "success" | "error" | "info",
//     summary: string,
//     detail: string
//   ) => {
//     toast.current?.show({ severity, summary, detail });
//   };

//   const connectWallet = async (connector: Connector) => {
//     try {
//       await connectAsync({ connector });
//       localStorage.setItem("connector", connector.id);
//       showToast("success", "Success", "Wallet connected successfully");
//     } catch (error: unknown) {
//       localStorage.removeItem("connector");
//       let errorMessage = "Failed to connect wallet.";
//       if (error instanceof Error) {
//         if (error.message.includes("rejected")) {
//           errorMessage =
//             "Connection rejected. Please approve the connection request.";
//         } else if (error.message.includes("Connector not found")) {
//           errorMessage = `${connector.name} is not installed.`;
//         } else {
//           errorMessage = "Connection Failed";
//         }
//       }
//       showToast("error", "Connection Failed", errorMessage);
//     }
//   };

//   const disconnectWallet = async () => {
//     try {
//       await disconnectAsync();
//       localStorage.removeItem("connector");
//       showToast("success", "Success", "Wallet disconnected successfully");
//       setTimeout(() => {
//         router.push("/");
//       }, 1000);
//     } catch (error) {
//       console.log(error);
//       showToast("error", "Error", "Failed to disconnect wallet");
//     }
//   };

//   return (
//     <AppContext.Provider
//       value={{
//         showToast,
//         connectWallet,
//         disconnectWallet,

//         address,
//         status,
//         balance,
//       }}
//     >
//       <Toast ref={toast} />
//       {children}
//     </AppContext.Provider>
//   );
// }

// export const useAppContext = () => {
//   const context = useContext(AppContext);
//   if (!context) {
//     throw new Error("useAppContext must be used within an AppProvider");
//   }
//   return context;
// };

"use client";
import React, { createContext, useContext, useEffect, useState } from "react";
import { Server, TransactionBuilder, Networks, Operation, Asset } from "stellar-sdk";
import { Transaction } from "stellar-sdk";

const HORIZON_URL = process.env.NEXT_PUBLIC_HORIZON_URL || "https://horizon-testnet.stellar.org";
const SOROBAN_RPC = process.env.NEXT_PUBLIC_SOROBAN_RPC || "https://soroban-testnet.stellar.org:443";
const NETWORK_PASSPHRASE = process.env.NEXT_PUBLIC_NETWORK_PASSPHRASE || Networks.TESTNET;

type AppContextType = {
  address?: string;
  status: "connected" | "disconnected" | "connecting" | "error";
  balance?: string | React.ReactNode;
  connectWallet: () => Promise<void>;
  disconnectWallet: () => Promise<void>;
  sendXLM: (destination: string, amount: string | number) => Promise<any>;
  invokeSorobanContract: (contractId: string, functionName: string, args?: any[]) => Promise<any>;
};

const AppContext = createContext<AppContextType | undefined>(undefined);

export function AppProvider({ children }: { children: React.ReactNode }) {
  const [address, setAddress] = useState<string | undefined>(undefined);
  const [status, setStatus] = useState<AppContextType["status"]>("disconnected");
  const server = new Server(HORIZON_URL);

  useEffect(() => {
    // Auto-detect previously connected Freighter account
    const stored = localStorage.getItem("freighter_address");
    if (stored) setAddress(stored);
  }, []);

  const isFreighterAvailable = (): boolean => {
    // freighter v2 exposes `window.freighterApi`, older versions add `window.freighter`
    return typeof window !== "undefined" && (!!(window as any).freighterApi || !!(window as any).freighter);
  };

  const connectWallet = async () => {
    setStatus("connecting");
    try {
      if (!isFreighterAvailable()) throw new Error("Freighter not found. Please install Freighter.");

      // Prefer the freighter-api package if available on window
      const freighter = (window as any).freighterApi || (window as any).freighter;
      // try to get public key
      const publicKey = await (freighter.getPublicKey ? freighter.getPublicKey() : freighter.getPublicKey());
      if (!publicKey) throw new Error("Unable to read public key from Freighter");
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
        if (res && typeof res === "object" && res.signed_envelope_xdr) return res.signed_envelope_xdr;
        if (typeof res === "string") return res;
      }
      if (freighter.sign) {
        const res = await freighter.sign({ transaction: txXDR, network: NETWORK_PASSPHRASE });
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
    throw new Error("Freighter signing interface not supported in this environment");
  }

  const sendXLM = async (destination: string, amount: string | number) => {
    if (!address) throw new Error("No wallet connected");
    const acct = await server.loadAccount(address);
    const fee = await server.fetchBaseFee();
    const tx = new TransactionBuilder(acct, { fee: fee.toString(), networkPassphrase: NETWORK_PASSPHRASE })
      .addOperation(
        Operation.payment({
          destination,
          asset: Asset.native(),
          amount: String(amount),
        })
      )
      .setTimeout(30)
      .build();

    const txXDR = tx.toXDR();
    const signedEnvelopeXDR = await signWithFreighter(txXDR);

    // submit signed XDR
    try {
      const txObj = TransactionBuilder.fromXDR(signedEnvelopeXDR, NETWORK_PASSPHRASE);
      const res = await server.submitTransaction(txObj);
      return res;
    } catch (err) {
      // some horizon clients expect a TransactionEnvelope object; try submitting as-is
      try {
        // fallback: try submitting as-is (deprecated)
        const res = await server.submitTransaction(signedEnvelopeXDR as unknown as Transaction);
        return res;
      } catch (e) {
        console.error("submitTransaction failed", e);
        throw e;
      }
    }
  };

  const invokeSorobanContract = async (contractId: string, functionName: string, args: any[] = []) => {
    if (!address) throw new Error("No wallet connected");
    try {
      // Dynamic import to avoid build-time issues
      const { Contract, TimeoutInfinite } = await import("stellar-sdk");
      
      // Use the Soroban RPC
      const rpcServer = new Server(SOROBAN_RPC, { allowHttp: true });
      const acct = await rpcServer.getAccount(address);
      const contract = new Contract(contractId);

      // Build the initial transaction
      let tx = new TransactionBuilder(acct, {
        fee: "10000",
        networkPassphrase: NETWORK_PASSPHRASE,
      })
        .addOperation(contract.call(functionName, ...args))
        .setTimeout(TimeoutInfinite)
        .build();

      // Simulate transaction to get correct footprints & fee
      const simulatedTx = await rpcServer.simulateTransaction(tx);
      
      if ("error" in simulatedTx && simulatedTx.error) {
        throw new Error(`Simulation failed: ${simulatedTx.error}`);
      }

      // Assemble transaction with simulation data
      if (!simulatedTx.transactionData) {
        throw new Error("Simulation failed: missing transactionData");
      }

      tx = TransactionBuilder.assembleTransaction(tx, NETWORK_PASSPHRASE, simulatedTx as any).build();
      
      // Sign with Freighter
      const txXDR = tx.toXDR();
      const signedEnvelopeXDR = await signWithFreighter(txXDR);
      const signedTx = TransactionBuilder.fromXDR(signedEnvelopeXDR, NETWORK_PASSPHRASE);
      
      // Submit the transaction to Soroban
      const sendResponse = await rpcServer.sendTransaction(signedTx);
      
      if (sendResponse.status === "ERROR") {
        throw new Error(`Transaction failed: ${JSON.stringify(sendResponse.errorResult)}`);
      }
      
      return sendResponse;
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
