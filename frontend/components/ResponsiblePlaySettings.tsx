"use client";

import React, { useEffect, useMemo, useState } from "react";
import { AlertTriangle, ShieldCheck } from "lucide-react";
import {
  cancelSelfExclusion,
  formatUsd,
  getDailyUsage,
  getResponsiblePlaySettings,
  getSelfExclusionState,
  remainingDailyLimit,
  requestSelfExclusion,
  saveDailyLimitUsd,
  SELF_EXCLUSION_DELAY_HOURS,
  type DailyUsage,
  type ResponsiblePlaySettings,
} from "@/lib/responsiblePlay";

const inputClass =
  "w-full rounded-xl border border-gray-700/50 bg-gray-900/70 px-3 py-2 text-sm text-white outline-none focus:border-teal-400";
const buttonClass =
  "rounded-xl bg-teal-500 px-4 py-2 text-sm font-bold text-black transition-all hover:bg-teal-400 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-300 disabled:cursor-not-allowed disabled:opacity-50";

export default function ResponsiblePlaySettings() {
  const [settings, setSettings] = useState<ResponsiblePlaySettings | null>(null);
  const [usage, setUsage] = useState<DailyUsage | null>(null);
  const [limitInput, setLimitInput] = useState("100");
  const [days, setDays] = useState("7");
  const [now, setNow] = useState(0);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    const initial = getResponsiblePlaySettings();
    setSettings(initial);
    setUsage(getDailyUsage());
    setLimitInput(String(initial.dailyLimitUsd));
    setNow(Date.now());
    const id = window.setInterval(() => setNow(Date.now()), 60_000);
    return () => window.clearInterval(id);
  }, []);

  const exclusion = useMemo(
    () => (settings && now ? getSelfExclusionState(settings, new Date(now)) : null),
    [settings, now],
  );
  const remaining = settings && usage ? remainingDailyLimit(settings, usage) : null;

  const saveLimit = () => {
    const parsed = Number(limitInput);
    if (!Number.isFinite(parsed) || parsed < 0) {
      setMessage("Enter a valid daily limit.");
      return;
    }
    const next = saveDailyLimitUsd(parsed);
    setSettings(next);
    setMessage(`Daily wager limit set to ${formatUsd(next.dailyLimitUsd)}.`);
  };

  const request = () => {
    const next = requestSelfExclusion(Number(days), now ? new Date(now) : new Date());
    setSettings(next);
    setMessage(
      `Self-exclusion will activate ${new Date(next.selfExclusionStartsAt ?? Date.now()).toLocaleString()} after a ${SELF_EXCLUSION_DELAY_HOURS}-hour cooling-off period.`,
    );
  };

  const cancel = () => {
    setSettings(cancelSelfExclusion());
    setMessage("Pending self-exclusion request cancelled.");
  };

  return (
    <section
      className="rounded-xl border border-gray-700/50 bg-gray-800/40 p-6 space-y-6"
      aria-labelledby="responsible-play-title"
    >
      <div className="flex items-center gap-3">
        <ShieldCheck className="h-6 w-6 text-teal-400" aria-hidden="true" />
        <div>
          <h2 id="responsible-play-title" className="text-lg font-bold text-white">
            Responsible Play
          </h2>
          <p className="text-xs text-gray-400">
            Limits are stored on this device and apply immediately.
          </p>
        </div>
      </div>

      <div className="space-y-3">
        <label htmlFor="daily-limit" className="block text-sm font-semibold text-gray-300">
          Daily wager limit (USD)
        </label>
        <div className="flex gap-2">
          <input
            id="daily-limit"
            type="number"
            min="0"
            step="1"
            value={limitInput}
            onChange={(event) => setLimitInput(event.target.value)}
            className={inputClass}
          />
          <button type="button" onClick={saveLimit} className={buttonClass}>
            Save
          </button>
        </div>
        {remaining !== null && (
          <p className="text-xs text-gray-400" role="status">
            {formatUsd(remaining)} remaining today
            {usage ? ` · ${formatUsd(usage.wageredUsd)} already wagered` : ""}
          </p>
        )}
      </div>

      <div className="space-y-3 border-t border-gray-700/50 pt-5">
        <h3 className="text-sm font-semibold text-gray-300">Self-exclusion</h3>
        <label htmlFor="exclusion-days" className="block text-xs text-gray-400">
          Exclude yourself for
        </label>
        <div className="flex gap-2">
          <select
            id="exclusion-days"
            value={days}
            onChange={(event) => setDays(event.target.value)}
            className={inputClass}
            disabled={exclusion?.active || exclusion?.pending}
          >
            <option value="1">1 day</option>
            <option value="7">7 days</option>
            <option value="30">30 days</option>
          </select>
          <button
            type="button"
            onClick={request}
            disabled={exclusion?.active || exclusion?.pending}
            className={buttonClass}
          >
            Request
          </button>
        </div>
        <p className="text-xs text-gray-400">
          Activation is delayed by {SELF_EXCLUSION_DELAY_HOURS} hours so you can cancel
          while the request is still pending. Once active, all staking controls are
          blocked immediately.
        </p>
        {exclusion?.pending && (
          <div className="rounded-xl border border-amber-500/30 bg-amber-500/10 p-3 text-sm text-amber-200" role="status">
            Pending: active in {exclusion.hoursUntilActive} hour
            {exclusion.hoursUntilActive === 1 ? "" : "s"} (
            {new Date(exclusion.startsAt ?? Date.now()).toLocaleString()}).
            <button type="button" onClick={cancel} className="ml-2 underline">
              Cancel
            </button>
          </div>
        )}
        {exclusion?.active && (
          <div className="flex gap-2 rounded-xl border border-red-500/30 bg-red-500/10 p-3 text-sm text-red-200" role="alert">
            <AlertTriangle className="h-4 w-4 shrink-0" aria-hidden="true" />
            Self-exclusion is active until {new Date(exclusion.endsAt ?? Date.now()).toLocaleString()}.
            Staking is blocked.
          </div>
        )}
      </div>

      {message && (
        <p className="text-xs text-teal-300" role="status">
          {message}
        </p>
      )}
    </section>
  );
}
