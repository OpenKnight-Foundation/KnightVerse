"use client";

import { useState, useCallback } from "react";
import { Button } from "@/components/ui/button";
import { useAuth } from "@/context/authContext";

interface PgnArchiveResult {
  game_id: string;
  ipfs_cid: string | null;
  arweave_tx_id: string | null;
  pgn_hash: string;
  archived_at: string;
}

interface PgnExportArchiveProps {
  gameId: string;
  pgnContent: string;
  onArchiveComplete?: (result: PgnArchiveResult) => void;
}

export function PgnExportArchive({ gameId, pgnContent, onArchiveComplete }: PgnExportArchiveProps) {
  const { accessToken, isAuthenticated } = useAuth();
  const [isExporting, setIsExporting] = useState(false);
  const [isArchiving, setIsArchiving] = useState(false);
  const [archiveResult, setArchiveResult] = useState<PgnArchiveResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";

  /**
   * Export PGN as a downloadable file
   */
  const exportPgn = useCallback(() => {
    try {
      setIsExporting(true);
      setError(null);

      // Create blob and download
      const blob = new Blob([pgnContent], { type: "application/x-chess-pgn" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `knightverse_game_${gameId}_${Date.now()}.pgn`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);

      console.log("[PGN Export] PGN exported successfully");
    } catch (err) {
      console.error("[PGN Export] Failed to export PGN:", err);
      setError("Failed to export PGN");
    } finally {
      setIsExporting(false);
    }
  }, [pgnContent, gameId]);

  /**
   * Copy PGN to clipboard
   */
  const copyPgnToClipboard = useCallback(async () => {
    try {
      setError(null);
      await navigator.clipboard.writeText(pgnContent);
      console.log("[PGN Export] PGN copied to clipboard");
    } catch (err) {
      console.error("[PGN Export] Failed to copy PGN:", err);
      setError("Failed to copy PGN to clipboard");
    }
  }, [pgnContent]);

  /**
   * Archive PGN to decentralized storage (IPFS/Arweave)
   */
  const archivePgn = useCallback(async () => {
    if (!isAuthenticated || !accessToken) {
      setError("You must be logged in to archive games");
      return;
    }

    try {
      setIsArchiving(true);
      setError(null);

      const response = await fetch(`${API_BASE}/v1/games/archive-pgn`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${accessToken}`,
        },
        body: JSON.stringify({
          game_id: gameId,
          pgn_content: pgnContent,
        }),
      });

      if (!response.ok) {
        const errorData = await response.json();
        throw new Error(errorData.message || "Failed to archive PGN");
      }

      const data = await response.json();
      setArchiveResult(data.archive_result);

      if (onArchiveComplete) {
        onArchiveComplete(data.archive_result);
      }

      console.log("[PGN Archive] PGN archived successfully:", data.archive_result);
    } catch (err) {
      console.error("[PGN Archive] Failed to archive PGN:", err);
      setError(err instanceof Error ? err.message : "Failed to archive PGN");
    } finally {
      setIsArchiving(false);
    }
  }, [gameId, pgnContent, accessToken, isAuthenticated, API_BASE, onArchiveComplete]);

  /**
   * View PGN on IPFS gateway
   */
  const viewOnIpfs = useCallback(() => {
    if (archiveResult?.ipfs_cid) {
      const ipfsGateway = process.env.NEXT_PUBLIC_IPFS_GATEWAY ?? "https://gateway.pinata.cloud";
      window.open(`${ipfsGateway}/ipfs/${archiveResult.ipfs_cid}`, "_blank");
    }
  }, [archiveResult]);

  /**
   * View PGN on Arweave explorer
   */
  const viewOnArweave = useCallback(() => {
    if (archiveResult?.arweave_tx_id) {
      window.open(`https://arweave.net/${archiveResult.arweave_tx_id}`, "_blank");
    }
  }, [archiveResult]);

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-2">
        <h3 className="text-lg font-semibold text-white">Export & Archive Game</h3>
        <p className="text-sm text-gray-400">
          Export your game as PGN or archive it to decentralized storage
        </p>
      </div>

      {/* Export Options */}
      <div className="space-y-2">
        <h4 className="text-sm font-medium text-gray-300">Export Options</h4>
        <div className="flex flex-wrap gap-2">
          <Button
            onClick={exportPgn}
            disabled={isExporting}
            variant="outline"
            className="bg-gray-800 hover:bg-gray-700 text-white"
          >
            {isExporting ? "Exporting..." : "Download PGN"}
          </Button>

          <Button
            onClick={copyPgnToClipboard}
            variant="outline"
            className="bg-gray-800 hover:bg-gray-700 text-white"
          >
            Copy to Clipboard
          </Button>
        </div>
      </div>

      {/* Archive Options */}
      {isAuthenticated && (
        <div className="space-y-2">
          <h4 className="text-sm font-medium text-gray-300">Decentralized Archive</h4>
          <Button
            onClick={archivePgn}
            disabled={isArchiving}
            className="bg-gradient-to-r from-teal-500 to-blue-700 hover:from-teal-600 hover:to-blue-800 text-white"
          >
            {isArchiving ? "Archiving..." : "Archive to IPFS & Arweave"}
          </Button>
        </div>
      )}

      {/* Archive Result */}
      {archiveResult && (
        <div className="p-4 bg-gray-800/50 border border-gray-700 rounded-lg space-y-3">
          <h4 className="text-sm font-medium text-emerald-400">✓ Game Archived Successfully</h4>

          <div className="space-y-2 text-xs">
            {archiveResult.ipfs_cid && (
              <div className="flex items-center justify-between">
                <span className="text-gray-400">IPFS CID:</span>
                <code className="text-teal-400 font-mono">{archiveResult.ipfs_cid}</code>
              </div>
            )}

            {archiveResult.arweave_tx_id && (
              <div className="flex items-center justify-between">
                <span className="text-gray-400">Arweave TX:</span>
                <code className="text-blue-400 font-mono">{archiveResult.arweave_tx_id}</code>
              </div>
            )}

            <div className="flex items-center justify-between">
              <span className="text-gray-400">PGN Hash:</span>
              <code className="text-gray-300 font-mono">{archiveResult.pgn_hash}</code>
            </div>

            <div className="flex items-center justify-between">
              <span className="text-gray-400">Archived At:</span>
              <span className="text-gray-300">
                {new Date(archiveResult.archived_at).toLocaleString()}
              </span>
            </div>
          </div>

          {/* View Links */}
          <div className="flex gap-2 pt-2">
            {archiveResult.ipfs_cid && (
              <Button
                onClick={viewOnIpfs}
                variant="outline"
                size="sm"
                className="text-xs"
              >
                View on IPFS
              </Button>
            )}

            {archiveResult.arweave_tx_id && (
              <Button
                onClick={viewOnArweave}
                variant="outline"
                size="sm"
                className="text-xs"
              >
                View on Arweave
              </Button>
            )}
          </div>
        </div>
      )}

      {/* Error Message */}
      {error && (
        <div className="p-3 bg-red-900/20 border border-red-800 rounded-lg">
          <p className="text-sm text-red-400">{error}</p>
        </div>
      )}
    </div>
  );
}
