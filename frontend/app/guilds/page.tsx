"use client";

import Link from "next/link";

/**
 * Guild directory (FE-119 / #1360). The per-guild profile lives at
 * `/guilds/[id]`; this index gives the sidebar "Guilds" entry a real
 * destination instead of a 404. Guild data is mocked until the backend
 * exposes a guilds endpoint (tracked as a backend dependency).
 */
const MOCK_GUILDS = [
  { id: "1", name: "Knights of the Realm", members: 24, rating: 1980 },
  { id: "2", name: "Rook & Roll", members: 18, rating: 1860 },
  { id: "3", name: "The En Passant Society", members: 12, rating: 1740 },
];

export default function GuildsPage() {
  return (
    <div className="min-h-screen p-4 md:p-8 text-white" role="main" aria-label="Guilds">
      <div className="max-w-4xl mx-auto">
        <h1 className="text-3xl font-extrabold bg-gradient-to-r from-teal-400 to-blue-500 bg-clip-text text-transparent">
          Guilds
        </h1>
        <p className="text-gray-400 text-sm mt-1">Browse and join a guild</p>

        <ul className="mt-8 flex flex-col gap-3">
          {MOCK_GUILDS.map((guild) => (
            <li key={guild.id}>
              <Link
                href={`/guilds/${guild.id}`}
                className="flex items-center justify-between px-5 py-4 rounded-xl border border-gray-700/50 bg-gray-800/30 hover:border-teal-500/40 hover:bg-teal-500/5 transition-all"
              >
                <div>
                  <p className="font-semibold">{guild.name}</p>
                  <p className="text-xs text-gray-500 mt-0.5">
                    {guild.members} members · Rating {guild.rating}
                  </p>
                </div>
                <span className="text-teal-400 text-sm">View →</span>
              </Link>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
