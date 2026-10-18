"use client";

import { useState } from "react";
import StakingDepositModal from "@/components/StakingDepositModal";
import { useAppContext } from "@/context/walletContext";
import { useWalletLink } from "@/hook/useWalletLink";
import { NETWORK_PASSPHRASE } from "@/lib/api";
import {
  TOKEN_ISSUERS,
  type StellarNetwork,
  type SupportedToken,
} from "@/services/stellarMarketService";

const STAKING_ESCROW_ADDRESS = process.env.NEXT_PUBLIC_STAKING_ESCROW_ADDRESS ?? "";
const network: StellarNetwork = NETWORK_PASSPHRASE.startsWith("Public Global") ? "mainnet" : "testnet";

/**
 * Resolve the Stellar asset a stake is denominated in (FE-133).
 *
 * XLM is native. USDC/EURC are issued by Circle, and an asset code alone is
 * not unique on Stellar — the issuer must be pinned to the one for the network
 * the app is pointed at, otherwise the payment would either fail or (worse)
 * target a look-alike token.
 */
function assetForToken(token: SupportedToken): { code: string; issuer: string } | undefined {
  if (token === "XLM") return undefined;
  return { code: token, issuer: TOKEN_ISSUERS[network][token] };
}

export default function StakePage() {
  const { address, connectWallet, sendPayment } = useAppContext();
  const { linkedWallet, isMismatched } = useWalletLink();
  const [open, setOpen] = useState(false);
  const [confirmation, setConfirmation] = useState<string | null>(null);

  const handleConfirm = async ({ token, total }: { token: SupportedToken; amount: number; total: number }) => {
    if (!STAKING_ESCROW_ADDRESS) throw new Error("Staking escrow is not configured.");
    // Stakes must come from the wallet linked to the account (#FE-97).
    if (isMismatched) throw new Error(`Switch Freighter to your linked wallet (${linkedWallet}) to stake.`);
    const result = await sendPayment(STAKING_ESCROW_ADDRESS, total.toFixed(7), assetForToken(token));
    setConfirmation(`Stake confirmed: ${total.toFixed(4)} ${token}${result?.hash ? ` (tx ${result.hash})` : ""}`);
  };

  return (
    <div className="min-h-screen p-4 md:p-8 text-white" role="main" aria-label="Staked match">
      <div className="max-w-xl mx-auto">
        <h1 className="text-3xl font-extrabold bg-gradient-to-r from-teal-400 to-blue-500 bg-clip-text text-transparent">
          Staked Match
        </h1>
        <p className="text-gray-400 text-sm mt-1 mb-8">Deposit a stake to play a rated match in XLM, USDC or EURC.</p>

        {isMismatched && (
          <div role="alert" className="mb-6 p-4 rounded-xl bg-amber-500/10 border border-amber-500/30 text-amber-200 text-sm">
            Your connected wallet isn&apos;t the one linked to your account ({linkedWallet}). Switch Freighter
            to the linked wallet before staking.
          </div>
        )}

        {confirmation && (
          <div role="status" className="mb-6 p-4 rounded-xl bg-teal-500/10 border border-teal-500/30 text-teal-300 text-sm">
            {confirmation}
          </div>
        )}

        {address ? (
          <button
            onClick={() => { setConfirmation(null); setOpen(true); }}
            className="px-6 py-3 rounded-xl bg-teal-500 hover:bg-teal-400 text-black font-bold text-sm transition-all"
          >
            Stake &amp; Play
          </button>
        ) : (
          <button
            onClick={() => void connectWallet()}
            className="px-6 py-3 rounded-xl bg-teal-500/10 border border-teal-500/30 text-teal-300 font-semibold text-sm transition-all"
          >
            Connect wallet to stake
          </button>
        )}
      </div>

      <StakingDepositModal
        open={open}
        address={address}
        network={network}
        onClose={() => setOpen(false)}
        onConfirm={handleConfirm}
      />
    </div>
  );
}
