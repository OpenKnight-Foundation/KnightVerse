"use client";

import { Bell, Coins, Swords, Trophy } from "lucide-react";
import { useRouter } from "next/navigation";
import {
  formatRelativeTime,
  useNotifications,
  type AppNotification,
  type NotificationKind,
} from "@/context/NotificationsContext";

const KIND_ICON: Record<NotificationKind, React.ReactNode> = {
  challenge: <Swords className="h-4 w-4 text-red-300" aria-hidden="true" />,
  tournament_round: <Trophy className="h-4 w-4 text-yellow-300" aria-hidden="true" />,
  payout: <Coins className="h-4 w-4 text-emerald-300" aria-hidden="true" />,
  system: <Bell className="h-4 w-4 text-gray-300" aria-hidden="true" />,
};

const KIND_LABEL: Record<NotificationKind, string> = {
  challenge: "Challenge",
  tournament_round: "Tournament",
  payout: "Payout",
  system: "System",
};

export function NotificationItem({
  notification: n,
  onNavigate,
  compact = false,
}: {
  notification: AppNotification;
  /** Called after the item opens its link (e.g. to close a dropdown). */
  onNavigate?: () => void;
  compact?: boolean;
}) {
  const { markRead } = useNotifications();
  const router = useRouter();

  const open = () => {
    void markRead(n.id);
    if (n.href) {
      router.push(n.href);
      onNavigate?.();
    }
  };

  return (
    <li
      className={`flex gap-3 rounded-lg p-3 ${n.read ? "bg-transparent" : "bg-teal-500/5"} hover:bg-gray-700/30`}
    >
      <div className="mt-0.5 shrink-0">{KIND_ICON[n.kind]}</div>
      <button onClick={open} className="min-w-0 flex-1 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-400 rounded">
        <p className={`text-sm ${n.read ? "text-gray-300" : "font-semibold text-white"}`}>
          {!n.read && <span className="sr-only">Unread: </span>}
          {n.title}
        </p>
        {n.body && (
          <p className={`text-xs text-gray-400 ${compact ? "line-clamp-2" : ""}`}>{n.body}</p>
        )}
        <p className="mt-1 text-[11px] text-gray-500">
          {KIND_LABEL[n.kind]} ·{" "}
          <time dateTime={new Date(n.createdAt).toISOString()}>{formatRelativeTime(n.createdAt)}</time>
        </p>
      </button>
      {!n.read && (
        <button
          onClick={() => void markRead(n.id)}
          className="shrink-0 self-start text-[11px] text-teal-300 hover:text-teal-200 hover:underline"
          aria-label={`Mark "${n.title}" as read`}
        >
          Mark read
        </button>
      )}
    </li>
  );
}
