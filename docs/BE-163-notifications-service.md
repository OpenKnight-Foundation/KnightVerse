# BE-163: Notifications Service

## Purpose

Players currently receive no in-app notice when a challenge arrives, a
tournament round starts, an opponent makes a correspondence-game move, or a
payout settles. BE-163 adds persisted notifications and pushes new notices to
connected clients.

## Scope

- Add the notifications service in
  `backend/modules/service/src/notifications.rs`.
- Persist notifications and their read state.
- Expose `GET /v1/notifications` to retrieve notifications and
  `POST /v1/notifications/read` to mark notifications as read.
- Push newly created notifications to connected clients over WebSocket.
- Emit notifications from matchmaking, tournaments, and game completion.

## Notification lifecycle

1. A supported application event creates a notification for its recipient.
2. The notification is persisted with an unread state.
3. If the recipient is connected, the new notification is pushed over the
   existing WebSocket connection.
4. The recipient can retrieve notifications through the HTTP API and mark
   them as read.

Persistence is the source of truth; delivery over WebSocket is an additional
real-time path for connected clients.

## API

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/v1/notifications` | Retrieve the player's notifications and read state. |
| `POST` | `/v1/notifications/read` | Mark notifications as read. |

Request and response payloads, pagination, and the WebSocket message shape
should follow the existing backend API and WebSocket conventions when
implemented.

## Acceptance checklist

- [ ] Notifications are persisted with a read state.
- [ ] `GET /v1/notifications` returns the player's notifications.
- [ ] `POST /v1/notifications/read` updates read state.
- [ ] New notifications are pushed to connected clients.
- [ ] Matchmaking, tournament, and game-completion flows emit notifications.

## Related context

The triggering events should cover the player-facing cases described in the
issue: incoming challenges, tournament round starts, correspondence-game
moves, and settled payouts. The issue's implementation location also calls for
a database migration and WebSocket push support.
