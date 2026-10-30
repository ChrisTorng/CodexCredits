# Android ↔ Windows cross-device synchronization design

## Current state (v2.0.12)

**Not implemented.** Signing in to the same ChatGPT account means the **remote ChatGPT
quota is the same service-side state**, but each client fetches it separately and
stores its own sample/event history, preferences, schedules, and device token
locally. Neither client currently publishes its readings to the other.
Android additionally stores samples/events in its native Java background service,
which Rust imports into device-local SQLite. Windows uses only local SQLite.

**Do not simultaneously enable credit-consuming auto schedules or armed Banked
Reset on Android and Windows for the same account until the coordination layer
is implemented.** A fresh quota check on both clients cannot prevent a race:
two clients can both read "inactive" before either triggers.

## Proposed opt-in synchronization

Use a separate owner-controlled Cloudflare Worker and a Durable Object *per paired
account*, plus durable history storage (D1). Do **not** attempt to use ChatGPT's
OAuth access token as a general-purpose Cloudflare authentication credential and
never upload access/refresh tokens, passwords, or local OS secure-store content.
Same ChatGPT account ID is a matching hint, **not** proof of device ownership.
Pair devices explicitly with a short-lived one-time pairing QR/code and establish
per-device, revocable credentials. Enforce authenticated account and tenant
scoping in every endpoint and never trust arbitrary client-supplied account IDs.
A user without pairing remains completely local/offline as today.

### Historical data: union, not overwrite

- Each installation gets a persistent random `device_id`. Each observation/event
  gets its own immutable `event_id` (UUIDv7/ULID or random UUID plus recorded
  server timestamp); existing rows receive a one-time deterministic import ID
  anchored to device ID + original SQLite ID. Import is idempotent.
- Samples include `observed_at_utc`, `received_at_utc`, device ID, used
  percent (5-hour and weekly), reset timestamps, and the confirmation flag.
  Events include kind, details, related reset-window ID, and provenance.
- Upsert on `(account, device_id, event_id)`; **never** dedupe only by seconds.
  A different device can sample during the same second; two devices can observe
  the *same logical reset or trigger* at different seconds. For graphs, sort
  using a normalized UTC epoch and group near-equivalent measurements only for
  presentation; do not throw away raw records or silently replace the newer
  authoritative server reading with an older peer reading.
- Store a per-device high-water mark, incremental cursors, retention and paging.
  Local SQLite remains a cache with offline reads; initial pairing merges both
  histories without destructive replacement. Account sign-out detaches the
  local device and stops sync without deleting the history unless requested.
- Sync events in addition to quota samples: actual/manual trigger outcomes,
  schedule claims, banked-reset redemption, global resets, and failures.
  Server IDs distinguish a single operation seen by two devices.

### One actual trigger across Android and Windows

- Both the **native Android AlarmManager/JobScheduler path** and the Rust Windows
  scheduler/manual trigger must call the same atomic coordinator *before* a
  potentially credit-consuming operation. Just guarding the Tauri UI is not enough.
- The per-account Durable Object serializes `try_claim(account, operation_id,
  window_id, device_id)`. Use a server-side durable claim, fencing token,
  expiry/lease, idempotency key and a post-operation result record. A competing
  device receives `already_claimed` and only refreshes quota; it never triggers.
- Device A claims → refreshes real ChatGPT quota → if the window is already
  active or unavailable, records skip → otherwise attempts **one** trigger →
  records result → broadcasts changed snapshot/event → each device refreshes
  from ChatGPT independently. Do not share OAuth tokens or raw API responses
  containing sensitive credentials.
- A crash/network loss between remote ChatGPT acceptance and coordinator ack
  is inherently ambiguous (no distributed transaction with ChatGPT). Treat
  unknown outcomes as **do-not-retry**, hold an uncertainty window, and recheck
  the remote 5-hour reset/usage state before any further automatic attempts.
  This is at-most-once attempted execution, not a false promise of exactly-once.
- Banked Reset consumption requires a separate exclusive lease and the same
  failure/ambiguity handling. A one-shot armed setting is not copied as an
  executable command to both devices.
- **Offline policy:** quota sampling/display can continue locally; an automatic
  trigger requiring coordination must fail closed and report why. Do not let
  both offline devices independently trigger. For a local-only account, retain
  the existing non-coordinated behavior but warn against enabling both devices.
- Prefer a single **execution owner** (Android background scheduler as default
  when paired and online; Windows as standby). Automatic failover requires a
  server-visible heartbeat, lease expiry and conservative dedup. No assumption
  that an Android process is continuously running.

### Account settings

Distinguish shared preferences (desired schedule/rules, sampling/retention
policy) from **device-specific** settings (Windows tray behavior, native
notifications, mobile exact alarm permission). A shared schedule is a desired
plan, not permission for every device to execute. Use revision and conflict
resolution with explicit origin/device metadata; never silently turn a local
disabled schedule on. Manual triggering is still explicit user action.

## Acceptance tests before enabling synchronization by default

1. Pair Windows and Android using the same ChatGPT account, verify distinct
   device identities, prevent another account from joining and allow revocation.
2. Seed partially overlapping historical records on both devices; reconcile
   without duplicates, lost updates, inverted percentages or backwards
   chronological plots, including equal and near-equal timestamps.
3. Race 2 simultaneous manual/scheduled claims from 2 devices; assert at most
   one downstream API attempt and one trigger event in the shared history.
4. Kill the winner after issuing the remote request but before ack; assert no
   automatic second attempt when its outcome is uncertain.
5. Simulate network loss, Android Doze/background alarms and Windows sleep/
   resume. No duplicate triggers and no automatic catch-up after uncertainty.
6. Broadcast quota updates quickly, but reconcile each device against current
   authoritative ChatGPT quota; stale peer values never overwrite newer ones.
7. Verify account sign-out, token refresh, revoked device credentials,
   retention, permission scopes and re-pairing across app reinstall.

This document is the design boundary; **the current builds do not yet implement
pairing, backend synchronization, or distributed trigger arbitration**.
