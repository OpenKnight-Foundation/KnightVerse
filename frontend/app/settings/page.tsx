"use client";

import React from 'react';
import AppearanceSettings from '@/components/AppearanceSettings';
import { GameSidebar } from '@/components/GameSidebar';
import { Header } from '@/components/Header';

export default function SettingsPage() {
  const [collapsed, setCollapsed] = React.useState(false);

  return (
    <div className="flex h-screen overflow-hidden bg-background">
      <GameSidebar collapsed={collapsed} setCollapsed={setCollapsed} />
      
      <div
        className={`flex-1 flex flex-col transition-all duration-500 ease-in-out ${
          collapsed ? "md:ml-16" : "md:ml-64"
        }`}
      >
        <Header collapsed={collapsed} />
        
        <main className="flex-1 overflow-y-auto p-4 md:p-8">
          <div className="max-w-4xl mx-auto space-y-8">
            <div>
              <h1 className="text-3xl font-bold text-white mb-2">Settings</h1>
              <p className="text-gray-400">Manage your game appearance and preferences.</p>
            </div>
            
            <AppearanceSettings />
          </div>
        </main>
      </div>
    </div>
  );
}
