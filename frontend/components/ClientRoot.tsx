"use client";

import { useState } from "react";
import { GameSidebar } from "@/components/GameSidebar";
import { TransactionStatusIndicator } from "@/components/TransactionStatusIndicator";
import { AuthButton } from "@/components/AuthButton";
import { NotificationBell } from "@/components/notifications/NotificationBell";
import { NetworkMismatchBanner } from "@/components/NetworkMismatchBanner";
import { NotificationsProvider } from "@/context/NotificationsContext";
import Image from "next/image";
import clsx from "clsx";

export default function ClientRoot({
  children,
}: {
  children: React.ReactNode;
}) {
  const [isSidebarCollapsed, setIsSidebarCollapsed] = useState(false);
  return (
    <NotificationsProvider>
    <div className="flex h-screen bg-background text-foreground">
      <GameSidebar
        collapsed={isSidebarCollapsed}
        setCollapsed={setIsSidebarCollapsed}
      />
      <main
        id="main-content"
        role="main"
        className={clsx(
          "flex-1 overflow-auto transition-all duration-300",
          isSidebarCollapsed ? "md:ml-16" : "md:ml-64"
        )}
      >
        <div className="md:hidden flex items-center p-4 border-b border-border">
          <GameSidebar
            isMobileView={true}
            collapsed={false}
            setCollapsed={() => {}}
          />
          <div className="ml-4 flex items-center">
            <div className="h-16 w-16 relative">
              <Image
                src="/images/KnightVerseLogo.png"
                alt="KnightVerse"
                width={64}
                height={64}
                className="object-contain"
                priority
              />
            </div>
          </div>
        </div>
        <div className="container mx-auto p-4 md:p-8">
          <div className="mb-2 flex items-center justify-end gap-2">
            <NotificationBell />
            <AuthButton />
          </div>
          <NetworkMismatchBanner />
          {children}
        </div>
      </main>
      <TransactionStatusIndicator />
    </div>
    </NotificationsProvider>
  );
}
