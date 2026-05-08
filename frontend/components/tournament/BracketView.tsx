"use client";

import React from "react";
import { FaTrophy } from "react-icons/fa";

export type MatchStatus = "Pending" | "InProgress" | "Completed";
export type TournamentStatus = "Registration" | "InProgress" | "Completed";
export type BracketFormat = "SingleElimination" | "DoubleElimination" | "RoundRobin";

export interface Participant {
  id: string;
  wallet_address: string;
  display_name: string;
  elo: number;
  seed: number;
}

export interface BracketMatch {
  id: string;
  round: number;
  match_number: number;
  player1_id: string | null;
  player2_id: string | null;
  winner_id: string | null;
  status: MatchStatus;
  scheduled_at: string | null;
  completed_at: string | null;
}

export interface TournamentBracket {
  id: string;
  name: string;
  format: BracketFormat;
  status: TournamentStatus;
  participants: Participant[];
  matches: BracketMatch[];
  total_rounds: number;
  winner_id: string | null;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
}

interface BracketViewProps {
  bracket: TournamentBracket;
}

function getParticipant(
  bracket: TournamentBracket,
  id: string | null
): Participant | undefined {
  if (!id) return undefined;
  return bracket.participants.find((p) => p.id === id);
}

function MatchCard({
  match,
  bracket,
}: {
  match: BracketMatch;
  bracket: TournamentBracket;
}) {
  const p1 = getParticipant(bracket, match.player1_id);
  const p2 = getParticipant(bracket, match.player2_id);

  const rowClass = (playerId: string | null) => {
    if (!playerId) return "text-gray-600 italic";
    if (match.winner_id === playerId) return "text-teal-300 font-bold";
    if (match.status === "Completed") return "text-gray-500 line-through";
    return "text-white";
  };

  return (
    <div
      className={`rounded-xl border p-3 w-44 text-xs ${
        match.status === "Completed"
          ? "border-teal-500/40 bg-teal-900/10"
          : match.status === "InProgress"
          ? "border-yellow-500/40 bg-yellow-900/10 animate-pulse"
          : "border-gray-700/50 bg-gray-800/40"
      }`}
    >
      <div className={`py-1 border-b border-gray-700/40 truncate ${rowClass(match.player1_id)}`}>
        {p1 ? `#${p1.seed} ${p1.display_name}` : "TBD"}
      </div>
      <div className={`py-1 truncate ${rowClass(match.player2_id)}`}>
        {p2 ? `#${p2.seed} ${p2.display_name}` : "TBD"}
      </div>
    </div>
  );
}

function RoundColumn({
  round,
  matches,
  bracket,
  totalRounds,
}: {
  round: number;
  matches: BracketMatch[];
  bracket: TournamentBracket;
  totalRounds: number;
}) {
  const label =
    round === totalRounds
      ? "Final"
      : round === totalRounds - 1
      ? "Semi-finals"
      : `Round ${round}`;

  return (
    <div className="flex flex-col gap-2 items-center">
      <span className="text-xs font-semibold text-gray-400 uppercase tracking-widest mb-2">
        {label}
      </span>
      <div
        className="flex flex-col"
        style={{ gap: `${Math.pow(2, round - 1) * 8}px` }}
      >
        {matches
          .sort((a, b) => a.match_number - b.match_number)
          .map((m) => (
            <MatchCard key={m.id} match={m} bracket={bracket} />
          ))}
      </div>
    </div>
  );
}

function StandingsTable({ bracket }: { bracket: TournamentBracket }) {
  const wins: Record<string, number> = {};
  for (const m of bracket.matches) {
    if (m.winner_id) wins[m.winner_id] = (wins[m.winner_id] ?? 0) + 1;
  }

  const rows = bracket.participants
    .map((p) => ({ participant: p, wins: wins[p.id] ?? 0 }))
    .sort((a, b) => b.wins - a.wins);

  return (
    <table className="w-full text-sm text-left">
      <thead>
        <tr className="text-gray-400 text-xs uppercase tracking-wider border-b border-gray-700">
          <th className="py-2 pr-4">Rank</th>
          <th className="py-2 pr-4">Player</th>
          <th className="py-2 pr-4">ELO</th>
          <th className="py-2">Wins</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((r, i) => (
          <tr
            key={r.participant.id}
            className={`border-b border-gray-800 ${
              bracket.winner_id === r.participant.id
                ? "text-teal-300"
                : "text-gray-300"
            }`}
          >
            <td className="py-2 pr-4 font-mono">
              {bracket.winner_id === r.participant.id ? (
                <FaTrophy className="inline text-yellow-400 mr-1" />
              ) : null}
              {i + 1}
            </td>
            <td className="py-2 pr-4 font-semibold">{r.participant.display_name}</td>
            <td className="py-2 pr-4 text-gray-400">{r.participant.elo}</td>
            <td className="py-2 font-mono">{r.wins}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

export default function BracketView({ bracket }: BracketViewProps) {
  const rounds = Array.from(
    new Set(bracket.matches.map((m) => m.round))
  ).sort((a, b) => a - b);

  const isElimination =
    bracket.format === "SingleElimination" ||
    bracket.format === "DoubleElimination";

  const statusBadge: Record<TournamentStatus, string> = {
    Registration: "bg-blue-500/20 text-blue-300 border-blue-500/30",
    InProgress: "bg-yellow-500/20 text-yellow-300 border-yellow-500/30",
    Completed: "bg-teal-500/20 text-teal-300 border-teal-500/30",
  };

  return (
    <div className="text-white">
      {/* Header */}
      <div className="flex flex-wrap items-center gap-3 mb-6">
        <h2 className="text-2xl font-extrabold tracking-tight bg-gradient-to-r from-teal-400 to-blue-500 bg-clip-text text-transparent">
          {bracket.name}
        </h2>
        <span
          className={`text-xs px-3 py-1 rounded-full border font-semibold ${statusBadge[bracket.status]}`}
        >
          {bracket.status}
        </span>
        <span className="text-xs text-gray-500">{bracket.format.replace(/([A-Z])/g, " $1").trim()}</span>
      </div>

      {/* Champion banner */}
      {bracket.status === "Completed" && bracket.winner_id && (
        <div className="mb-6 p-4 rounded-2xl bg-gradient-to-r from-yellow-500/10 to-teal-500/10 border border-yellow-500/30 flex items-center gap-3">
          <FaTrophy className="text-2xl text-yellow-400" />
          <div>
            <p className="text-xs text-gray-400 uppercase tracking-widest">Champion</p>
            <p className="text-lg font-bold text-yellow-300">
              {getParticipant(bracket, bracket.winner_id)?.display_name ?? "Unknown"}
            </p>
          </div>
        </div>
      )}

      {/* Bracket grid (elimination) or standings (round-robin) */}
      {isElimination ? (
        <div className="overflow-x-auto pb-4">
          <div className="flex gap-8 min-w-max">
            {rounds.map((round) => (
              <RoundColumn
                key={round}
                round={round}
                matches={bracket.matches.filter((m) => m.round === round)}
                bracket={bracket}
                totalRounds={bracket.total_rounds}
              />
            ))}
          </div>
        </div>
      ) : (
        <StandingsTable bracket={bracket} />
      )}

      {/* Participants */}
      <div className="mt-8">
        <h3 className="text-sm font-semibold text-gray-400 uppercase tracking-widest mb-3">
          Participants ({bracket.participants.length})
        </h3>
        <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-2">
          {bracket.participants.map((p) => (
            <div
              key={p.id}
              className="flex items-center gap-2 bg-gray-800/50 border border-gray-700/40 rounded-lg px-3 py-2 text-xs"
            >
              <span className="text-gray-500 font-mono w-5">#{p.seed}</span>
              <span className="text-white truncate font-medium">{p.display_name}</span>
              <span className="ml-auto text-gray-400">{p.elo}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
