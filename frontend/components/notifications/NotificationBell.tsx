"use client";

import { useEffect, useId, useRef, useState } from "react";
import Link from "next/link";
import { Bell } from "lucide-react";
import { useAuth } from "@/context/authContext";
import { useNotifications } from "@/context/NotificationsContext";
import { NotificationItem } from "./NotificationItem";

const DROPDOWN_LIMIT = 8;

export function NotificationBell() {
  const { isAuthenticated } = useAuth();
  const { notifications, unreadCount, markAllRead, isLoading, error } = useNotifications();
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const panelId = useId();

  // Close on outside click / Escape.
  useEffect(() => {
    if (!open) return;
    const onPointer = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setOpen(false);
        buttonRef.current?.focus();
      }
    };
    document.addEventListener("mousedown", onPointer);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onPointer);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  if (!isAuthenticated) return null;

  const badge = unreadCount > 99 ? "99+" : String(unreadCount);
  const label = unreadCount > 0 ? `Notifications, ${unreadCount} unread` : "Notifications";

  return (
    <div ref={rootRef} className="relative">
      <button
        ref={buttonRef}
        onClick={() => setOpen((o) => !o)}
        aria-label={label}
        aria-haspopup="true"
        aria-expanded={open}
        aria-controls={panelId}
        className="relative rounded-full p-2 text-gray-300 hover:bg-gray-800/60 hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-400"
      >
        <Bell className="h-5 w-5" aria-hidden="true" />
        {unreadCount > 0 && (
          <span
            className="absolute -right-0.5 -top-0.5 min-w-[18px] rounded-full bg-red-500 px-1 text-center text-[10px] font-bold leading-[18px] text-white"
            aria-hidden="true"
          >
            {badge}
          </span>
        )}
      </button>

      {open && (
        <div
          id={panelId}
          role="region"
          aria-label="Recent notifications"
          className="absolute right-0 z-50 mt-2 w-80 max-w-[calc(100vw-2rem)] rounded-xl border border-gray-700/50 bg-gray-900/95 shadow-2xl backdrop-blur"
        >
          <div className="flex items-center justify-between border-b border-gray-700/50 px-4 py-3">
            <h2 className="text-sm font-semibold text-white">Notifications</h2>
            {unreadCount > 0 && (
              <button
                onClick={() => void markAllRead()}
                className="text-xs text-teal-300 hover:text-teal-200 hover:underline"
              >
                Mark all as read
              </button>
            )}
          </div>
          <div className="max-h-96 overflow-y-auto p-1">
            {isLoading && notifications.length === 0 ? (
              <p className="p-4 text-center text-xs text-gray-500">Loading…</p>
            ) : error && notifications.length === 0 ? (
              <p className="p-4 text-center text-xs text-red-300">{error}</p>
            ) : notifications.length === 0 ? (
              <p className="p-4 text-center text-xs text-gray-500">You&apos;re all caught up.</p>
            ) : (
              <ul aria-label="Notifications">
                {notifications.slice(0, DROPDOWN_LIMIT).map((n) => (
                  <NotificationItem
                    key={n.id}
                    notification={n}
                    compact
                    onNavigate={() => setOpen(false)}
                  />
                ))}
              </ul>
            )}
          </div>
          <div className="border-t border-gray-700/50 p-2 text-center">
            <Link
              href="/notifications"
              onClick={() => setOpen(false)}
              className="text-xs font-medium text-gray-300 hover:text-white hover:underline"
            >
              View all notifications
            </Link>
          </div>
        </div>
      )}
    </div>
  );
}

export default NotificationBell;
