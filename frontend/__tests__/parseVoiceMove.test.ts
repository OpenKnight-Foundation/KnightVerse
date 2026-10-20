import { describe, it, expect } from "vitest";
import { parseVoiceToSan } from "@/lib/parseVoiceMove";

describe("parseVoiceToSan (FE-52)", () => {
  it("parses piece + square", () => {
    expect(parseVoiceToSan("Knight f3")).toBe("Nf3");
    expect(parseVoiceToSan("knight to f3")).toBe("Nf3");
  });

  it("parses pawn pushes", () => {
    expect(parseVoiceToSan("Pawn to e4")).toBe("e4");
    expect(parseVoiceToSan("e4")).toBe("e4");
    expect(parseVoiceToSan("pawn e four")).toBe("e4");
  });

  it("parses captures", () => {
    expect(parseVoiceToSan("Queen takes d7")).toBe("Qxd7");
    expect(parseVoiceToSan("Bishop captures c6")).toBe("Bxc6");
    expect(parseVoiceToSan("Knight takes f3")).toBe("Nxf3");
  });

  it("parses castling", () => {
    expect(parseVoiceToSan("castle kingside")).toBe("O-O");
    expect(parseVoiceToSan("castle queenside")).toBe("O-O-O");
    expect(parseVoiceToSan("short castle")).toBe("O-O");
    expect(parseVoiceToSan("long castle")).toBe("O-O-O");
  });

  it("handles suffixes", () => {
    expect(parseVoiceToSan("Queen h5 check")).toBe("Qh5+");
  });

  it("returns null for unparseable speech", () => {
    expect(parseVoiceToSan("hello world")).toBeNull();
    expect(parseVoiceToSan("")).toBeNull();
  });
});
