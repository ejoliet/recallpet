# Phase 0 — Human Verification Checklist

This checklist covers everything that needs a person at a real Mac with a
display, a mouse, and Activity Monitor. It was **not** run by the agent that
built this code — this development environment is a headless Linux
container with no macOS, no display server, and no Xcode, so none of the
boxes below have been checked off. See `HANDOFF.md` for exactly what
*was* verified automatically (unit/integration tests, `cargo build`,
`cargo build --release`, all on Linux) and what that does and does not
prove.

Run `./scripts/dev.sh` (or `./scripts/build.sh` then open the built
`.app`) on macOS and work through this list.

## Menu-bar presence

- [ ] The app has no Dock icon and does not appear in Cmd+Tab.
- [ ] A tray icon appears in the macOS menu bar on launch.

## Popover window

- [ ] Left-clicking the tray icon opens a compact popover positioned just
      below the tray icon (~380×540 px).
- [ ] Clicking the tray icon again (while open) hides the popover.
- [ ] Clicking anywhere outside the popover (focus loss) hides it.
- [ ] Closing and reopening the popover preserves state (e.g. scroll
      position / in-progress search text is not reset each time).
- [ ] Right-clicking the tray icon shows a "Quit RecallPet" menu, and it
      actually quits the app.

## Avatar

- [ ] Idle state renders a calm, static avatar face (no animation running).
- [ ] Copying text briefly shows a "capture acknowledged" blink/bounce,
      then returns to idle.
- [ ] Pausing shows a distinct "sleeping" avatar (closed eyes), and it
      stays that way (not idle) until resumed.
- [ ] With **System Settings → Accessibility → Display → Reduce Motion**
      turned on, the capture bounce animation does not play (the icon may
      still swap, but no motion/scaling animation should run).

## Clipboard capture

- [ ] Copying plain text from any app (e.g. TextEdit, Safari, Notes) makes
      it appear at the top of the Recent list within ~1 second.
- [ ] Copying the exact same text again increments its "Copied N×" count
      and moves it back to the top instead of creating a second row.
- [ ] Copying only whitespace, or selecting nothing, does not create an
      entry.
- [ ] Clicking "Copy" on an item puts its text back on the clipboard
      **and does not create a new/duplicate entry** for that action
      (self-write suppression).
- [ ] Copying a very large text selection (well over 100,000 characters —
      e.g. a huge log file) does not create an entry.

## Search

- [ ] Typing in the search box filters the Recent list to matching items.
- [ ] Clearing the search box restores the full recent list.

## Pause / resume

- [ ] Clicking "Pause" changes the status text and avatar, and further
      copies are **not** captured while paused.
- [ ] Clicking "Resume" restores capture of new copies (items copied while
      paused are not retroactively captured).
- [ ] Quitting and relaunching the app while paused keeps it paused.

## Delete all

- [ ] Clicking "Delete all" shows a confirmation before deleting anything.
- [ ] Confirming clears the Recent list immediately and shows the empty
      state.
- [ ] Cancelling the confirmation leaves existing items untouched.

## Persistence & data location

- [ ] Quitting and relaunching the app preserves previously captured
      items.
- [ ] `~/Library/Application Support/RecallPet/recallpet.db` exists after
      first launch, and `recallpet.db-wal` / `-shm` appear during normal
      use (WAL mode).
- [ ] "Open data folder" opens that exact folder in Finder.

## Performance

- [ ] With the popover closed and no clipboard activity, RecallPet's CPU
      usage in Activity Monitor stays under 1% (sampled over ~30s).

## Build

- [ ] `./scripts/build.sh` (`tauri build`) completes successfully on
      macOS and produces a `.app` bundle (and `.dmg`) under
      `src-tauri/target/release/bundle/`.
- [ ] The built `.app` launches standalone (not just via `tauri dev`).

## Accessibility

- [ ] Every control (search box, Pause, Delete all, Open data folder, Copy
      per item, the delete confirmation) is reachable and operable via
      keyboard alone (Tab/Shift+Tab, Enter/Space, Escape to cancel the
      delete confirmation).
- [ ] Light and dark mode (System Settings → Appearance) both render
      legibly.
