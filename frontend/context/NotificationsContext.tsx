"use client";

/**
 * In-app notifications (BE-163): challenges, tournament rounds and payouts.
 *
 * - Loads the inbox over REST (`GET /v1/notifications`).
 * - Receives live updates over a ticket-authenticated WebSocket
 *   (`/v1/ws/notifications`) with exponential-backoff reconnects.
 * - Mark-as-read is optimistic and persisted via REST.
 * - New notifications are announced through a polite ARIA live region.
 */

import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { authFetch, endpoints, requestWebSocketTicket } from "@/lib/api";
import { useAuth } from "@/context/authContext";

export type NotificationKind = "challenge" | "tournament_round" | "payout" | "system";

export interface AppNotification {
  id: string;
  kind: NotificationKind;
  title: string;
  body?: string;
  /** In-app link to open when the notification is clicked. */
  href?: string;
  /** Epoch ms. */
  createdAt: number;
  read: boolean;
}

interface NotificationsContextValue {
  notifications: AppNotification[];
  unreadCount: number;
  isLoading: boolean;
  error: string | null;
  /** True while the live WebSocket is open. */
  isLive: boolean;
  markRead: (id: string) => Promise<void>;
  markAllRead: () => Promise<void>;
  refresh: () => Promise<void>;
}

const NotificationsContext = createContext<NotificationsContextValue | null>(null);

const KINDS: NotificationKind[] = ["challenge", "tournament_round", "payout", "system"];
const MAX_RECONNECT_DELAY = 30_000;

function toKind(v: unknown): NotificationKind {
  const s = String(v ?? "").toLowerCase();
  if (KINDS.includes(s as NotificationKind)) return s as NotificationKind;
  if (s.includes("challenge")) return "challenge";
  if (s.includes("tournament") || s.includes("round")) return "tournament_round";
  if (s.includes("payout") || s.includes("prize") || s.includes("reward")) return "payout";
  return "system";
}

function defaultTitle(kind: NotificationKind): string {
  switch (kind) {
    case "challenge":
      return "New challenge";
    case "tournament_round":
      return "Tournament round starting";
    case "payout":
      return "Payout received";
    default:
      return "Notification";
  }
}

/** Normalises a backend notification payload (snake_case or camelCase). */
export function toNotification(raw: unknown): AppNotification | null {
  if (!raw || typeof raw !== "object") return null;
  const r = raw as Record<string, unknown>;
  const id = typeof r.id === "string" ? r.id : typeof r.id === "number" ? String(r.id) : null;
  if (!id) return null;
  const kind = toKind(r.kind ?? r.type ?? r.category);
  const created = r.createdAt ?? r.created_at ?? r.timestamp;
  const createdAt =
    typeof created === "number"
      ? created < 1e12
        ? created * 1000
        : created
      : typeof created === "string" && !Number.isNaN(Date.parse(created))
        ? Date.parse(created)
        : Date.now();
  const readAt = r.readAt ?? r.read_at;
  return {
    id,
    kind,
    title: typeof r.title === "string" && r.title ? r.title : defaultTitle(kind),
    body:
      typeof r.body === "string" ? r.body : typeof r.message === "string" ? r.message : undefined,
    href:
      typeof r.href === "string"
        ? r.href
        : typeof r.link === "string"
          ? r.link
          : typeof r.url === "string"
            ? r.url
            : undefined,
    createdAt,
    read: r.read === true || r.is_read === true || (readAt != null && readAt !== ""),
  };
}

function parseList(body: unknown): AppNotification[] {
  const list = Array.isArray(body)
    ? body
    : Array.isArray((body as { notifications?: unknown })?.notifications)
      ? (body as { notifications: unknown[] }).notifications
      : Array.isArray((body as { data?: unknown })?.data)
        ? (body as { data: unknown[] }).data
        : [];
  return list
    .map(toNotification)
    .filter((n): n is AppNotification => n !== null)
    .sort((a, b) => b.createdAt - a.createdAt);
}

/** Merges notifications by id, newest first. */
function mergeNotifications(
  current: AppNotification[],
  incoming: AppNotification[],
): AppNotification[] {
  const byId = new Map(current.map((n) => [n.id, n]));
  for (const n of incoming) byId.set(n.id, { ...byId.get(n.id), ...n });
  return [...byId.values()].sort((a, b) => b.createdAt - a.createdAt);
}

export function NotificationsProvider({ children }: { children: React.ReactNode }) {
  const { accessToken, isAuthenticated, refreshToken, logout } = useAuth();
  const [notifications, setNotifications] = useState<AppNotification[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isLive, setIsLive] = useState(false);
  const [announcement, setAnnouncement] = useState("");

  const tokenRef = useRef(accessToken);
  tokenRef.current = accessToken;
  const notificationsRef = useRef(notifications);
  notificationsRef.current = notifications;

  const authDeps = useMemo(
    () => ({ getToken: () => tokenRef.current, refresh: refreshToken, logout }),
    [refreshToken, logout],
  );

  const refresh = useCallback(async () => {
    if (!isAuthenticated) return;
    setIsLoading(true);
    try {
      const res = await authFetch(endpoints.notifications.list(), {}, authDeps);
      if (!res.ok) throw new Error(`Could not load notifications (${res.status})`);
      setNotifications(parseList(await res.json()));
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not load notifications");
    } finally {
      setIsLoading(false);
    }
  }, [isAuthenticated, authDeps]);

  // Initial load / reset on sign-in state change.
  useEffect(() => {
    if (!isAuthenticated) {
      setNotifications([]);
      setError(null);
      return;
    }
    void refresh();
  }, [isAuthenticated, refresh]);

  // Live updates over WebSocket.
  useEffect(() => {
    if (!isAuthenticated) return;
    let ws: WebSocket | null = null;
    let cancelled = false;
    let attempt = 0;
    let timer: ReturnType<typeof setTimeout> | null = null;

    const scheduleReconnect = () => {
      if (cancelled) return;
      const delay = Math.min(1000 * 2 ** attempt, MAX_RECONNECT_DELAY);
      attempt++;
      timer = setTimeout(connect, delay);
    };

    async function connect() {
      if (cancelled) return;
      try {
        const ticket = await requestWebSocketTicket(tokenRef.current);
        if (cancelled) return;
        const url = new URL(endpoints.notifications.ws());
        url.searchParams.set("ticket", ticket);
        ws = new WebSocket(url.toString());
      } catch {
        scheduleReconnect();
        return;
      }

      ws.onopen = () => {
        attempt = 0;
        setIsLive(true);
      };
      ws.onmessage = (event) => {
        let data: Record<string, unknown>;
        try {
          data = JSON.parse(event.data);
        } catch {
          return;
        }
        if (data.type === "notification_read" || data.type === "notifications_read") {
          const ids = Array.isArray(data.ids)
            ? data.ids.map(String)
            : data.id != null
              ? [String(data.id)]
              : null;
          setNotifications((prev) =>
            prev.map((n) => (ids === null || ids.includes(n.id) ? { ...n, read: true } : n)),
          );
          return;
        }
        const payload =
          data.type === "notification" ? (data.notification ?? data.payload ?? data) : null;
        const n = toNotification(payload);
        if (!n) return;
        const isNew = !notificationsRef.current.some((p) => p.id === n.id);
        if (isNew && !n.read) {
          setAnnouncement(`New notification: ${n.title}${n.body ? `. ${n.body}` : ""}`);
        }
        setNotifications((prev) => mergeNotifications(prev, [n]));
      };
      ws.onclose = () => {
        setIsLive(false);
        ws = null;
        scheduleReconnect();
      };
      ws.onerror = () => {
        ws?.close();
      };
    }

    void connect();
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
      setIsLive(false);
      if (ws) {
        ws.onclose = null;
        ws.close();
      }
    };
  }, [isAuthenticated]);

  const markRead = useCallback(
    async (id: string) => {
      const target = notificationsRef.current.find((n) => n.id === id);
      if (!target || target.read) return;
      setNotifications((prev) => prev.map((n) => (n.id === id ? { ...n, read: true } : n)));
      try {
        const res = await authFetch(endpoints.notifications.markRead(id), { method: "POST" }, authDeps);
        if (!res.ok) throw new Error();
      } catch {
        // Roll back so the UI reflects the server.
        setNotifications((prev) => prev.map((n) => (n.id === id ? { ...n, read: false } : n)));
      }
    },
    [authDeps],
  );

  const markAllRead = useCallback(async () => {
    const previous = notificationsRef.current;
    if (!previous.some((n) => !n.read)) return;
    setNotifications((prev) => prev.map((n) => ({ ...n, read: true })));
    try {
      const res = await authFetch(endpoints.notifications.markAllRead(), { method: "POST" }, authDeps);
      if (!res.ok) throw new Error();
    } catch {
      setNotifications(previous);
    }
  }, [authDeps]);

  const unreadCount = useMemo(() => notifications.filter((n) => !n.read).length, [notifications]);

  const value = useMemo(
    () => ({ notifications, unreadCount, isLoading, error, isLive, markRead, markAllRead, refresh }),
    [notifications, unreadCount, isLoading, error, isLive, markRead, markAllRead, refresh],
  );

  return (
    <NotificationsContext.Provider value={value}>
      {children}
      <div role="status" aria-live="polite" aria-atomic="true" className="sr-only">
        {announcement}
      </div>
    </NotificationsContext.Provider>
  );
}

export function useNotifications(): NotificationsContextValue {
  const ctx = useContext(NotificationsContext);
  if (!ctx) throw new Error("useNotifications must be used within NotificationsProvider");
  return ctx;
}

/** Relative time label ("5m ago"). */
export function formatRelativeTime(ts: number, now = Date.now()): string {
  const s = Math.max(0, Math.round((now - ts) / 1000));
  if (s < 60) return "just now";
  const m = Math.round(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h}h ago`;
  const d = Math.round(h / 24);
  return `${d}d ago`;
}
