"use client";

import React from "react";
import dynamic from "next/dynamic";

const PlayGameEngine = dynamic(
  () => import("@/components/chess/PlayGameEngine"),
  {
    ssr: false,
    loading: () => (
      <div className="min-h-screen p-4 md:p-8">
        <div className="max-w-7xl mx-auto">
          <div className="flex items-center justify-between mb-4">
            <div className="h-5 w-32 bg-gray-700/50 rounded animate-pulse" />
            <div className="h-8 w-48 bg-gray-700/50 rounded-lg animate-pulse" />
          </div>
          <div className="flex flex-col lg:flex-row gap-6 items-start justify-center">
            <div className="w-full max-w-[600px] min-w-0 px-2 sm:px-0">
              <div className="flex items-center justify-between mb-3 px-1">
                <div className="flex items-center gap-3">
                  <div className="w-8 h-8 rounded-full bg-gradient-to-br from-red-400 to-red-600 animate-pulse" />
                  <div className="space-y-1">
                    <div className="h-4 w-20 bg-gray-700/50 rounded animate-pulse" />
                    <div className="h-3 w-12 bg-gray-700/50 rounded animate-pulse" />
                  </div>
                </div>
                <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-gray-800/60 border border-gray-700/30">
                  <div className="h-4 w-12 bg-gray-700/50 rounded animate-pulse" />
                </div>
              </div>
              <div className="w-full max-w-[560px] min-w-[320px] aspect-square rounded-md border-2 border-gray-700/50 p-1">
                <div className="grid grid-cols-8 grid-rows-8 gap-0 w-full h-full">
                  {Array.from({ length: 64 }).map((_, i) => (
                    <div
                      key={i}
                      className={`${
                        (Math.floor(i / 8) + (i % 8)) % 2 === 0
                          ? "bg-gray-700/30"
                          : "bg-gray-600/20"
                      } rounded-sm shimmer-bg`}
                    />
                  ))}
                </div>
              </div>
            </div>
            <div className="w-full lg:w-80 space-y-4">
              <div className="h-32 rounded-xl border border-gray-700/50 bg-gray-800/40 animate-pulse" />
              <div className="h-48 rounded-xl border border-gray-700/50 bg-gray-800/40 animate-pulse" />
              <div className="h-40 rounded-xl border border-gray-700/50 bg-gray-800/40 animate-pulse" />
            </div>
          </div>
        </div>
      </div>
    ),
  },
);

export default function PlayOnlinePage() {
  return <PlayGameEngine />;
}
