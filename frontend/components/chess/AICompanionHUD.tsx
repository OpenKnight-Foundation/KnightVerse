"use client";

import { useEffect, useState } from "react";
import { Volume2, VolumeX } from "lucide-react";
import { getCompanionEmotion, type CompanionEmotion } from "@/lib/aiCompanion";

type AICompanionHUDProps = { evaluation: number | null; commentary?: string; isVictory?: boolean; isTactical?: boolean; className?: string };
const emotionCopy: Record<CompanionEmotion, { label: string; icon: string }> = { confident: { label: "Confident", icon: "♞" }, panicked: { label: "Panicked", icon: "!" }, calculating: { label: "Calculating", icon: "⌁" }, celebratory: { label: "Celebratory", icon: "★" }, thoughtful: { label: "Thoughtful", icon: "?" } };

export default function AICompanionHUD({ evaluation, commentary, isVictory = false, isTactical = false, className = "" }: AICompanionHUDProps) {
  const [muted, setMuted] = useState(false);
  const [visibleText, setVisibleText] = useState("");
  const emotion = getCompanionEmotion(evaluation, isVictory, isTactical);
  const copy = emotionCopy[emotion];

  useEffect(() => {
    if (!commentary || muted) { setVisibleText(""); return; }
    let index = 0; setVisibleText("");
    const timer = window.setInterval(() => { index += 1; setVisibleText(commentary.slice(0, index)); if (index >= commentary.length) window.clearInterval(timer); }, 28);
    return () => window.clearInterval(timer);
  }, [commentary, muted]);

  return <aside className={`flex min-h-20 items-center gap-3 border border-teal-400/30 bg-slate-950/90 p-3 ${className}`} aria-live="polite"><div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-full border border-teal-300 bg-teal-400/10 text-2xl text-teal-300" aria-label={`${copy.label} AI avatar`}>{copy.icon}</div><div className="min-w-0 flex-1"><div className="flex items-center justify-between"><p className="text-xs font-bold uppercase tracking-widest text-teal-300">{copy.label}</p><button onClick={() => setMuted((value) => !value)} aria-label={muted ? "Unmute AI commentary" : "Mute AI commentary"} className="p-1 text-slate-400 hover:text-white">{muted ? <VolumeX size={16} /> : <Volume2 size={16} />}</button></div><p className="min-h-6 text-sm text-slate-200">{muted ? "Commentary muted" : visibleText || "Watching the position..."}</p></div></aside>;
}