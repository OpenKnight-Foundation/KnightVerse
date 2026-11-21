"use client";

import React from 'react';
import Link from 'next/link';
import { GameSidebar } from '@/components/GameSidebar';
import { Header } from '@/components/Header';
import { useI18n } from '@/i18n/I18nProvider';

export default function PlayPage() {
  const [collapsed, setCollapsed] = React.useState(false);
  const { t } = useI18n();

  return (
    <div className="flex h-screen overflow-hidden bg-background">
      <GameSidebar collapsed={collapsed} setCollapsed={setCollapsed} />

      <div
        className={`flex-1 flex flex-col transition-all duration-500 ease-in-out ${
          collapsed ? "md:ml-16" : "md:ml-64"
        }`}
      >
        <Header />

        <main className="flex-1 overflow-y-auto p-4 md:p-8">
          <div className="max-w-4xl mx-auto space-y-8">
            <div>
              <h1 className="text-3xl font-bold text-white mb-2">{t("play.title")}</h1>
              <p className="text-gray-400">{t("play.subtitle")}</p>
            </div>
            <div className="grid gap-4 sm:grid-cols-2">
              <Link
                href="/offline"
                className="rounded-xl border border-gray-700 bg-gray-900 p-5 transition-colors hover:border-teal-400 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-400"
              >
                <h2 className="text-lg font-bold text-white">Play offline</h2>
                <p className="mt-1 text-sm text-gray-400">
                  Practise against the computer.
                </p>
              </Link>
              <Link
                href="/"
                className="rounded-xl border border-gray-700 bg-gray-900 p-5 transition-colors hover:border-teal-400 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-400"
              >
                <h2 className="text-lg font-bold text-white">Play online</h2>
                <p className="mt-1 text-sm text-gray-400">
                  Find an opponent in the lobby.
                </p>
              </Link>
            </div>
          </div>
        </main>
      </div>
    </div>
  );
}
