# RecallPet

A local-first macOS menu-bar clipboard history tool with a small animated
avatar. This is the **Phase 0 technical spike**: the smallest useful
end-to-end product, not the full app described in `ROADMAP.md`.

## Phase 0 scope

RecallPet Phase 0 does exactly this and nothing more:

- Lives in the menu bar (no Dock icon) with an animated avatar.
- Clicking the tray icon opens a compact popover below it.
- Polls the clipboard (`NSPasteboard.changeCount`) once a second and
  stores plain-text copies in a local SQLite database.
- Deduplicates by content hash (line-ending-normalized SHA-256): repeat
  copies bump a counter and move the item to the top instead of creating
  new rows.
- Shows recent items with search (SQLite FTS5, with a safe fallback).
- Lets you pause collection and delete all stored data.
- Persists everything across app restarts.

**No browser history, no command capture, no AI, and no secret redaction
exist in Phase 0. Clipboard text is stored raw and unencrypted, locally,
in a SQLite file. There is no network access of any kind.** Privacy
controls (secret detection, redaction, encryption) are a deliberate,
recorded Phase 4 scope decision — see `docs/architecture.md` and the
`AIDEV-TODO: Phase 4 redaction hook` marker in `models.rs`/`lib.rs`.

## Architecture

See `docs/architecture.md` for the full module map and data flow diagram.
Short version: a Rust background thread polls the pasteboard, normalizes
and hashes new text, and upserts it into SQLite inside a transaction. A
vanilla HTML/CSS/JS popover (no framework, no build step) talks to Rust
through Tauri's `invoke`/`listen` and renders the recent list.

## Prerequisites

- macOS (Phase 0 targets macOS only; it will not build a working app on
  other platforms — see "Non-macOS builds" below).
- [Rust](https://rustup.rs/) (stable toolchain).
- [Node.js](https://nodejs.org/) 18+ and npm (only used to run the Tauri
  CLI; the frontend itself has no build step).
- Xcode Command Line Tools (`xcode-select --install`).

## Install / dev / build

```bash
npm install          # installs the pinned @tauri-apps/cli
./scripts/dev.sh      # tauri dev - hot reloads the frontend
./scripts/build.sh    # tauri build - release .app/.dmg under
                       # src-tauri/target/release/bundle/
```

Run the Rust test suite from `src-tauri/`:

```bash
cd src-tauri
cargo test
```

## Data location

```
~/Library/Application Support/RecallPet/recallpet.db
```

Resolved via Tauri's path API (`app.path().app_data_dir()`), never
hardcoded elsewhere. SQLite runs in WAL mode, so you'll also see
`recallpet.db-wal` and `recallpet.db-shm` alongside it during normal use.

## Pause, delete, and reset

- **Pause** (popover button): stops all clipboard writes; the paused
  state itself persists across restarts.
- **Delete all** (popover button, with confirmation): permanently removes
  every stored item and its search index entries.
- **Open data folder** (popover button): reveals the data directory above
  in Finder.
- **Full reset from the terminal**: `./scripts/reset-local-data.sh` prints
  the exact path it's about to delete and asks for confirmation before
  removing the entire data directory. Quit RecallPet first.

## Uninstall

1. Quit RecallPet (right-click the tray icon → Quit RecallPet).
2. Delete the `.app` bundle.
3. Optionally run `./scripts/reset-local-data.sh` to remove stored data,
   or manually delete `~/Library/Application Support/RecallPet`.

## Known limitations (Phase 0)

- Plain text only - no images, files, or rich text.
- No secret detection/redaction/encryption (deliberate Phase 0 decision;
  see above).
- No cloud sync, no network access, no AI features.
- Tray icons and the app icon are simple placeholder artwork generated for
  this spike, not final branding.
- No non-macOS support: the pasteboard backend is macOS-only by design; a
  mock backend is compiled in on other platforms purely so the Rust crate
  can be built/tested outside macOS (see `HANDOFF.md`).

## Verification

- Automated Rust tests: `cd src-tauri && cargo test` (20 tests covering
  normalization, dedup, pause/resume, delete-all, FTS sync, self-write
  suppression, and restart persistence).
- `docs/phase-0-results.md` is a human verification checklist for
  everything that needs a real Mac, a display, and Activity Monitor
  (tray click behavior, popover positioning, Reduce Motion, idle CPU,
  etc.) - it is intentionally left unchecked; nobody has run it on real
  hardware yet.
