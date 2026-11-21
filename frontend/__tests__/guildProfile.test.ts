import { describe, it, expect } from "vitest";
import {
  isGuildCaptain,
  isGuildMember,
  previewGuild,
  toGuildMember,
  toGuildProfile,
} from "@/lib/guildService";

describe("guild profile normalisation (#1349)", () => {
  it("normalises a snake_case payload with a nested captain and roster", () => {
    const guild = toGuildProfile(
      {
        id: 42,
        guild_name: "Knights of the Realm",
        captain: { user_id: 7, username: "KnightRider" },
        win_rate: 0.62,
        guild_rank: 12,
        treasury_xlm: 500.5,
        members: [
          { user_id: 7, username: "KnightRider", elo: 2100, role: "owner", joined_at: "2025-01-10" },
          { name: "PawnStorm", rating: 1850, guild_role: "moderator" },
        ],
      },
      "42",
    );

    expect(guild).toMatchObject({
      id: "42",
      name: "Knights of the Realm",
      captainUsername: "KnightRider",
      captainUserId: 7,
      winRate: 62,
      rank: 12,
      treasuryXlm: 500.5,
      isPreview: false,
    });
    expect(guild.members).toEqual([
      {
        userId: 7,
        username: "KnightRider",
        rating: 2100,
        role: "Captain",
        joinedAt: "2025-01-10",
      },
      {
        userId: undefined,
        username: "PawnStorm",
        rating: 1850,
        role: "Officer",
        joinedAt: undefined,
      },
    ]);
  });

  it("derives the captain from the roster and flat id fields", () => {
    const fromRoster = toGuildProfile(
      {
        name: "Rook & Roll",
        members: [{ username: "RookStar", role: "Captain", rating: 2020 }],
      },
      "2",
    );
    expect(fromRoster).toMatchObject({
      id: "2",
      captainUsername: "RookStar",
      winRate: undefined,
      rank: undefined,
      treasuryXlm: undefined,
    });

    const flat = toGuildProfile(
      { captain_id: 9, captain_username: "EnPassantPete", members: [] },
      "3",
    );
    expect(flat).toMatchObject({ captainUserId: 9, captainUsername: "EnPassantPete" });
  });

  it("tolerates a missing body and skips malformed members", () => {
    const guild = toGuildProfile({ members: [null, { rating: 1200 }, { username: "ok" }] }, "9");
    expect(guild.name).toBe("Guild #9");
    expect(guild.members).toHaveLength(1);
    expect(guild.captainUsername).toBeNull();
  });

  it("rejects members without a name", () => {
    expect(toGuildMember({ rating: 1500 })).toBeNull();
    expect(toGuildMember("nope")).toBeNull();
  });
});

describe("guild permissions (#1349)", () => {
  const guild = toGuildProfile(
    {
      id: "1",
      captain: { user_id: 7, username: "KnightRider" },
      members: [
        { user_id: 7, username: "KnightRider", role: "captain" },
        { username: "PawnStorm", role: "officer" },
      ],
    },
    "1",
  );

  it("detects the captain by account id or username", () => {
    expect(isGuildCaptain(guild, { user_id: 7, username: "someone-else" })).toBe(true);
    expect(isGuildCaptain(guild, { user_id: 99, username: "knightrider" })).toBe(true);
    expect(isGuildCaptain(guild, { user_id: 99, username: "PawnStorm" })).toBe(false);
    expect(isGuildCaptain(guild, null)).toBe(false);
  });

  it("detects roster membership", () => {
    expect(isGuildMember(guild, { username: "pawnstorm" })).toBe(true);
    expect(isGuildMember(guild, { user_id: 7 })).toBe(true);
    expect(isGuildMember(guild, { user_id: 1, username: "outsider" })).toBe(false);
    expect(isGuildMember(guild, null)).toBe(false);
  });
});

describe("guild preview fixtures (#1349)", () => {
  it("has a distinct captain and roster per directory guild", () => {
    const first = previewGuild("1");
    const second = previewGuild("2");
    expect(first?.isPreview).toBe(true);
    expect(first?.captainUsername).toBe("KnightRider");
    expect(second?.captainUsername).toBe("RookStar");
    expect(first?.members[0].username).not.toBe(second?.members[0].username);
  });

  it("returns null for ids without a preview", () => {
    expect(previewGuild("does-not-exist")).toBeNull();
  });

  it("returns a copy that cannot mutate the fixture", () => {
    const guild = previewGuild("1");
    expect(guild).not.toBeNull();
    if (!guild) return;
    guild.members[0].username = "changed";
    expect(previewGuild("1")?.members[0].username).toBe("KnightRider");
  });
});
