# RecallPet Phase 0 — Architecture

## Overview

RecallPet is a Tauri v2 menu-bar app. A Rust background thread polls
`NSPasteboard.changeCount` once a second; new plain-text content is
normalized, hashed, and upserted into a local SQLite database (WAL mode,
FTS5 full-text search). A borderless popover window, toggled from the tray
icon, renders the recent list, search box, and pause/delete controls in
vanilla HTML/CSS/JS.

```
┌─────────────────────────────────────────────────────────────┐
│ Tray icon (tray.rs)                                          │
│  - left click: toggle popover (tauri-plugin-positioner)      │
│  - right click: "Quit RecallPet" menu                         │
│  - swaps idle/capture/paused template-image frames            │
└───────────────┬───────────────────────────────────────────────┘
                │ show/hide, avatar:state-changed
┌───────────────▼───────────────────────────────────────────────┐
│ Popover webview (src/index.html, app.js, avatar.js)           │
│  - invoke() the command API                                   │
│  - listen() for clipboard:*, collector:*, storage:*, avatar:*│
└───────────────┬───────────────────────────────────────────────┘
                │ tauri::command (commands.rs)
┌───────────────▼───────────────────────────────────────────────┐
│ AppState (app_state.rs): db handle (Option), paused flag,      │
│ ClipboardController (writes for copy_item_again)               │
└───────────────┬─────────────────────┬─────────────────────────┘
                │                     │
┌───────────────▼─────────┐ ┌────────▼──────────────────────────┐
│ Database (database.rs)   │ │ Poll loop (lib.rs::spawn_poll_loop)│
│  - WAL, FK, busy_timeout │ │  - 1 background OS thread          │
│  - upsert_capture (tx)   │ │  - ClipboardPoller (clipboard.rs)  │
│  - recent/search/stats   │ │  - self-write suppression          │
│  - migrations.rs runner  │ │  - NSPasteboard via objc2 (macOS)  │
└───────────────────────────┘ └─────────────────────────────────────┘
```

## Module map

| File | Responsibility |
|---|---|
| `main.rs` | Thin entry point, calls `recallpet_lib::run()`. |
| `lib.rs` | Builds the Tauri app, resolves the DB path, wires state, spawns/stops the poll thread. |
| `app_state.rs` | `AppState`: `Option<Arc<Database>>`, paused flag, `ClipboardController`. |
| `clipboard.rs` | `ClipboardBackend` trait, `ClipboardPoller` (self-write suppression logic), `ClipboardController`, the macOS `NSPasteboard` backend. Fully unit-testable without macOS. |
| `database.rs` | Single `Mutex<Connection>`, WAL/FK/busy-timeout setup, upsert-in-a-transaction dedup, recent/search/stats/pause queries. |
| `migrations.rs` | Versioned migration runner (`schema_migrations` table). |
| `models.rs` | `ClipboardItem` / `ClipboardItemSummary`, normalization, SHA-256 hashing, preview generation. |
| `commands.rs` | The 8 Tauri commands, argument validation, structured `CommandError`. |
| `tray.rs` | Tray icon, popover show/hide + focus-loss auto-hide, avatar icon swapping, Quit menu. |

## Data flow: a single capture

1. Poll thread calls `ClipboardPoller::poll_once()` every 1000ms.
2. If `changeCount` is unchanged, or it matches a pending self-write, or
   collection is paused: no-op.
3. Otherwise the pasteboard's string is read and passed through
   `models::prepare_capture` (line-ending normalization, empty/whitespace
   rejection, 100,000-character cap).
   - **AIDEV-TODO: Phase 4 redaction hook** — this is the single point
     where captured text enters persistence; Phase 4 will insert secret
     detection/redaction here. Phase 0 deliberately does neither.
4. `Database::upsert_capture` runs a transaction: look up by
   `content_hash` (SHA-256 of the normalized text); update
   `duplicate_count`/`last_seen_at` on a hit, insert a new row otherwise.
5. `clipboard:item-created` or `clipboard:item-updated` is emitted with a
   `ClipboardItemSummary` (no raw text) and the tray briefly shows the
   "capture" icon frame.

## Self-write suppression

`copy_item_again` writes through `ClipboardController`, which records the
pasteboard's `changeCount` immediately after the write into a shared
`Arc<Mutex<Option<i64>>>`. The next `poll_once()` that observes that exact
`changeCount` treats it as `PollOutcome::SelfWrite` and returns without
reading/storing anything. See `clipboard.rs` tests for the exact sequencing
this depends on.

## Why a plain OS thread instead of async/tokio

The poll loop is a `std::thread::spawn` with `std::thread::sleep(1s)`
between ticks, not a tokio task. Phase 0 has no other need for async, and a
sleeping OS thread already satisfies "idle CPU < 1%, the poll timer is the
only recurring background work" without adding an async runtime dependency
beyond what `tauri` itself already pulls in.

## Cross-platform build note

RecallPet only ever ships for macOS (`tauri.conf.json` bundles the macOS
target only). `clipboard.rs`'s real `NsPasteboardBackend` is gated behind
`#[cfg(target_os = "macos")]`; a `MockClipboard` fallback is compiled in on
other targets purely so `cargo check`/`cargo test`/`cargo build` can run in
non-macOS CI/dev sandboxes. See `HANDOFF.md` for what that does and does
not verify.
