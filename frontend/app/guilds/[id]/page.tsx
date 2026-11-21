"use client";

import Link from "next/link";
import { useParams } from "next/navigation";
import { useCallback, useEffect, useState } from "react";
import { useAuth } from "@/context/authContext";
import {
  GuildNotFoundError,
  GuildUnavailableError,
  fetchGuild,
  isGuildCaptain,
  isGuildMember,
  previewGuild,
  type GuildProfile,
} from "@/lib/guildService";

function formatJoined(joinedAt: string | undefined): string {
  if (!joinedAt) return "—";
  const date = new Date(joinedAt);
  return Number.isNaN(date.getTime()) ? "—" : date.toLocaleDateString();
}

/**
 * Guild profile (#FE-108). Loads the guild from `GET /v1/guilds/{id}` and
 * shows a clearly-labelled preview fixture for the directory guilds while the
 * backend endpoint is still missing.
 */
export default function GuildProfilePage() {
  const params = useParams<{ id: string }>();
  const guildId = decodeURIComponent(params?.id ?? "");
  const { user } = useAuth();

  const [guild, setGuild] = useState<GuildProfile | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notFound, setNotFound] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);

  useEffect(() => {
    if (!guildId) return;
    const controller = new AbortController();
    setLoading(true);
    setError(null);
    setNotFound(false);

    (async () => {
      try {
        const data = await fetchGuild(guildId, controller.signal);
        if (controller.signal.aborted) return;
        setGuild(data);
      } catch (err) {
        if (controller.signal.aborted) return;
        // No guild API yet: fall back to the sample roster for known ids, and
        // only claim "not found" when there is no preview either.
        if (err instanceof GuildNotFoundError || err instanceof GuildUnavailableError) {
          const preview = previewGuild(guildId);
          if (preview) {
            setGuild(preview);
            setLoading(false);
            return;
          }
          if (err instanceof GuildNotFoundError) {
            setNotFound(true);
          } else {
            setError(err.message);
          }
        } else {
          setError(err instanceof Error ? err.message : "Could not load this guild.");
        }
        setGuild(null);
      } finally {
        if (!controller.signal.aborted) setLoading(false);
      }
    })();

    return () => controller.abort();
  }, [guildId, reloadKey]);

  const retry = useCallback(() => setReloadKey((k) => k + 1), []);

  if (loading) {
    return (
      <div className="mx-auto max-w-4xl space-y-6 p-4" role="status" aria-label="Loading guild">
        <div className="h-24 animate-pulse rounded-lg border bg-muted/40" />
        <div className="h-20 animate-pulse rounded-lg border bg-muted/40" />
        <p className="text-sm text-muted-foreground">Loading guild…</p>
      </div>
    );
  }

  if (notFound) {
    return (
      <div className="mx-auto max-w-xl space-y-3 p-4 py-16 text-center">
        <h1 className="text-2xl font-bold">Guild not found</h1>
        <p className="text-sm text-muted-foreground">
          No guild with id <span className="font-mono">{guildId}</span> exists.
        </p>
        <Link href="/guilds" className="inline-block text-sm text-teal-400 hover:underline">
          Back to the guild directory
        </Link>
      </div>
    );
  }

  if (error || !guild) {
    return (
      <div className="mx-auto max-w-xl space-y-3 p-4 py-16 text-center" role="alert">
        <p className="text-sm text-red-400">{error ?? "Could not load this guild."}</p>
        <button
          onClick={retry}
          className="rounded-md border px-4 py-2 text-sm hover:bg-muted/40"
        >
          Retry
        </button>
      </div>
    );
  }

  const isCaptain = isGuildCaptain(guild, user);
  const isMember = isGuildMember(guild, user);
  const captainLabel = guild.captainUsername ?? "Unknown";

  return (
    <div className="mx-auto max-w-4xl space-y-6 p-4">
      {guild.isPreview && (
        <div
          role="status"
          className="rounded-lg border border-amber-500/40 bg-amber-500/10 p-3 text-sm text-amber-200"
        >
          <strong className="font-semibold">Preview data.</strong> The guild API is not
          available yet, so this roster is sample content for guild #{guild.id}. Joining and
          roster management are disabled.
        </div>
      )}

      <header className="rounded-lg border p-4">
        <h1 className="text-xl font-bold">{guild.name}</h1>
        <p className="text-sm text-muted-foreground">
          Captain: {captainLabel} · {guild.members.length}{" "}
          {guild.members.length === 1 ? "member" : "members"}
        </p>
      </header>

      <section aria-label="Guild statistics" className="grid grid-cols-3 gap-4 text-center">
        <div className="rounded-lg border p-3">
          <p className="text-xs text-muted-foreground">Win Rate</p>
          <p className="text-lg font-semibold">
            {guild.winRate !== undefined ? `${Math.round(guild.winRate)}%` : "--%"}
          </p>
        </div>
        <div className="rounded-lg border p-3">
          <p className="text-xs text-muted-foreground">Guild Rank</p>
          <p className="text-lg font-semibold">
            {guild.rank !== undefined ? `#${guild.rank}` : "--"}
          </p>
        </div>
        <div className="rounded-lg border p-3">
          <p className="text-xs text-muted-foreground">Treasury (XLM)</p>
          <p className="text-lg font-semibold">
            {guild.treasuryXlm !== undefined
              ? guild.treasuryXlm.toLocaleString(undefined, { maximumFractionDigits: 2 })
              : "--"}
          </p>
        </div>
      </section>

      <section aria-labelledby="guild-roster-heading">
        <h2 id="guild-roster-heading" className="mb-2 font-semibold">
          Roster
        </h2>
        {guild.members.length === 0 ? (
          <p className="rounded-lg border border-dashed p-4 text-sm text-muted-foreground">
            This guild has no members yet.
          </p>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-xs uppercase tracking-wide text-muted-foreground">
                <th scope="col" className="p-2 font-medium">
                  Player
                </th>
                <th scope="col" className="p-2 font-medium">
                  Rating
                </th>
                <th scope="col" className="p-2 font-medium">
                  Role
                </th>
                <th scope="col" className="p-2 font-medium">
                  Joined
                </th>
              </tr>
            </thead>
            <tbody>
              {guild.members.map((m) => (
                <tr key={`${m.userId ?? m.username}-${m.role}`} className="border-t">
                  <td className="p-2">{m.username}</td>
                  <td className="p-2">{m.rating}</td>
                  <td className="p-2">{m.role}</td>
                  <td className="p-2">{formatJoined(m.joinedAt)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>

      <div className="flex flex-wrap items-center gap-2">
        {isCaptain ? (
          <button
            disabled={guild.isPreview}
            title={guild.isPreview ? "Unavailable while preview data is shown" : undefined}
            className="rounded-md border px-4 py-2 text-sm disabled:cursor-not-allowed disabled:opacity-50"
          >
            Manage Roster
          </button>
        ) : (
          <button
            disabled={guild.isPreview || isMember}
            title={
              guild.isPreview
                ? "Unavailable while preview data is shown"
                : isMember
                  ? "You are already a member of this guild"
                  : undefined
            }
            className="rounded-md bg-primary px-4 py-2 text-sm text-primary-foreground disabled:cursor-not-allowed disabled:opacity-50"
          >
            {isMember ? "Already a member" : "Request to Join"}
          </button>
        )}
      </div>
    </div>
  );
}
