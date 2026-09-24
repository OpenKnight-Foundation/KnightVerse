"use client";

import { useParams } from "next/navigation";

interface GuildMember {
  name: string;
  rating: number;
  role: "Captain" | "Officer" | "Member";
  joinedAt: string;
}

const MOCK_MEMBERS: GuildMember[] = [
  { name: "KnightRider", rating: 2100, role: "Captain", joinedAt: "2025-01-10" },
  { name: "PawnStorm", rating: 1850, role: "Officer", joinedAt: "2025-02-04" },
];

export default function GuildProfilePage() {
  const params = useParams<{ id: string }>();
  const isCaptain = false; // TODO: derive from authenticated user vs. captain id

  return (
    <div className="mx-auto max-w-4xl space-y-6 p-4">
      <header className="rounded-lg border p-4">
        <h1 className="text-xl font-bold">Guild #{params.id}</h1>
        <p className="text-sm text-muted-foreground">Captain: {MOCK_MEMBERS[0].name}</p>
      </header>

      <section className="grid grid-cols-3 gap-4 text-center">
        <div className="rounded-lg border p-3">
          <p className="text-xs text-muted-foreground">Win Rate</p>
          <p className="text-lg font-semibold">--%</p>
        </div>
        <div className="rounded-lg border p-3">
          <p className="text-xs text-muted-foreground">Guild Rank</p>
          <p className="text-lg font-semibold">--</p>
        </div>
        <div className="rounded-lg border p-3">
          <p className="text-xs text-muted-foreground">Treasury (XLM)</p>
          <p className="text-lg font-semibold">--</p>
        </div>
      </section>

      <section>
        <h2 className="mb-2 font-semibold">Roster</h2>
        <table className="w-full text-sm">
          <tbody>
            {MOCK_MEMBERS.map((m) => (
              <tr key={m.name} className="border-t">
                <td className="p-2">{m.name}</td>
                <td className="p-2">{m.rating}</td>
                <td className="p-2">{m.role}</td>
                <td className="p-2">{m.joinedAt}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      <div className="flex gap-2">
        <button className="rounded-md bg-primary px-4 py-2 text-sm text-primary-foreground">
          Request to Join
        </button>
        {isCaptain && (
          <button className="rounded-md border px-4 py-2 text-sm">Manage Roster</button>
        )}
      </div>
    </div>
  );
}
