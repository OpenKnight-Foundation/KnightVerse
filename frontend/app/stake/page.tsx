
"use client";

import { useEffect, useState } from "react";
import StakingDepositModal from "@/components/StakingDepositModal";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { useAppContext } from "@/context/walletContext";
import { useWalletLink } from "@/hook/useWalletLink";
import {
  getYieldStakingConfig,
  stakeInPool,
  toTokenAmount,
} from "@/contracts/client";
import { NETWORK_PASSPHRASE } from "@/lib/api";
import {
  formatUsd,
  getDailyUsage,
  getResponsiblePlaySettings,
  getSelfExclusionState,
  recordWager,
  remainingDailyLimit,
  type DailyUsage,
  type ResponsiblePlaySettings,
} from "@/lib/responsiblePlay";
import {
  TOKEN_ISSUERS,
  type StellarNetwork,
  type SupportedToken,
} from "@/services/stellarMarketService";

/**
 * Staking Configuration
 *
 * The escrow address receives staking deposits, while the network configuration
 * determines which token issuers are used for non-native Stellar assets.
 */
const STAKING_ESCROW_ADDRESS =
  process.env.NEXT_PUBLIC_STAKING_ESCROW_ADDRESS ?? "";

// Determine the Stellar network from the configured network passphrase.
const network: StellarNetwork = NETWORK_PASSPHRASE.startsWith("Public Global")
  ? "mainnet"
  : "testnet";

/**
 * Resolves the Stellar asset associated with a staking token (FE-133).
 *
 * XLM is the native Stellar asset and does not require an issuer.
 * USDC and EURC are issued assets, so their issuer addresses must match
 * the configured network to avoid targeting an incorrect or look-alike token.
 *
 * @param token - The token selected for the staking deposit.
 * @returns The asset code and issuer, or undefined for native XLM.
 */
function assetForToken(
  token: SupportedToken,
): { code: string; issuer: string } | undefined {
  if (token === "XLM") return undefined;

  return {
    code: token,
    issuer: TOKEN_ISSUERS[network][token],
  };
}

/**
 * Stake Page
 *
 * Allows users to deposit XLM, USDC, or EURC into the configured staking
 * escrow before participating in a rated match.
 *
 * The page handles wallet connection, linked-wallet validation, deposit
 * confirmation, and transaction feedback.
 */
export default function StakePage() {
  const { address, connectWallet, sendPayment, signTransaction } = useAppContext();
  const { linkedWallet, isMismatched } = useWalletLink();

  const [open, setOpen] = useState(false);
  const [confirmation, setConfirmation] = useState<string | null>(null);
  const [settings, setSettings] = useState<ResponsiblePlaySettings | null>(null);
  const [usage, setUsage] = useState<DailyUsage | null>(null);
  const [now, setNow] = useState(0);

  useEffect(() => {
    setSettings(getResponsiblePlaySettings());
    setUsage(getDailyUsage());
    setNow(Date.now());
    const id = window.setInterval(() => {
      setSettings(getResponsiblePlaySettings());
      setUsage(getDailyUsage());
      setNow(Date.now());
    }, 60_000);
    return () => window.clearInterval(id);
  }, []);

  const exclusion = settings && now ? getSelfExclusionState(settings, new Date(now)) : null;
  const remaining = settings && usage ? remainingDailyLimit(settings, usage) : null;

  useEffect(() => {
    if (exclusion?.active) setOpen(false);
  }, [exclusion?.active]);

  /**
   * Processes a staking deposit after the user confirms the amount and token.
   *
   * Ensures the connected wallet matches the wallet linked to the user's
   * account, then stakes through the yield-staking contract when it is
   * configured, or falls back to a payment to the escrow account.
   *
   * @param deposit - The selected token, entered amount, and total deposit.
   * @throws Error if the wallet is mismatched or no staking target is configured.
   */
  const handleConfirm = async ({ token, total, usdValue }: { token: SupportedToken; amount: number; total: number; usdValue: number }) => {
    if (getSelfExclusionState(getResponsiblePlaySettings()).active) {
      throw new Error("Self-exclusion is active. Staking is blocked.");
    }
    const currentSettings = getResponsiblePlaySettings();
    const currentUsage = getDailyUsage();
    if (usdValue > remainingDailyLimit(currentSettings, currentUsage)) {
      throw new Error(`This stake exceeds your remaining daily limit of ${formatUsd(remainingDailyLimit(currentSettings, currentUsage))}.`);
    }

    // Stakes must come from the wallet linked to the account (#FE-97).
    if (isMismatched) throw new Error(`Switch Freighter to your linked wallet (${linkedWallet}) to stake.`);

    // SC-91: when the yield-staking contract is deployed, the stake is placed
    // through the typed bindings (`yield_staking.stake`) instead of a plain
    // payment to the escrow account. The pool must be whitelisted on-chain and
    // the amount is denominated in the staking contract's token (7 decimals).
    const staking = getYieldStakingConfig();
    if (staking) {
      if (!address) throw new Error("Connect your wallet to stake.");
      const { hash } = await stakeInPool({
        caller: address,
        pool: staking.poolAddress,
        amount: toTokenAmount(total),
        signer: { address, sign: signTransaction },
      });
      setConfirmation(`Stake confirmed: ${total.toFixed(4)} ${token}${hash ? ` (tx ${hash})` : ""}`);
      setUsage(recordWager(usdValue));
      return;
    }

    if (!STAKING_ESCROW_ADDRESS) throw new Error("Staking escrow is not configured.");
    const result = await sendPayment(STAKING_ESCROW_ADDRESS, total.toFixed(7), assetForToken(token));
    setConfirmation(`Stake confirmed: ${total.toFixed(4)} ${token}${result?.hash ? ` (tx ${result.hash})` : ""}`);
    setUsage(recordWager(usdValue));
  };

  return (
    <main
      className="min-h-screen p-4 text-white md:p-8"
      aria-label="Staked match"
    >
      <div className="mx-auto max-w-xl">
        {/* Page heading and supported staking assets. */}
        <h1 className="bg-gradient-to-r from-teal-400 to-blue-500 bg-clip-text text-3xl font-extrabold text-transparent">
          Staked Match
        </h1>

        <p className="mb-8 mt-1 text-sm text-gray-400">
          Deposit a stake to play a rated match in XLM, USDC, or EURC.
        </p>

        {/* Warn users when their connected wallet differs from the linked wallet. */}
        {isMismatched && (
          <Alert variant="warning" className="mb-6 text-sm text-amber-200">
            <AlertDescription>
              Your connected wallet isn&apos;t the one linked to your account (
              {linkedWallet}). Switch Freighter to the linked wallet before
              staking.
            </AlertDescription>
          </Alert>
        )}

        {exclusion?.active && (
          <Alert variant="warning" className="mb-6 text-sm text-red-200">
            <AlertDescription>
              Self-exclusion is active until {new Date(exclusion.endsAt ?? Date.now()).toLocaleString()}.
              Staking is blocked. Manage this in{" "}
              <a href="/settings" className="underline">
                Settings
              </a>
              .
            </AlertDescription>
          </Alert>
        )}

        {remaining !== null && (
          <p className="mb-6 text-sm text-gray-400" role="status">
            Daily limit: {formatUsd(remaining)} remaining of {formatUsd(settings?.dailyLimitUsd ?? 0)}
            {usage ? ` · ${formatUsd(usage.wageredUsd)} used today` : ""}
          </p>
        )}

        {/* Show transaction feedback after a successful payment request. */}
        {confirmation && (
          <div
            role="status"
            className="mb-6 rounded-xl border border-teal-500/30 bg-teal-500/10 p-4 text-sm text-teal-300"
          >
            {confirmation}
          </div>
        )}

        {/* Allow connected users to initiate staking or prompt others to connect. */}
        {exclusion?.active ? (
          <button
            type="button"
            disabled
            className="cursor-not-allowed rounded-xl bg-gray-700/50 px-6 py-3 text-sm font-bold text-gray-400"
          >
            Staking blocked by self-exclusion
          </button>
        ) : address ? (
          <button
            type="button"
            onClick={() => {
              setConfirmation(null);
              setOpen(true);
            }}
            className="rounded-xl bg-teal-500 px-6 py-3 text-sm font-bold text-black transition-all hover:bg-teal-400 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-300"
          >
            Stake &amp; Play
          </button>
        ) : (
          <button
            type="button"
            onClick={() => void connectWallet()}
            className="rounded-xl border border-teal-500/30 bg-teal-500/10 px-6 py-3 text-sm font-semibold text-teal-300 transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-300"
          >
            Connect wallet to stake
          </button>
        )}
      </div>

      {/* Collect the deposit details and handle the confirmed staking payment. */}
      <StakingDepositModal
        open={open}
        address={address}
        network={network}
        remainingUsd={remaining}
        onClose={() => setOpen(false)}
        onConfirm={handleConfirm}
      />
    </main>
  );
}
