"use client";

import { useParams } from "next/navigation";
import { useState, useEffect } from "react";
import { useAuth } from "@/context/authContext";

interface GuildMember {
  name: string;
  rating: number;
  role: "Captain" | "Officer" | "Member";
  joinedAt: string;
}

interface GuildData {
  id: string;
  name: string;
  captainName: string;
  winRate: number;
  rank: string;
  treasuryXLM: number;
  members: GuildMember[];
}

const DEFAULT_MEMBERS: GuildMember[] = [
  { name: "KnightRider", rating: 2100, role: "Captain", joinedAt: "2025-01-10" },
  { name: "PawnStorm", rating: 1850, role: "Officer", joinedAt: "2025-02-04" },
];

export default function GuildProfilePage() {
  const params = useParams<{ id: string }>();
  const { user } = useAuth();
  const guildId = params?.id || "1";

  const [guild, setGuild] = useState<GuildData>({
    id: guildId,
    name: `Guild #${guildId}`,
    captainName: DEFAULT_MEMBERS[0].name,
    winRate: 68,
    rank: "#14",
    treasuryXLM: 2500,
    members: DEFAULT_MEMBERS,
  });

  const [joinStatus, setJoinStatus] = useState<string | null>(null);
  const [manageOpen, setManageOpen] = useState(false);

  useEffect(() => {
    async function loadGuild() {
      try {
        const apiBase =
          process.env.NEXT_PUBLIC_API_URL ||
          process.env.NEXT_PUBLIC_BACKEND_URL ||
          "http://localhost:8000";
        const res = await fetch(`${apiBase}/v1/guilds/${encodeURIComponent(guildId)}`, {
          headers: { Accept: "application/json" },
        });
        if (res.ok) {
          const json = await res.json();
          const g = json?.data?.guild || json?.guild || json;
          if (g) {
            setGuild((prev) => ({
              ...prev,
              name: g.name || prev.name,
              captainName: g.captain_name || g.captainName || prev.captainName,
              winRate: g.win_rate ?? g.winRate ?? prev.winRate,
              rank: g.rank ? `#${g.rank}` : prev.rank,
              treasuryXLM: g.treasury_xlm ?? g.treasuryXLM ?? prev.treasuryXLM,
              members: Array.isArray(g.members) && g.members.length > 0 ? g.members : prev.members,
            }));
          }
        }
      } catch {
        // Retain preview sandbox state on network or endpoint absence
      }
    }
    loadGuild();
  }, [guildId]);

  // Derive captain privileges from authenticated user matching guild captain
  const isCaptain = Boolean(
    user?.username &&
    guild.captainName &&
    user.username.trim().toLowerCase() === guild.captainName.trim().toLowerCase()
  );

  const handleJoin = () => {
    if (!user) {
      setJoinStatus("Please connect your wallet or sign in to request guild membership.");
      return;
    }
    setJoinStatus(`Membership request for Guild #${guildId} submitted! Awaiting captain review.`);
  };

  return (
    <div className="mx-auto max-w-4xl space-y-6 p-4">
      {/* Preview Mode Banner */}
      <div className="p-4 rounded-xl border border-amber-500/30 bg-amber-500/10 text-amber-300 text-sm flex items-center justify-between">
        <div className="flex items-center gap-3">
          <span className="px-2 py-0.5 rounded text-xs font-bold uppercase bg-amber-500/20 border border-amber-500/40">
            Preview Mode
          </span>
          <span>
            Guild profiles and on-chain rosters are in preview. Live guild registration will unlock on Soroban mainnet.
          </span>
        </div>
      </div>

      {joinStatus && (
        <div className="p-3 rounded-lg border border-indigo-500/30 bg-indigo-500/10 text-indigo-300 text-xs flex justify-between items-center">
          <span>{joinStatus}</span>
          <button
            onClick={() => setJoinStatus(null)}
            className="text-xs text-indigo-200 hover:text-white underline ml-4"
          >
            Dismiss
          </button>
        </div>
      )}

      <header className="rounded-lg border border-slate-800 bg-slate-900/50 p-4">
        <h1 className="text-xl font-bold">{guild.name}</h1>
        <p className="text-sm text-muted-foreground">
          Captain: <span className="font-semibold text-slate-200">{guild.captainName}</span>
          {isCaptain && (
            <span className="ml-2 inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-emerald-500/20 text-emerald-300 border border-emerald-500/30">
              You are Captain
            </span>
          )}
        </p>
      </header>

      <section className="grid grid-cols-3 gap-4 text-center">
        <div className="rounded-lg border border-slate-800 bg-slate-900/50 p-3">
          <p className="text-xs text-muted-foreground">Win Rate</p>
          <p className="text-lg font-semibold">{guild.winRate}%</p>
        </div>
        <div className="rounded-lg border border-slate-800 bg-slate-900/50 p-3">
          <p className="text-xs text-muted-foreground">Guild Rank</p>
          <p className="text-lg font-semibold">{guild.rank}</p>
        </div>
        <div className="rounded-lg border border-slate-800 bg-slate-900/50 p-3">
          <p className="text-xs text-muted-foreground">Treasury (XLM)</p>
          <p className="text-lg font-semibold">{guild.treasuryXLM} XLM</p>
        </div>
      </section>

      <section>
        <h2 className="mb-2 font-semibold">Roster</h2>
        <div className="overflow-x-auto rounded-lg border border-slate-800">
          <table className="w-full text-sm">
            <thead className="bg-slate-800/50 text-left text-xs uppercase text-slate-400">
              <tr>
                <th className="p-3">Member</th>
                <th className="p-3">Rating</th>
                <th className="p-3">Role</th>
                <th className="p-3">Joined</th>
              </tr>
            </thead>
            <tbody>
              {guild.members.map((m) => (
                <tr key={m.name} className="border-t border-slate-800">
                  <td className="p-3 font-medium">{m.name}</td>
                  <td className="p-3">{m.rating}</td>
                  <td className="p-3">
                    <span
                      className={`inline-block px-2 py-0.5 rounded text-xs ${
                        m.role === "Captain"
                          ? "bg-amber-500/20 text-amber-300 border border-amber-500/30"
                          : m.role === "Officer"
                          ? "bg-indigo-500/20 text-indigo-300 border border-indigo-500/30"
                          : "bg-slate-800 text-slate-300"
                      }`}
                    >
                      {m.role}
                    </span>
                  </td>
                  <td className="p-3 text-slate-400">{m.joinedAt}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      <div className="flex gap-2">
        <button
          onClick={handleJoin}
          className="rounded-md bg-indigo-600 hover:bg-indigo-500 px-4 py-2 text-sm font-semibold text-white transition-colors"
        >
          Request to Join
        </button>
        {isCaptain && (
          <button
            onClick={() => setManageOpen(!manageOpen)}
            className="rounded-md border border-slate-700 bg-slate-800 hover:bg-slate-700 px-4 py-2 text-sm font-medium transition-colors"
          >
            {manageOpen ? "Close Management" : "Manage Roster"}
          </button>
        )}
      </div>

      {manageOpen && isCaptain && (
        <div className="p-4 rounded-lg border border-indigo-500/40 bg-slate-900/80 text-sm">
          <h3 className="font-semibold mb-2 text-indigo-300">Captain Management Panel</h3>
          <p className="text-xs text-slate-400">
            You have captain permissions to promote officers, adjust dues, or remove roster entries once on-chain governance transactions are enabled.
          </p>
        </div>
      )}
    </div>
  );
}
