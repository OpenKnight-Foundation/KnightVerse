"use client";

import React from 'react';
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
          </div>
        </main>
      </div>
    </div>
  );
}
