import { describe, expect, it } from "vitest";

import {
  MAINNET_PASSPHRASE,
  TESTNET_PASSPHRASE,
  describeNetwork,
  isNetworkMismatch,
  networkMismatchMessage,
  passphraseFromNetworkName,
  passphraseFromWalletResponse,
} from "@/lib/walletNetwork";

describe("passphraseFromNetworkName", () => {
  it("maps Freighter's network names to passphrases", () => {
    expect(passphraseFromNetworkName("PUBLIC")).toBe(MAINNET_PASSPHRASE);
    expect(passphraseFromNetworkName("testnet")).toBe(TESTNET_PASSPHRASE);
    expect(passphraseFromNetworkName(" TestNet ")).toBe(TESTNET_PASSPHRASE);
  });

  it("returns undefined for an unknown or missing name", () => {
    expect(passphraseFromNetworkName("SOMETHING_ELSE")).toBeUndefined();
    expect(passphraseFromNetworkName(undefined)).toBeUndefined();
    expect(passphraseFromNetworkName("")).toBeUndefined();
  });
});

describe("passphraseFromWalletResponse", () => {
  it("prefers the explicit passphrase from getNetworkDetails", () => {
    expect(
      passphraseFromWalletResponse({
        network: "PUBLIC",
        networkPassphrase: TESTNET_PASSPHRASE,
      }),
    ).toBe(TESTNET_PASSPHRASE);
  });

  it("falls back to the network name", () => {
    expect(passphraseFromWalletResponse({ network: "TESTNET" })).toBe(
      TESTNET_PASSPHRASE,
    );
    expect(passphraseFromWalletResponse("PUBLIC")).toBe(MAINNET_PASSPHRASE);
  });

  it("treats an unknown passphrase string as itself", () => {
    expect(passphraseFromWalletResponse("Some Custom Network")).toBe(
      "Some Custom Network",
    );
  });

  it("returns undefined for junk", () => {
    expect(passphraseFromWalletResponse(undefined)).toBeUndefined();
    expect(passphraseFromWalletResponse(null)).toBeUndefined();
    expect(passphraseFromWalletResponse({})).toBeUndefined();
    expect(passphraseFromWalletResponse(42)).toBeUndefined();
  });
});

describe("isNetworkMismatch", () => {
  it("flags mainnet signing against a testnet app, and the reverse", () => {
    expect(isNetworkMismatch(MAINNET_PASSPHRASE, TESTNET_PASSPHRASE)).toBe(true);
    expect(isNetworkMismatch(TESTNET_PASSPHRASE, MAINNET_PASSPHRASE)).toBe(true);
  });

  it("does not flag matching networks", () => {
    expect(isNetworkMismatch(TESTNET_PASSPHRASE, TESTNET_PASSPHRASE)).toBe(false);
  });

  it("does not flag an unknown wallet network", () => {
    expect(isNetworkMismatch(undefined, TESTNET_PASSPHRASE)).toBe(false);
    expect(isNetworkMismatch(TESTNET_PASSPHRASE, undefined)).toBe(false);
  });
});

describe("networkMismatchMessage", () => {
  it("names both networks so the fix is obvious", () => {
    const message = networkMismatchMessage(MAINNET_PASSPHRASE, TESTNET_PASSPHRASE);
    expect(message).toContain("Mainnet");
    expect(message).toContain("Testnet");
    expect(message).toMatch(/Switch networks in Freighter/);
  });
});

describe("describeNetwork", () => {
  it("uses friendly names, falling back to the raw passphrase", () => {
    expect(describeNetwork(MAINNET_PASSPHRASE)).toBe("Mainnet");
    expect(describeNetwork(TESTNET_PASSPHRASE)).toBe("Testnet");
    expect(describeNetwork("Custom Chain")).toBe('"Custom Chain"');
    expect(describeNetwork(undefined)).toBe("an unknown network");
  });
});
