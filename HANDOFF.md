# HANDOFF — RecallPet Phase 0

Status: **automated tests pass, `cargo build` and `cargo build --release`
both succeed, human verification checklist written.** Read this whole file
before touching the code - it explains what was and wasn't actually
verified.

## Environment this was built in (important)

This was built in a headless Linux container: no macOS, no Xcode, no
display server, no `tauri dev` GUI smoke test. That constrains what could
be verified directly:

- **Verified on Linux, in this sandbox**: `cargo check`, `cargo test`
  (20 tests, all passing), `cargo build`, and `cargo build --release` all
  succeed, using the Linux/GTK/webkit2gtk backend as a stand-in Tauri
  target (`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`,
  `libayatana-appindicator3-dev`, `librsvg2-dev`, `libsoup-3.0-dev` were
  installed to make this possible). This exercises essentially the entire
  codebase - `tauri.conf.json`, `capabilities/default.json`, the icon
  files, `generate_context!`/`generate_handler!` macro expansion, the
  tray/menu/positioner wiring in `tray.rs`, all of `commands.rs`,
  `app_state.rs`, `database.rs`, `migrations.rs`, `models.rs`, and
  `lib.rs` - **except** the one block gated behind
  `#[cfg(target_os = "macos")]`.
- **Never compiled, only hand-verified against docs.rs**: the real
  `NsPasteboardBackend` in `clipboard.rs` (the `objc2`/`objc2-app-kit`
  calls to `NSPasteboard`). I fetched the exact method signatures for
  `generalPasteboard`, `changeCount`, `clearContents`, `setString_forType`,
  `stringForType`, and `NSPasteboardTypeString` from docs.rs for
  `objc2-app-kit` 0.3.2 and wrote the code to match, but it has never been
  compiled or run, because this sandbox has no macOS SDK/frameworks to
  link against.
- **Never run at all**: anything requiring a display (tray click, popover
  positioning, focus-loss hiding, avatar animation, Reduce Motion,
  Activity Monitor CPU sampling, the Quit menu). `docs/phase-0-results.md`
  is an unchecked checklist for exactly this - go through it on a real Mac
  before considering Phase 0 "done" in the field.

**Next step for whoever picks this up next: run it on an actual Mac.**
`./scripts/dev.sh`, then work through `docs/phase-0-results.md` top to
bottom. If the macOS-only pasteboard code has a bug, it will most likely
surface there first.

## Deviations from the spec (and why)

1. **`rusqlite` pinned to `0.39.0`, not the newest `0.40.1`.**
   `rusqlite 0.40.1` depends on `libsqlite3-sys 0.38.1`, whose `build.rs`
   uses the nightly-only `cfg_select!` macro and fails to compile on
   stable Rust (confirmed: `error[E0658]: use of unstable library feature
   'cfg_select'`). `rusqlite 0.39.0` depends on `libsqlite3-sys 0.37.0`,
   which has no such issue and was confirmed to build. All other
   dependencies are pinned to their latest stable versions as of this
   writing.
2. **`identifier` in `tauri.conf.json` is `"RecallPet"`, not a reverse-DNS
   string like `pet.recall.app`.** Tauri's `app.path().app_data_dir()`
   resolves to `<data_dir>/<identifier>`. Using `"RecallPet"` as the
   identifier makes that call resolve to exactly
   `~/Library/Application Support/RecallPet` - the exact path the spec
   requires - via the standard path API, with no extra hardcoded path
   joining. Revisit this (reverse-DNS is conventional for App
   Store/notarized distribution) if RecallPet ever needs to be
   distributed through the Mac App Store.
3. **A right-click "Quit RecallPet" tray menu was added.** Not in the
   spec, but an Accessory-policy app has no Dock icon and no Cmd+Q
   affordance; without some quit path the only way out would be Force
   Quit via Activity Monitor. Left-click still exclusively toggles the
   popover (`show_menu_on_left_click(false)`).
4. **`open_data_folder` is a custom Tauri command**, wrapping
   `tauri_plugin_opener::OpenerExt::opener().reveal_item_in_dir()`
   directly in Rust, rather than exposing the opener plugin's JS bindings
   to the frontend. This keeps the frontend at zero npm dependencies
   (no `@tauri-apps/plugin-opener` package, no bundler) while still
   satisfying "No frontend build step beyond what tauri dev/build
   inherently requires."
5. **The clipboard poll loop is a plain `std::thread` with a 1-second
   `sleep`, not a tokio task.** Nothing else in Phase 0 needs an async
   runtime, so this avoids pulling `tokio` in as a direct dependency
   (`tauri` already depends on it internally, but we never touch it).
   Shutdown is a shared `AtomicBool`, checked once per tick, joined from
   the `RunEvent::Exit` handler.
6. **A `MockClipboard` backend compiles in on non-macOS targets**
   (`clipboard.rs`, `#[cfg(any(test, not(target_os = "macos")))]`) as
   `run()`'s fallback (`make_backend()`). This exists solely so the crate
   can be built/tested outside macOS, as in this sandbox. It is never
   wired into a real build - `tauri.conf.json` only bundles macOS, and
   the real backend is `NsPasteboardBackend`, selected whenever
   `target_os = "macos"`.
7. **App/tray icons are placeholder art**, generated programmatically
   with a small pure-Python PNG encoder (no design tool, no external
   assets) - a blue rounded-square face for the app icon, three
   monochrome template glyphs (open eyes / filled circle / closed eyes)
   for the tray idle/capture/paused frames. Replace before any real
   release.

## What's implemented and tested

All 7 required Tauri commands plus `open_data_folder`
(`get_recent_clipboard_items`, `search_clipboard_items`,
`set_collection_paused`, `get_collection_status`,
`delete_all_clipboard_items`, `copy_item_again`, `get_storage_stats`,
`open_data_folder`), all 6 events
(`clipboard:item-created`, `clipboard:item-updated`, `collector:paused`,
`collector:resumed`, `storage:cleared`, `avatar:state-changed`), the full
schema + FTS5 + triggers, versioned migrations, WAL/FK/busy-timeout, and
the popover UI (search, recent list, pause, delete-all with confirmation,
open data folder, light/dark mode, Reduce-Motion-aware avatar).

20 Rust tests in `src-tauri/src/{models,database,migrations,clipboard}.rs`
cover all 11 required scenarios: plain text storage, whitespace
rejection, duplicate increment + reordering, oversized-content rejection,
pause/resume, delete-all + FTS clearing, restart persistence, FTS search
on a fresh insert, FTS staying in sync through update/delete, self-write
suppression, and CRLF/LF dedup.

## Not implemented / explicitly out of scope

- Everything in `ROADMAP.md`'s later phases (this task never read or
  implemented from it, per instructions) - most notably any secret
  detection/redaction/encryption. The single
  `AIDEV-TODO: Phase 4 redaction hook` marker is in `models.rs` at
  `prepare_capture`, the one point where captured text enters
  persistence.
- Any non-macOS platform support (by design).
- A JS test runner / frontend automated tests - the popover UI is only
  covered by the human checklist in `docs/phase-0-results.md`.

## Permissions introduced

`src-tauri/capabilities/default.json` grants the popover window exactly
one permission set: `core:default`. No plugin-specific permissions
(`opener:*`, `fs:*`, etc.) are granted to the frontend - `open_data_folder`
calls `tauri-plugin-opener`'s Rust API directly from a custom command, so
the plugin's own JS-facing permissions are never exercised. This is the
Phase 0 baseline; later phases (shell/browser transports, Ollama) will
need to add to this file, not silently rely on broader default grants.

## Privacy implications

Everything captured is plain text, stored raw and unencrypted in a local
SQLite file, with no network access anywhere in the app (checked: no
`reqwest`/`ureq`/`http`-client dependency exists in `Cargo.toml`, no
`fetch`/`XMLHttpRequest` in the frontend). Events sent to the frontend
carry previews/ids/timestamps only, never full text (`ClipboardItemSummary`
excludes `text`); the only way full text leaves Rust is
`copy_item_again` writing straight back to the pasteboard. This matches
D1: the raw-storage decision is real debt, explicitly deferred to Phase 4,
not silently forgotten.

## Exact next milestone

Per `ROADMAP.md`, the next milestone is **Phase 1 - shell command capture**
(zsh `preexec`/`precmd` hooks, Unix domain socket + spool-file transport).
Per the roadmap's own instruction ("Agents implement ONE phase at a time
... Never build ahead"), nothing from Phase 1 was started or scaffolded
here. Phase 1 should preserve every Phase 0 behavior in this handoff and
extend `HANDOFF.md` again on completion, per the roadmap's milestone gate.

## Repo state

Branch: `claude/recallpet-phase-0-kz9k5b`. `ROADMAP.md` was added directly
to this branch after the Phase 0 commit (not authored by this task, per
the original instructions to treat it as reference-only).
