"use client";

import React from 'react';
import AppearanceSettings from '@/components/AppearanceSettings';
import SoundSettings from '@/components/SoundSettings';
import GamePreferencesSettings from '@/components/GamePreferencesSettings';
import LinkedWalletSettings from '@/components/LinkedWalletSettings';
import { LanguageSelector } from '@/components/LanguageSelector';
import { GameSidebar } from '@/components/GameSidebar';
import { Header } from '@/components/Header';
import { useI18n } from '@/i18n/I18nProvider';

export default function SettingsPage() {
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
              <h1 className="text-3xl font-bold text-white mb-2">{t("settings.title")}</h1>
              <p className="text-gray-400">{t("settings.subtitle")}</p>
            </div>
            
            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              <AppearanceSettings />
              <SoundSettings />
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              <GamePreferencesSettings />
              <LinkedWalletSettings />
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              <LanguageSelector />
            </div>
          </div>
        </main>
      </div>
    </div>
  );
}
