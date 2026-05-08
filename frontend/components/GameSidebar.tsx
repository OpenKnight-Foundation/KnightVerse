"use client";

import type React from "react"
import Image from "next/image"
import Link from "next/link"
import { usePathname } from "next/navigation"
import { Button } from "@/components/ui/button"
import { Sheet, SheetContent, SheetTrigger } from "@/components/ui/sheet"
import { useState, useEffect } from "react"
import { WalletConnectModal } from "./WalletConnectModal"
import { useAppContext } from "@/context/walletContext"
import {
  ChessIcon,
  DashboardIcon,
  WatchIcon,
  NewsIcon,
  UserIcon,
  SettingsIcon,
  SupportIcon,
  WalletIcon,
  MenuIcon,
  CollapseIcon,
} from "@/components/icons"

interface SidebarProps {
  collapsed: boolean;
  setCollapsed: (collapsed: boolean) => void;
  isMobileView?: boolean;
}

export function GameSidebar({
  collapsed: propCollapsed,
  setCollapsed,
  isMobileView = false,
}: SidebarProps) {
  const [isHovered, setIsHovered] = useState(false);
  const [collapsed, setLocalCollapsed] = useState(propCollapsed);
  const [isWalletModalOpen, setIsWalletModalOpen] = useState(false);
  const { address, status } = useAppContext();
  const pathname = usePathname();

  useEffect(() => {
    setCollapsed(collapsed);
  }, [collapsed, setCollapsed]);
  
  useEffect(() => setLocalCollapsed(propCollapsed), [propCollapsed]);

  const truncateAddress = (addr: string) => {
    return `${addr.slice(0, 6)}...${addr.slice(-4)}`;
  };

  // For mobile view, we'll use a Sheet component
  if (isMobileView) {
    return (
      <>
        <Sheet>
          <SheetTrigger asChild>
            <Button variant="ghost" size="icon" className="md:hidden">
              <MenuIcon />
              <span className="sr-only">Toggle menu</span>
            </Button>
          </SheetTrigger>
          <SheetContent side="left" className="p-0 w-64 bg-gray-900 border-r border-gray-800">
            <MobileSidebar />
          </SheetContent>
        </Sheet>
        <WalletConnectModal isOpen={isWalletModalOpen} onClose={() => setIsWalletModalOpen(false)} />
      </>
    );
  }

  return (
    <>
      <div
        className={`fixed left-0 top-0 h-full bg-gray-900/95 backdrop-blur-sm border-r border-gray-800/50 flex-col transition-all duration-500 ease-in-out hidden md:flex ${
          collapsed && !isHovered ? "w-16" : "w-64"
        } shadow-xl group z-[50]`}
        role="navigation"
        aria-label="Main Navigation"
        onMouseEnter={() => {
          setIsHovered(true);
          setLocalCollapsed(false);
        }}
        onMouseLeave={() => {
          setIsHovered(false);
          setLocalCollapsed(true);
        }}
      >
        <div className="p-4 flex items-center justify-center overflow-hidden">
          <div
            className={`transition-all duration-500 ease-in-out ${
              collapsed && !isHovered ? "w-16" : "w-full"
            }`}
          >
            <div className="w-16 h-16 relative transform hover:scale-105 transition-transform duration-300">
              <Image
                src="/images/XLMateLogo.png"
                alt="XLMate"
                fill
                className="object-contain drop-shadow-lg"
              />
            </div>
          </div>
        </div>

        <nav className="flex-1 px-2 overflow-hidden">
          <SidebarItem
            icon={<ChessIcon />}
            label="Play"
            href="/"
            collapsed={collapsed && !isHovered}
            active={pathname === "/"}
          />
          <SidebarItem
            icon={<DashboardIcon />}
            label="Dashboard"
            href="/dashboard"
            collapsed={collapsed && !isHovered}
            active={pathname === "/dashboard"}
          />
          <SidebarItem
            icon={<WatchIcon />}
            label="Watch"
            href="/watch"
            collapsed={collapsed && !isHovered}
            active={pathname === "/watch"}
          />
          <SidebarItem
            icon={<NewsIcon />}
            label="News"
            href="/news"
            collapsed={collapsed && !isHovered}
            active={pathname === "/news"}
          />
          <SidebarItem
            icon={<UserIcon />}
            label="Profile"
            href="/profile"
            collapsed={collapsed && !isHovered}
            active={pathname === "/profile"}
          />
          <SidebarItem
            icon={<SettingsIcon />}
            label="Settings"
            href="/settings"
            collapsed={collapsed && !isHovered}
            active={pathname === "/settings"}
          />
          <SidebarItem
            icon={<SupportIcon />}
            label="Support"
            href="/support"
            collapsed={collapsed && !isHovered}
            active={pathname === "/support"}
          />
        </nav>

        <div
          className={`p-4 space-y-3 overflow-hidden transition-all duration-500 ease-in-out ${
            collapsed && !isHovered ? "opacity-0" : "opacity-100"
          }`}
        >
          {status === "connected" && address ? (
            <div className="flex items-center space-x-3 p-3 rounded-xl bg-gray-800/60 border border-gray-700/40">
              <div className="w-8 h-8 rounded-full bg-gradient-to-br from-teal-400 to-blue-600 flex items-center justify-center text-white text-xs font-bold flex-shrink-0">
                {address.slice(0, 2).toUpperCase()}
              </div>
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium text-white truncate">
                  {truncateAddress(address)}
                </p>
                <div className="flex items-center gap-1.5 mt-0.5">
                  <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse-glow" />
                  <span className="text-xs text-emerald-400">Connected</span>
                </div>
              </div>
              <button
                onClick={() => setIsWalletModalOpen(true)}
                className="p-1.5 rounded-lg hover:bg-gray-700/50 text-gray-400 hover:text-white transition-colors"
                aria-label="Manage wallet"
              >
                <svg xmlns="http://www.w3.org/2000/svg" className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
                  <path strokeLinecap="round" strokeLinejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                </svg>
              </button>
            </div>
          ) : (
            <Button
              className="w-full bg-gradient-to-r from-teal-500 to-blue-700 hover:from-teal-600 hover:to-blue-800 text-white shadow-lg hover:shadow-teal-500/20 transition-all duration-300 rounded-lg"
              onClick={() => setIsWalletModalOpen(true)}
            >
              <div className="flex items-center">
                <div className="transform group-hover:scale-110 transition-transform duration-300">
                  <WalletIcon />
                </div>
                <span
                  className={`ml-2 transition-opacity duration-500 ${
                    collapsed && !isHovered ? "opacity-0" : "opacity-100"
                  }`}
                >
                  Connect Wallet
                </span>
              </div>
            </Button>
          )}
        </div>

        <div className="border-t border-gray-800 p-2">
          <button
            onClick={() => setLocalCollapsed(!collapsed)}
            aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
            className="p-2 w-full flex items-center justify-center text-gray-400 hover:text-white hover:bg-gray-800 rounded-md transition-all duration-300 group"
          >
            <div className={`transform transition-transform duration-300 ${collapsed ? "rotate-180" : ""}`}>
              <CollapseIcon />
            </div>
            {!collapsed && (
              <span className="ml-2 transition-opacity duration-300 group-hover:text-teal-400">Collapse</span>
            )}
          </button>
        </div>
      </div>
      <WalletConnectModal isOpen={isWalletModalOpen} onClose={() => setIsWalletModalOpen(false)} />
    </>
  )
}

interface SidebarItemProps {
  icon: React.ReactNode;
  label: string;
  href: string;
  collapsed?: boolean;
  active?: boolean;
}

function SidebarItem({
  icon,
  label,
  href,
  collapsed,
  active,
}: SidebarItemProps) {
  return (
    <Link
      href={href}
      className={`flex items-center gap-4 px-4 py-3 rounded-lg transition-all duration-300 ${
        active
          ? "bg-gray-800/50 text-white"
          : "text-gray-400 hover:text-white hover:bg-gray-800/30"
      }`}
      aria-current={active ? "page" : undefined}
    >
      <span
        className={`${
          active ? "text-teal-400" : "text-gray-300"
        } transition-all duration-300 group-hover:text-teal-400 transform group-hover:scale-110 min-w-[24px]`}
      >
        {icon}
      </span>
      <span
        className={`ml-3 text-sm font-medium text-gray-300 group-hover:text-white transition-all duration-500 ${
          collapsed ? "opacity-0 w-0" : "opacity-100 w-auto"
        } whitespace-nowrap`}
      >
        {label}
      </span>
    </Link>
  );
}

interface MobileSidebarProps {
  className?: string;
}

function MobileSidebar({ className = "" }: MobileSidebarProps) {
  const { address, status } = useAppContext();
  const truncateAddress = (addr: string) => `${addr.slice(0, 6)}...${addr.slice(-4)}`;
  const pathname = usePathname();

  return (
    <div className={`flex flex-col h-full bg-gray-900 ${className}`} role="navigation" aria-label="Main Navigation">
      <div className="p-4 flex items-center space-x-2">
        <div className="w-16 h-16 relative">
          <Image
            src="/images/XLMateLogo.png"
            alt="XLMate"
            fill
            className="object-contain"
          />
        </div>
      </div>
      <nav className="flex-1">
        <MobileSidebarItem icon={<ChessIcon />} label="Play" href="/" active={pathname === "/"} />
        <MobileSidebarItem icon={<DashboardIcon />} label="Dashboard" href="/dashboard" active={pathname === "/dashboard"} />
        <MobileSidebarItem icon={<WatchIcon />} label="Watch" href="/watch" active={pathname === "/watch"} />
        <MobileSidebarItem icon={<NewsIcon />} label="News" href="/news" active={pathname === "/news"} />
        <MobileSidebarItem icon={<UserIcon />} label="Profile" href="/profile" active={pathname === "/profile"} />
        <MobileSidebarItem icon={<SettingsIcon />} label="Settings" href="/settings" active={pathname === "/settings"} />
        <MobileSidebarItem icon={<SupportIcon />} label="Support" href="/support" active={pathname === "/support"} />
      </nav>
      <div className="p-4 space-y-2">
        {status === "connected" && address ? (
          <div className="flex items-center space-x-3 p-3 rounded-xl bg-gray-800/60 border border-gray-700/40">
            <div className="w-8 h-8 rounded-full bg-gradient-to-br from-teal-400 to-blue-600 flex items-center justify-center text-white text-xs font-bold flex-shrink-0">
              {address.slice(0, 2).toUpperCase()}
            </div>
            <div className="flex-1 min-w-0">
              <p className="text-sm font-medium text-white truncate">
                {truncateAddress(address)}
              </p>
              <div className="flex items-center gap-1.5 mt-0.5">
                <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse-glow" />
                <span className="text-xs text-emerald-400">Connected</span>
              </div>
            </div>
          </div>
        ) : (
          <Button
            className="w-full bg-gradient-to-r from-teal-500 to-blue-700 hover:from-teal-600 hover:to-blue-800 text-white"
            aria-label="Connect wallet"
            onClick={() => {}}
          >
            <div className="flex items-center">
              <WalletIcon />
              <span className="ml-2">Connect Wallet</span>
            </div>
          </Button>
        )}
      </div>
    </div>
  );
}

function MobileSidebarItem({ icon, label, href, active }: SidebarItemProps) {
  return (
    <Link
      href={href}
      className={`flex items-center p-3 px-4 hover:bg-gray-800/50 transition-all duration-300 rounded-lg mb-1 group ${
        active ? "bg-gray-800/50 shadow-lg" : ""
      }`}
    >
      <span
        className={`${
          active ? "text-teal-400" : "text-gray-300"
        } transition-all duration-300 group-hover:text-teal-400 transform group-hover:scale-110`}
      >
        {icon}
      </span>
      <span className="ml-3 text-sm font-medium text-gray-300 group-hover:text-white transition-colors duration-300">
        {label}
      </span>
    </Link>
  );
}
