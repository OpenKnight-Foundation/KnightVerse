"use client";

import { useEffect, useMemo, useState } from "react";
import { AlertTriangle, Check, Loader2, X } from "lucide-react";
import { fetchNativeBalance, fetchTokenPrices, type SupportedToken } from "@/services/stellarMarketService";

type StakingDepositModalProps = { open: boolean; address?: string; network?: "mainnet" | "testnet"; onClose: () => void; onConfirm: (details: { token: SupportedToken; amount: number; total: number }) => Promise<void> };
const tokens: SupportedToken[] = ["XLM", "USDC", "EURC"];
const bond = 0.1;

export default function StakingDepositModal({ open, address, network = "mainnet", onClose, onConfirm }: StakingDepositModalProps) {
  const [token, setToken] = useState<SupportedToken>("XLM");
  const [stake, setStake] = useState("");
  const [prices, setPrices] = useState<Record<SupportedToken, number> | null>(null);
  const [balance, setBalance] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [error, setError] = useState("");
  const amount = Number(stake);
  const fee = Number.isFinite(amount) && amount > 0 ? amount * 0.025 : 0;
  const total = amount > 0 ? amount + fee + bond : 0;
  const price = prices?.[token] ?? 0;
  const insufficient = balance !== null && total > balance;
  const valid = Number.isFinite(amount) && amount > 0 && !insufficient && network === "mainnet" && Boolean(address);

  useEffect(() => {
    if (!open) return;
    setError(""); setLoading(true);
    Promise.all([fetchTokenPrices(), address ? fetchNativeBalance(address) : Promise.resolve(null)])
      .then(([nextPrices, nextBalance]) => { setPrices(nextPrices); setBalance(nextBalance); })
      .catch(() => setError("Live pricing or wallet balance is temporarily unavailable."))
      .finally(() => setLoading(false));
  }, [open, address]);

  const usdValue = useMemo(() => (amount > 0 ? amount * price : 0), [amount, price]);
  if (!open) return null;

  const confirm = async () => {
    if (!valid) return;
    setConfirming(true); setError("");
    try { await onConfirm({ token, amount, total }); onClose(); }
    catch (cause) { setError(cause instanceof Error ? cause.message : "Unable to confirm stake."); }
    finally { setConfirming(false); }
  };

  return <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4" role="dialog" aria-modal="true" aria-labelledby="staking-title"><div className="w-full max-w-lg border border-slate-700 bg-slate-950 p-6 shadow-2xl"><div className="mb-6 flex items-start justify-between"><div><p className="text-xs uppercase tracking-[0.2em] text-teal-400">Staked match</p><h2 id="staking-title" className="mt-1 text-2xl font-bold text-white">Confirm your deposit</h2></div><button onClick={onClose} aria-label="Close deposit modal" className="p-2 text-slate-400 hover:text-white"><X size={20} /></button></div><div className="mb-5 grid grid-cols-3 gap-2">{tokens.map((item) => <button key={item} onClick={() => setToken(item)} aria-pressed={token === item} className={`border p-3 text-sm font-bold ${token === item ? "border-teal-400 bg-teal-400/10 text-teal-300" : "border-slate-700 text-slate-400"}`}>{item}</button>)}</div><label className="text-sm text-slate-300" htmlFor="stake-amount">Match stake</label><div className="relative mt-2"><input id="stake-amount" inputMode="decimal" min="0" step="any" value={stake} onChange={(event) => setStake(event.target.value)} placeholder="0.00" className="w-full border border-slate-700 bg-slate-900 px-4 py-3 pr-16 text-lg text-white outline-none focus:border-teal-400" /><span className="absolute right-4 top-3.5 text-sm text-slate-400">{token}</span></div><p className="mt-2 text-sm text-slate-400">{loading ? "Updating rates..." : `${amount > 0 ? amount.toFixed(4) : "0.0000"} ${token} ≈ $${usdValue.toFixed(2)} USD`}</p><div className="my-5 space-y-3 border-y border-slate-800 py-4 text-sm"><div className="flex justify-between text-slate-300"><span>Match Stake</span><span>{amount.toFixed(4)} {token}</span></div><div className="flex justify-between text-slate-300"><span>Platform Fee (2.5%)</span><span>{fee.toFixed(4)} {token}</span></div><div className="flex justify-between text-slate-300"><span>Refundable Rating Bond</span><span>{bond.toFixed(4)} {token}</span></div><div className="flex justify-between text-base font-bold text-white"><span>Total Required</span><span>{total.toFixed(4)} {token}</span></div></div>{network !== "mainnet" && <p className="mb-3 flex gap-2 border border-amber-400/40 bg-amber-400/10 p-3 text-sm text-amber-200"><AlertTriangle size={18} />Switch your wallet to Stellar Mainnet before staking.</p>}{insufficient && <p className="mb-3 flex gap-2 border border-red-400/40 bg-red-400/10 p-3 text-sm text-red-200"><AlertTriangle size={18} />Insufficient balance. Available: {balance?.toFixed(4)} {token}.</p>}{error && <p role="alert" className="mb-3 text-sm text-red-300">{error}</p>}<button onClick={confirm} disabled={!valid || confirming || loading} className="flex w-full items-center justify-center gap-2 bg-teal-500 px-4 py-3 font-bold text-slate-950 disabled:cursor-not-allowed disabled:opacity-40">{confirming ? <Loader2 className="animate-spin" size={18} /> : <Check size={18} />}Confirm Stake</button></div></div>;
}