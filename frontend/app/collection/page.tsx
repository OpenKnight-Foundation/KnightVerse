"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { StrKey } from "@stellar/stellar-sdk";
import { GameSidebar } from "@/components/GameSidebar";
import { Header } from "@/components/Header";
import { Button } from "@/components/ui/button";
import { useToast } from "@/components/ui/toast";
import {
  fetchOwnedAiNfts,
  transferOwnedAiNft,
  type AiNftCollectionItem,
} from "@/contracts/client";
import type { ContractMethodInvoker } from "@/contracts";
import { useAppContext } from "@/context/walletContext";
import { useTrackedTransaction } from "@/hook/useTrackedTransaction";
import { CONTRACT_AI_NFT } from "@/lib/api";
import { truncateAddress } from "@/lib/utils";

function shortHash(hash: string): string {
  if (!hash.startsWith("0x")) return truncateAddress(hash);
  if (hash.length <= 14) return hash;
  return `${hash.slice(0, 10)}…${hash.slice(-6)}`;
}

export default function CollectionPage() {
  const { address, status, connectWallet, invokeSorobanContract } =
    useAppContext();
  const { addToast } = useToast();
  const [collapsed, setCollapsed] = useState(false);
  const [items, setItems] = useState<AiNftCollectionItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [recipientByToken, setRecipientByToken] = useState<Record<string, string>>({});
  const [pendingTransfer, setPendingTransfer] =
    useState<AiNftCollectionItem | null>(null);
  const [transferringId, setTransferringId] = useState<string | null>(null);
  const invokeRef = useRef(invokeSorobanContract);
  const loadIdRef = useRef(0);

  useEffect(() => {
    invokeRef.current = invokeSorobanContract;
  }, [invokeSorobanContract]);

  const { execute } = useTrackedTransaction({
    type: "contract",
    label: "Transfer AI NFT",
    destination: pendingTransfer?.tokenId.toString(),
  });

  const load = useCallback(
    async (forceRefresh = false) => {
      const loadId = ++loadIdRef.current;
      if (!address || !CONTRACT_AI_NFT) {
        setItems([]);
        setLoading(false);
        return;
      }

      setLoading(true);
      setError(null);
      const invoke: ContractMethodInvoker = (method, args = [], options = {}) =>
        invokeRef.current(CONTRACT_AI_NFT, method, args, options);

      try {
        const result = await fetchOwnedAiNfts({
          owner: address,
          invoke,
          forceRefresh,
        });
        if (loadId === loadIdRef.current) setItems(result);
      } catch (err) {
        if (loadId === loadIdRef.current) {
          setError(err instanceof Error ? err.message : "Could not load NFTs.");
        }
      } finally {
        if (loadId === loadIdRef.current) setLoading(false);
      }
    },
    [address],
  );

  useEffect(() => {
    void load();
  }, [load]);

  const requestTransfer = (item: AiNftCollectionItem) => {
    const recipient = (recipientByToken[item.tokenId.toString()] ?? "").trim();
    if (!StrKey.isValidEd25519PublicKey(recipient)) {
      addToast({
        severity: "error",
        title: "Invalid recipient",
        detail: "Enter a valid Stellar public address (G…).",
      });
      return;
    }
    setPendingTransfer(item);
  };

  const confirmTransfer = async () => {
    if (!pendingTransfer || !address) return;
    const tokenId = pendingTransfer.tokenId;
    const recipient = (
      recipientByToken[tokenId.toString()] ?? ""
    ).trim();
    setTransferringId(tokenId.toString());

    const result = await execute(async () => {
      const response = await transferOwnedAiNft({
        tokenId,
        to: recipient,
        owner: address,
        invoke: (method, args = [], options = {}) =>
          invokeRef.current(CONTRACT_AI_NFT, method, args, options),
      });
      return { hash: (response as { hash?: string } | undefined)?.hash };
    });

    if (result !== undefined) {
      setPendingTransfer(null);
      setRecipientByToken((prev) => {
        const next = { ...prev };
        delete next[tokenId.toString()];
        return next;
      });
      void load(true);
    }
    setTransferringId(null);
  };

  return (
    <div className="flex h-screen overflow-hidden bg-background">
      <GameSidebar collapsed={collapsed} setCollapsed={setCollapsed} />
      <div
        className={`flex flex-1 flex-col transition-all duration-500 ease-in-out ${
          collapsed ? "md:ml-16" : "md:ml-64"
        }`}
      >
        <Header />
        <main className="flex-1 overflow-y-auto p-4 md:p-8">
          <div className="mx-auto max-w-6xl space-y-6">
            <header className="flex flex-wrap items-start justify-between gap-4">
              <div>
                <h1 className="text-3xl font-bold text-white">AI NFT Collection</h1>
                <p className="mt-1 text-sm text-gray-400">
                  {address
                    ? `Wallet ${truncateAddress(address)}`
                    : "Connect a wallet to view your AI companions."}
                </p>
              </div>
              <div className="flex items-center gap-2">
                <Button
                  variant="outline"
                  onClick={() => void load(true)}
                  disabled={!address || loading}
                >
                  {loading ? "Refreshing…" : "Refresh"}
                </Button>
                {status !== "connected" && (
                  <Button onClick={() => void connectWallet()}>
                    Connect Wallet
                  </Button>
                )}
              </div>
            </header>

            {!CONTRACT_AI_NFT && (
              <div className="rounded-xl border border-amber-500/30 bg-amber-500/10 p-4 text-sm text-amber-300">
                The AI NFT contract is not configured. Set
                NEXT_PUBLIC_CONTRACT_AI_NFT to enable on-chain collection data.
              </div>
            )}

            {error && (
              <div role="alert" className="rounded-xl border border-red-500/30 bg-red-500/10 p-4 text-sm text-red-300">
                {error}
              </div>
            )}

            {loading ? (
              <div role="status" aria-label="Loading AI NFT collection" className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
                {Array.from({ length: 6 }).map((_, index) => (
                  <div
                    key={index}
                    className="h-64 animate-pulse rounded-2xl border border-gray-700/40 bg-gray-800/40"
                  />
                ))}
              </div>
            ) : items.length === 0 ? (
              <div className="rounded-2xl border border-dashed border-gray-700 bg-gray-900/40 px-6 py-16 text-center">
                <h2 className="text-lg font-semibold text-white">
                  No AI NFTs yet
                </h2>
                <p className="mt-2 text-sm text-gray-400">
                  AI companions minted to this wallet will appear here.
                </p>
              </div>
            ) : (
              <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
                {items.map((item) => {
                  const tokenId = item.tokenId.toString();
                  const recipient = recipientByToken[tokenId] ?? "";
                  return (
                    <article
                      key={tokenId}
                      className="flex flex-col gap-4 rounded-2xl border border-gray-700/50 bg-gray-900/60 p-5 shadow-lg shadow-black/10"
                    >
                      <div className="flex items-start justify-between gap-3">
                        <div>
                          <h2 className="text-lg font-semibold text-white">
                            {item.displayName}
                          </h2>
                          <p className="text-xs text-gray-500">AI NFT #{item.nftId.toString()}</p>
                        </div>
                        <span className="rounded-md border border-teal-500/40 bg-teal-500/10 px-2 py-1 text-xs font-medium text-teal-300">
                          Owned
                        </span>
                      </div>

                      <dl className="space-y-2 text-xs text-gray-400">
                        <div className="flex justify-between gap-3">
                          <dt>Owner</dt>
                          <dd className="font-mono text-gray-300">
                            {truncateAddress(item.owner)}
                          </dd>
                        </div>
                        <div className="flex justify-between gap-3">
                          <dt>Minter</dt>
                          <dd className="font-mono text-gray-300">
                            {truncateAddress(item.minter)}
                          </dd>
                        </div>
                        <div className="flex justify-between gap-3">
                          <dt>Metadata</dt>
                          <dd className="font-mono text-gray-300" title={item.metadataHash}>
                            {shortHash(item.metadataHash)}
                          </dd>
                        </div>
                        <div className="flex justify-between gap-3">
                          <dt>Minted at ledger</dt>
                          <dd className="font-mono text-gray-300">
                            {item.createdAt.toString()}
                          </dd>
                        </div>
                      </dl>

                      <div>
                        <p className="text-xs uppercase tracking-[0.16em] text-gray-500">
                          Personality
                        </p>
                        <p className="mt-1 whitespace-pre-wrap break-words text-sm text-gray-300">
                          {item.personalityTraits || "No traits recorded"}
                        </p>
                      </div>

                      <div className="mt-auto space-y-2">
                        <label
                          htmlFor={`recipient-${tokenId}`}
                          className="text-xs uppercase tracking-[0.16em] text-gray-500"
                        >
                          Transfer to
                        </label>
                        <input
                          id={`recipient-${tokenId}`}
                          value={recipient}
                          onChange={(event) =>
                            setRecipientByToken((prev) => ({
                              ...prev,
                              [tokenId]: event.target.value,
                            }))
                          }
                          placeholder="G…"
                          className="w-full rounded-lg border border-gray-700 bg-gray-950/60 px-3 py-2 font-mono text-xs text-white placeholder:text-gray-600"
                        />
                        <Button
                          variant="outline"
                          className="w-full"
                          onClick={() => requestTransfer(item)}
                          disabled={!recipient.trim() || !address || status !== "connected"}
                        >
                          Transfer
                        </Button>
                      </div>
                    </article>
                  );
                })}
              </div>
            )}
          </div>
        </main>
      </div>

      {pendingTransfer && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
          <div
            role="alertdialog"
            aria-modal="true"
            aria-labelledby="transfer-title"
            aria-describedby="transfer-description"
            className="w-full max-w-md rounded-2xl border border-gray-700 bg-gray-900 p-6 shadow-2xl"
          >
            <h2 id="transfer-title" className="text-lg font-semibold text-white">
              Confirm transfer
            </h2>
            <p id="transfer-description" className="mt-2 text-sm text-gray-400">
              Transfer {pendingTransfer.displayName} (AI NFT #
              {pendingTransfer.nftId.toString()}) to{" "}
              <span className="break-all font-mono text-gray-200">
                {recipientByToken[pendingTransfer.tokenId.toString()] ?? ""}
              </span>
              ?
            </p>
            <div className="mt-6 flex justify-end gap-2">
              <Button
                variant="outline"
                onClick={() => setPendingTransfer(null)}
                disabled={transferringId !== null}
              >
                Cancel
              </Button>
              <Button
                onClick={() => void confirmTransfer()}
                disabled={transferringId !== null}
              >
                {transferringId === pendingTransfer.tokenId.toString()
                  ? "Transferring…"
                  : "Confirm"}
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
