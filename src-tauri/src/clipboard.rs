//! Clipboard polling and the hard self-write-suppression requirement.
//!
//! `ClipboardBackend` isolates the platform pasteboard call from the polling
//! and suppression logic below, so the latter (the part with real bugs to
//! catch) is unit-testable without macOS. The only production backend is
//! [`macos::NsPasteboardBackend`], wired in on `target_os = "macos"`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Abstraction over "the system pasteboard" so the poller can be tested
/// without `NSPasteboard`.
pub trait ClipboardBackend: Send + Sync {
    fn change_count(&self) -> i64;
    fn read_text(&self) -> Option<String>;
    /// Writes `text` and returns the pasteboard's change count immediately
    /// after the write, so the caller can record it for self-write
    /// suppression.
    fn write_text(&self, text: &str) -> i64;
}

#[cfg(target_os = "macos")]
pub mod macos {
    use super::ClipboardBackend;
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
    use objc2_foundation::NSString;

    /// The only production clipboard backend. Talks to `NSPasteboard`
    /// directly, per the Phase 0 hard constraint of native macOS clipboard
    /// access (no `arboard`/cross-platform crate).
    pub struct NsPasteboardBackend;

    impl ClipboardBackend for NsPasteboardBackend {
        fn change_count(&self) -> i64 {
            let pasteboard = NSPasteboard::generalPasteboard();
            pasteboard.changeCount() as i64
        }

        fn read_text(&self) -> Option<String> {
            let pasteboard = NSPasteboard::generalPasteboard();
            let string_type = unsafe { NSPasteboardTypeString };
            pasteboard
                .stringForType(string_type)
                .map(|ns_string| ns_string.to_string())
        }

        fn write_text(&self, text: &str) -> i64 {
            let pasteboard = NSPasteboard::generalPasteboard();
            pasteboard.clearContents();
            let ns_text = NSString::from_str(text);
            let string_type = unsafe { NSPasteboardTypeString };
            pasteboard.setString_forType(&ns_text, string_type);
            pasteboard.changeCount() as i64
        }
    }
}

/// Result of a single poll tick.
#[derive(Debug, PartialEq, Eq)]
pub enum PollOutcome {
    /// `changeCount` did not move; nothing was read.
    Unchanged,
    /// `changeCount` moved because `copy_item_again` wrote to the
    /// pasteboard; this cycle is intentionally skipped.
    SelfWrite,
    /// `changeCount` moved but collection is paused; skipped.
    Paused,
    /// `changeCount` moved but there was no plain-text content.
    NoText,
    /// New clipboard text, ready for normalization and storage.
    Captured(String),
}

/// Polls `NSPasteboard.changeCount` and decides, per tick, whether this is
/// new user content, a self-write to suppress, or nothing.
pub struct ClipboardPoller {
    backend: Arc<dyn ClipboardBackend>,
    last_seen_change_count: i64,
    pending_self_write: Arc<Mutex<Option<i64>>>,
    paused: Arc<AtomicBool>,
}

impl ClipboardPoller {
    pub fn new(
        backend: Arc<dyn ClipboardBackend>,
        pending_self_write: Arc<Mutex<Option<i64>>>,
        paused: Arc<AtomicBool>,
    ) -> Self {
        let last_seen_change_count = backend.change_count();
        ClipboardPoller {
            backend,
            last_seen_change_count,
            pending_self_write,
            paused,
        }
    }

    pub fn poll_once(&mut self) -> PollOutcome {
        let current = self.backend.change_count();
        if current == self.last_seen_change_count {
            return PollOutcome::Unchanged;
        }
        self.last_seen_change_count = current;

        {
            let mut pending = self.pending_self_write.lock().unwrap();
            if *pending == Some(current) {
                *pending = None;
                return PollOutcome::SelfWrite;
            }
        }

        if self.paused.load(Ordering::SeqCst) {
            return PollOutcome::Paused;
        }

        match self.backend.read_text() {
            Some(text) if !text.is_empty() => PollOutcome::Captured(text),
            _ => PollOutcome::NoText,
        }
    }
}

/// Writes to the pasteboard on behalf of the app (`copy_item_again`) and
/// records the resulting change count so the next poll tick suppresses it.
pub struct ClipboardController {
    backend: Arc<dyn ClipboardBackend>,
    pending_self_write: Arc<Mutex<Option<i64>>>,
}

impl ClipboardController {
    pub fn new(
        backend: Arc<dyn ClipboardBackend>,
        pending_self_write: Arc<Mutex<Option<i64>>>,
    ) -> Self {
        ClipboardController {
            backend,
            pending_self_write,
        }
    }

    pub fn copy_text(&self, text: &str) {
        let new_change_count = self.backend.write_text(text);
        *self.pending_self_write.lock().unwrap() = Some(new_change_count);
    }
}

/// In-memory pasteboard stand-in. Always available in tests; also compiled
/// in on non-macOS targets as `run()`'s fallback backend, purely so the app
/// crate can be built/checked on non-macOS dev machines. RecallPet only ever
/// ships for macOS (see `tauri.conf.json`), so this path never runs for real.
#[cfg(any(test, not(target_os = "macos")))]
pub mod mock {
    use super::ClipboardBackend;
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct MockClipboard {
        change_count: AtomicI64,
        text: Mutex<Option<String>>,
    }

    impl MockClipboard {
        pub fn new() -> Self {
            Self::default()
        }

        /// Simulates a user pressing Cmd+C: bumps the change count and sets
        /// the new text, exactly as `NSPasteboard` would from outside the app.
        #[cfg(test)]
        pub fn simulate_user_copy(&self, text: &str) -> i64 {
            *self.text.lock().unwrap() = Some(text.to_string());
            self.change_count.fetch_add(1, Ordering::SeqCst) + 1
        }
    }

    impl ClipboardBackend for MockClipboard {
        fn change_count(&self) -> i64 {
            self.change_count.load(Ordering::SeqCst)
        }

        fn read_text(&self) -> Option<String> {
            self.text.lock().unwrap().clone()
        }

        fn write_text(&self, text: &str) -> i64 {
            *self.text.lock().unwrap() = Some(text.to_string());
            self.change_count.fetch_add(1, Ordering::SeqCst) + 1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::mock::MockClipboard;
    use super::*;

    fn poller(
        backend: Arc<MockClipboard>,
    ) -> (ClipboardPoller, Arc<Mutex<Option<i64>>>, Arc<AtomicBool>) {
        let pending = Arc::new(Mutex::new(None));
        let paused = Arc::new(AtomicBool::new(false));
        let poller = ClipboardPoller::new(backend, pending.clone(), paused.clone());
        (poller, pending, paused)
    }

    #[test]
    fn unchanged_pasteboard_is_skipped() {
        let backend = Arc::new(MockClipboard::new());
        let (mut poller, _, _) = poller(backend);
        assert_eq!(poller.poll_once(), PollOutcome::Unchanged);
    }

    #[test]
    fn user_copy_is_captured() {
        // The poller must already be running before the copy happens: content
        // that was on the pasteboard at startup is not retroactively captured.
        let backend = Arc::new(MockClipboard::new());
        let (mut poller, _, _) = poller(backend.clone());
        backend.simulate_user_copy("hello from the user");
        assert_eq!(
            poller.poll_once(),
            PollOutcome::Captured("hello from the user".to_string())
        );
        assert_eq!(poller.poll_once(), PollOutcome::Unchanged);
    }

    #[test]
    fn self_write_is_suppressed_but_next_user_copy_is_captured() {
        let backend = Arc::new(MockClipboard::new());
        let (mut poller, pending, _) = poller(backend.clone());

        // App writes to the pasteboard (as `copy_item_again` would).
        let controller = ClipboardController::new(backend.clone(), pending);
        controller.copy_text("app-authored text");

        assert_eq!(poller.poll_once(), PollOutcome::SelfWrite);

        // A subsequent real user copy must still be captured normally.
        backend.simulate_user_copy("real user text");
        assert_eq!(
            poller.poll_once(),
            PollOutcome::Captured("real user text".to_string())
        );
    }

    #[test]
    fn paused_skips_capture_but_resume_captures_new_copies() {
        let backend = Arc::new(MockClipboard::new());
        let (mut poller, _, paused) = poller(backend.clone());

        paused.store(true, Ordering::SeqCst);
        backend.simulate_user_copy("copied while paused");
        assert_eq!(poller.poll_once(), PollOutcome::Paused);

        paused.store(false, Ordering::SeqCst);
        // The item copied while paused is not retroactively captured.
        assert_eq!(poller.poll_once(), PollOutcome::Unchanged);

        backend.simulate_user_copy("copied after resume");
        assert_eq!(
            poller.poll_once(),
            PollOutcome::Captured("copied after resume".to_string())
        );
    }
}
