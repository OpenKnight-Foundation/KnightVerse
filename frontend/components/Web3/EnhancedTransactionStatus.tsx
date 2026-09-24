"use client";

import React from "react";
import {
  type TransactionRecord,
  type TxPhase,
  type TxType,
} from "@/context/transactionContext";
import { useTransactionContext } from "@/context/transactionContext";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Progress } from "@/components/ui/progress";

/* ------------------------------------------------------------------ */
/*  Phase configuration                                                */
/* ------------------------------------------------------------------ */

const PHASE_CONFIG: Record<
  TxPhase,
  { icon: React.ReactNode; label: string; color: string; progress: number }
> = {
  preparing: {
    icon: (
      <svg className="h-4 w-4 animate-spin" viewBox="0 0 24 24">
        <circle
          className="opacity-25"
          cx="12"
          cy="12"
          r="10"
          stroke="currentColor"
          strokeWidth="4"
          fill="none"
        />
        <path
          className="opacity-75"
          fill="currentColor"
          d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
        />
      </svg>
    ),
    label: "Preparing",
    color: "text-blue-400",
    progress: 20,
  },
  signing: {
    icon: (
      <svg
        className="h-4 w-4 animate-pulse"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={2}
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z"
        />
      </svg>
    ),
    label: "Awaiting Signature",
    color: "text-yellow-400",
    progress: 40,
  },
  submitting: {
    icon: (
      <svg className="h-4 w-4 animate-spin" viewBox="0 0 24 24">
        <circle
          className="opacity-25"
          cx="12"
          cy="12"
          r="10"
          stroke="currentColor"
          strokeWidth="4"
          fill="none"
        />
        <path
          className="opacity-75"
          fill="currentColor"
          d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
        />
      </svg>
    ),
    label: "Submitting",
    color: "text-indigo-400",
    progress: 60,
  },
  confirming: {
    icon: (
      <svg
        className="h-4 w-4 animate-pulse"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={2}
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
        />
      </svg>
    ),
    label: "Confirming",
    color: "text-cyan-400",
    progress: 80,
  },
  confirmed: {
    icon: (
      <svg
        className="h-4 w-4"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={2}
      >
        <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
      </svg>
    ),
    label: "Confirmed",
    color: "text-emerald-400",
    progress: 100,
  },
  failed: {
    icon: (
      <svg
        className="h-4 w-4"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={2}
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          d="M6 18L18 6M6 6l12 12"
        />
      </svg>
    ),
    label: "Failed",
    color: "text-red-400",
    progress: 0,
  },
};

const TX_TYPE_CONFIG: Record<
  TxType,
  { icon: string; label: string; color: string }
> = {
  payment: { icon: "💸", label: "Payment", color: "text-blue-400" },
  contract: { icon: "📜", label: "Contract", color: "text-purple-400" },
  stake: { icon: "🔒", label: "Stake", color: "text-yellow-400" },
  claim: { icon: "🎁", label: "Claim", color: "text-emerald-400" },
};

/* ------------------------------------------------------------------ */
/*  Single transaction card                                            */
/* ------------------------------------------------------------------ */

function TransactionCard({
  tx,
  onDismiss,
}: {
  tx: TransactionRecord;
  onDismiss: (id: string) => void;
}) {
  const phaseCfg = PHASE_CONFIG[tx.phase];
  const typeCfg = TX_TYPE_CONFIG[tx.type];
  const isTerminal = tx.phase === "confirmed" || tx.phase === "failed";
  const elapsed = tx.resolvedAt
    ? ((tx.resolvedAt - tx.createdAt) / 1000).toFixed(1)
    : null;

  return (
    <Card className="p-4 animate-slide-up hover:scale-[1.01] transition-transform duration-200">
      <div className="space-y-3">
        {/* Header */}
        <div className="flex items-start justify-between gap-3">
          <div className="flex items-center gap-2 flex-1 min-w-0">
            <span className="text-lg" aria-hidden="true">
              {typeCfg.icon}
            </span>
            <div className="flex-1 min-w-0">
              <p className="text-sm font-semibold text-white truncate">
                {tx.label}
              </p>
              <div className="flex items-center gap-2 mt-0.5">
                <Badge variant="outline" className="text-[10px] px-1.5 py-0">
                  {typeCfg.label}
                </Badge>
                {tx.amount && (
                  <span className="text-xs text-gray-400">{tx.amount} XLM</span>
                )}
              </div>
            </div>
          </div>

          {isTerminal && (
            <button
              onClick={() => onDismiss(tx.id)}
              className="text-gray-500 hover:text-white transition-colors p-1 rounded hover:bg-gray-700/50"
              aria-label="Dismiss transaction"
            >
              <svg
                className="h-4 w-4"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth={2}
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M6 18L18 6M6 6l12 12"
                />
              </svg>
            </button>
          )}
        </div>

        {/* Progress bar for active transactions */}
        {!isTerminal && (
          <div className="space-y-1.5">
            <Progress value={phaseCfg.progress} max={100} />
            <div className="flex items-center justify-between">
              <span
                className={`flex items-center gap-1.5 ${phaseCfg.color} text-xs font-medium`}
              >
                {phaseCfg.icon}
                {phaseCfg.label}
              </span>
              <span className="text-xs text-gray-500">
                {phaseCfg.progress}%
              </span>
            </div>
          </div>
        )}

        {/* Terminal state badge */}
        {isTerminal && (
          <div className="flex items-center justify-between">
            <Badge
              variant={tx.phase === "confirmed" ? "success" : "destructive"}
              className="gap-1"
            >
              {phaseCfg.icon}
              {phaseCfg.label}
            </Badge>
            {elapsed && (
              <span className="text-xs text-gray-500">{elapsed}s</span>
            )}
          </div>
        )}

        {/* Hash link */}
        {tx.hash && (
          <div className="flex items-center gap-2 text-xs">
            <span className="text-gray-500">Hash:</span>
            <code className="flex-1 font-mono text-gray-400 truncate bg-gray-800/60 px-2 py-1 rounded">
              {tx.hash}
            </code>
            <button
              onClick={() => navigator.clipboard.writeText(tx.hash!)}
              className="text-gray-500 hover:text-teal-400 transition-colors"
              aria-label="Copy transaction hash"
            >
              <svg
                className="h-3.5 w-3.5"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth={2}
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
                />
              </svg>
            </button>
          </div>
        )}

        {/* Error message */}
        {tx.error && (
          <div className="text-xs text-red-400 bg-red-500/10 border border-red-500/20 rounded-lg p-2">
            {tx.error}
          </div>
        )}
      </div>
    </Card>
  );
}

/* ------------------------------------------------------------------ */
/*  Main component: EnhancedTransactionStatus                          */
/* ------------------------------------------------------------------ */

export function EnhancedTransactionStatus() {
  const { transactions, dismissTransaction, clearResolved, activeCount } =
    useTransactionContext();

  if (transactions.length === 0) return null;

  return (
    <div
      className="fixed bottom-4 left-4 z-[90] w-96 max-w-[calc(100vw-2rem)] space-y-2"
      role="region"
      aria-label="Transaction status"
    >
      {/* Active counter header */}
      {activeCount > 0 && (
        <Card className="p-3 bg-gradient-to-r from-blue-500/10 to-indigo-500/10 border-blue-500/30">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <span
                className="w-2 h-2 rounded-full bg-blue-400 animate-pulse-glow"
                aria-hidden="true"
              />
              <span className="text-sm text-blue-400 font-semibold">
                {activeCount} Active Transaction{activeCount !== 1 ? "s" : ""}
              </span>
            </div>
            <Badge variant="info" className="text-[10px]">
              In Progress
            </Badge>
          </div>
        </Card>
      )}

      {/* Transaction cards */}
      <div className="max-h-[500px] overflow-y-auto space-y-2 custom-scrollbar">
        {transactions.slice(0, 5).map((tx) => (
          <TransactionCard
            key={tx.id}
            tx={tx}
            onDismiss={dismissTransaction}
          />
        ))}
      </div>

      {/* Clear resolved button */}
      {transactions.some(
        (t) => t.phase === "confirmed" || t.phase === "failed"
      ) && (
        <button
          onClick={clearResolved}
          className="w-full text-xs text-gray-500 hover:text-gray-300 transition-colors py-2 px-3 rounded-lg hover:bg-gray-800/40 border border-transparent hover:border-gray-700/30"
          aria-label="Clear completed transactions"
        >
          Clear Completed Transactions
        </button>
      )}
    </div>
  );
}
