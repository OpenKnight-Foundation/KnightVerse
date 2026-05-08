"use client";

import React, { useCallback, useEffect, useState } from "react";
import dynamic from "next/dynamic";
import { FaPlus, FaSpinner } from "react-icons/fa";
import type { TournamentBracket, BracketFormat } from "@/components/tournament/BracketView";

const BracketView = dynamic(() => import("@/components/tournament/BracketView"), {
  ssr: false,
});

const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8000";

const FORMAT_OPTIONS: { value: BracketFormat; label: string; description: string }[] = [
  { value: "SingleElimination", label: "Single Elimination", description: "One loss and you're out" },
  { value: "DoubleElimination", label: "Double Elimination", description: "Two losses to be eliminated" },
  { value: "RoundRobin", label: "Round Robin", description: "Everyone plays everyone" },
];

export default function TournamentPage() {
  const [brackets, setBrackets] = useState<TournamentBracket[]>([]);
  const [selected, setSelected] = useState<TournamentBracket | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);

  // Create form state
  const [name, setName] = useState("");
  const [format, setFormat] = useState<BracketFormat>("SingleElimination");
  const [creating, setCreating] = useState(false);

  const fetchBrackets = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch(`${API_BASE}/v1/tournaments`, { credentials: "include" });
      if (!res.ok) throw new Error(`Server error: ${res.status}`);
      const data: TournamentBracket[] = await res.json();
      setBrackets(data);
      if (data.length > 0 && !selected) setSelected(data[0]);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load tournaments");
    } finally {
      setLoading(false);
    }
  }, [selected]);

  useEffect(() => {
    void fetchBrackets();
  }, [fetchBrackets]);

  async function handleCreate(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim()) return;
    setCreating(true);
    try {
      const res = await fetch(`${API_BASE}/v1/tournaments`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({ name: name.trim(), format }),
      });
      if (!res.ok) throw new Error(`Failed to create tournament: ${res.status}`);
      const created: TournamentBracket = await res.json();
      setBrackets((prev) => [created, ...prev]);
      setSelected(created);
      setShowCreate(false);
      setName("");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to create tournament");
    } finally {
      setCreating(false);
    }
  }

  return (
    <div className="min-h-screen p-4 md:p-8 text-white" role="main" aria-label="Tournaments">
      <div className="max-w-7xl mx-auto">
        {/* Page header */}
        <div className="flex items-center justify-between mb-8">
          <div>
            <h1 className="text-3xl font-extrabold bg-gradient-to-r from-teal-400 to-blue-500 bg-clip-text text-transparent">
              Tournaments
            </h1>
            <p className="text-gray-400 text-sm mt-1">Manage brackets and track results</p>
          </div>
          <button
            onClick={() => setShowCreate(!showCreate)}
            className="flex items-center gap-2 px-4 py-2 rounded-xl bg-teal-500/10 border border-teal-500/30 hover:bg-teal-500/20 hover:border-teal-400/50 text-teal-300 font-semibold text-sm transition-all"
          >
            <FaPlus />
            New Tournament
          </button>
        </div>

        {/* Create form */}
        {showCreate && (
          <form
            onSubmit={handleCreate}
            className="mb-8 p-6 rounded-2xl bg-gray-900/80 border border-teal-500/20"
            aria-label="Create tournament"
          >
            <h2 className="text-lg font-bold mb-4">Create Tournament</h2>
            <div className="flex flex-col sm:flex-row gap-4">
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Tournament name"
                required
                className="flex-1 px-4 py-2 rounded-xl bg-gray-800 border border-gray-700 text-white placeholder-gray-500 focus:outline-none focus:border-teal-500 text-sm"
              />
              <select
                value={format}
                onChange={(e) => setFormat(e.target.value as BracketFormat)}
                className="px-4 py-2 rounded-xl bg-gray-800 border border-gray-700 text-white focus:outline-none focus:border-teal-500 text-sm"
              >
                {FORMAT_OPTIONS.map((o) => (
                  <option key={o.value} value={o.value}>
                    {o.label}
                  </option>
                ))}
              </select>
              <button
                type="submit"
                disabled={creating || !name.trim()}
                className="px-6 py-2 rounded-xl bg-teal-500 hover:bg-teal-400 disabled:opacity-50 disabled:cursor-not-allowed text-black font-bold text-sm transition-all"
              >
                {creating ? <FaSpinner className="animate-spin" /> : "Create"}
              </button>
            </div>
            <p className="text-xs text-gray-500 mt-2">
              {FORMAT_OPTIONS.find((o) => o.value === format)?.description}
            </p>
          </form>
        )}

        {/* Error */}
        {error && (
          <div className="mb-6 p-4 rounded-xl bg-red-500/10 border border-red-500/30 text-red-300 text-sm">
            {error}
          </div>
        )}

        {/* Loading */}
        {loading && (
          <div className="flex justify-center py-20">
            <FaSpinner className="animate-spin text-3xl text-teal-400" />
          </div>
        )}

        {/* Content */}
        {!loading && (
          <div className="flex flex-col lg:flex-row gap-6">
            {/* Sidebar: tournament list */}
            <aside className="lg:w-64 flex-shrink-0">
              <h2 className="text-xs text-gray-400 uppercase tracking-widest mb-3 font-semibold">
                Tournaments ({brackets.length})
              </h2>
              {brackets.length === 0 ? (
                <p className="text-gray-600 text-sm">No tournaments yet. Create one above.</p>
              ) : (
                <ul className="flex flex-col gap-2">
                  {brackets.map((b) => (
                    <li key={b.id}>
                      <button
                        onClick={() => setSelected(b)}
                        className={`w-full text-left px-4 py-3 rounded-xl border text-sm transition-all ${
                          selected?.id === b.id
                            ? "border-teal-500/50 bg-teal-500/10 text-teal-300"
                            : "border-gray-700/50 bg-gray-800/30 text-gray-300 hover:border-gray-600"
                        }`}
                      >
                        <p className="font-semibold truncate">{b.name}</p>
                        <p className="text-xs text-gray-500 mt-0.5">
                          {b.status} · {b.participants.length} players
                        </p>
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </aside>

            {/* Main: bracket view */}
            <main className="flex-1 min-w-0 bg-gray-900/60 border border-gray-700/40 rounded-2xl p-6">
              {selected ? (
                <BracketView bracket={selected} />
              ) : (
                <div className="flex flex-col items-center justify-center py-20 text-gray-600">
                  <p>Select or create a tournament to view its bracket.</p>
                </div>
              )}
            </main>
          </div>
        )}
      </div>
    </div>
  );
}
