"use client";

import { useMemo, useState } from "react";
import { Bell } from "lucide-react";
import { useAuth } from "@/context/authContext";
import { useNotifications, type NotificationKind } from "@/context/NotificationsContext";
import { NotificationItem } from "@/components/notifications/NotificationItem";

type Filter = "all" | "unread" | NotificationKind;

const FILTERS: { id: Filter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "unread", label: "Unread" },
  { id: "challenge", label: "Challenges" },
  { id: "tournament_round", label: "Tournaments" },
  { id: "payout", label: "Payouts" },
];

export default function NotificationsPage() {
  const { isAuthenticated } = useAuth();
  const { notifications, unreadCount, markAllRead, isLoading, error, isLive, refresh } =
    useNotifications();
  const [filter, setFilter] = useState<Filter>("all");

  const visible = useMemo(() => {
    if (filter === "all") return notifications;
    if (filter === "unread") return notifications.filter((n) => !n.read);
    return notifications.filter((n) => n.kind === filter);
  }, [notifications, filter]);

  if (!isAuthenticated) {
    return (
      <div className="max-w-3xl mx-auto py-16 text-center">
        <Bell className="mx-auto mb-3 h-10 w-10 text-gray-500" aria-hidden="true" />
        <h1 className="text-2xl font-bold text-white">Notifications</h1>
        <p className="mt-2 text-sm text-gray-400">Sign in to see your notifications.</p>
      </div>
    );
  }

  return (
    <div className="max-w-3xl mx-auto" role="region" aria-label="Notification inbox">
      <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-3xl font-bold text-white">Notifications</h1>
          <p className="text-sm text-gray-400">
            {unreadCount} unread ·{" "}
            <span className={isLive ? "text-emerald-400" : "text-gray-500"}>
              {isLive ? "Live" : "Offline"}
            </span>
          </p>
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => void refresh()}
            className="px-3 py-2 text-sm rounded-xl bg-gray-700/60 text-gray-200 hover:bg-gray-600/60"
          >
            Refresh
          </button>
          <button
            onClick={() => void markAllRead()}
            disabled={unreadCount === 0}
            className="px-3 py-2 text-sm rounded-xl bg-teal-500/20 border border-teal-500/40 text-teal-200 hover:bg-teal-500/30 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Mark all as read
          </button>
        </div>
      </div>

      <div className="mb-4 flex flex-wrap gap-2" role="tablist" aria-label="Filter notifications">
        {FILTERS.map((f) => (
          <button
            key={f.id}
            role="tab"
            aria-selected={filter === f.id}
            onClick={() => setFilter(f.id)}
            className={`px-3 py-1.5 rounded-xl text-sm font-medium border ${
              filter === f.id
                ? "bg-emerald-500/20 border-emerald-500/50 text-emerald-300"
                : "bg-gray-800/60 border-gray-700/50 text-gray-300 hover:text-white"
            }`}
          >
            {f.label}
          </button>
        ))}
      </div>

      <div className="rounded-xl border border-gray-700/50 bg-gray-800/40 p-2" aria-busy={isLoading}>
        {isLoading && notifications.length === 0 ? (
          <p className="p-6 text-center text-sm text-gray-500" role="status">Loading notifications…</p>
        ) : error && notifications.length === 0 ? (
          <p className="p-6 text-center text-sm text-red-300" role="alert">{error}</p>
        ) : visible.length === 0 ? (
          <p className="p-6 text-center text-sm text-gray-500">No notifications here.</p>
        ) : (
          <ul aria-label="Notifications" className="space-y-1">
            {visible.map((n) => (
              <NotificationItem key={n.id} notification={n} />
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
