//! Data shapes and the pure text-processing rules Phase 0 relies on:
//! line-ending normalization, size limits, content hashing, and previews.

use serde::Serialize;
use sha2::{Digest, Sha256};

/// Content longer than this (in `char`s, not bytes) is rejected outright.
pub const MAX_CHARACTERS: usize = 100_000;

/// Length of the single-line preview shown in the recent list.
pub const PREVIEW_LENGTH: usize = 120;

/// A stored clipboard entry, as persisted in `clipboard_items`.
///
/// Mirrors the full row; a few fields (`content_hash`, `text`,
/// `created_at`, `updated_at`) are only consumed by tests and
/// `Database::item_text` today, not by `ClipboardItemSummary` conversion.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ClipboardItem {
    pub id: i64,
    pub content_hash: String,
    pub text: String,
    pub preview: String,
    pub character_count: i64,
    pub duplicate_count: i64,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
    pub pinned: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// What the frontend actually receives: no raw `text`, so full clipboard
/// contents never cross the IPC boundary except via `copy_item_again`,
/// which writes straight back to the pasteboard from Rust.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardItemSummary {
    pub id: i64,
    pub preview: String,
    pub character_count: i64,
    pub duplicate_count: i64,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
    pub pinned: bool,
}

impl From<&ClipboardItem> for ClipboardItemSummary {
    fn from(item: &ClipboardItem) -> Self {
        ClipboardItemSummary {
            id: item.id,
            preview: item.preview.clone(),
            character_count: item.character_count,
            duplicate_count: item.duplicate_count,
            first_seen_at: item.first_seen_at,
            last_seen_at: item.last_seen_at,
            pinned: item.pinned,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionStatus {
    pub paused: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStats {
    pub total_items: i64,
    pub total_duplicates: i64,
    pub db_size_bytes: i64,
    pub oldest_item_at: Option<i64>,
    pub newest_item_at: Option<i64>,
}

/// Why a captured clipboard string was not turned into a stored item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    EmptyOrWhitespace,
    TooLarge,
}

/// Normalizes line endings to `\n` (handles `\r\n` and lone `\r`).
pub fn normalize_line_endings(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            normalized.push('\n');
        } else {
            normalized.push(c);
        }
    }
    normalized
}

/// Applies the Phase 0 capture rules: normalize, then reject empty/whitespace
/// or oversized content. Returns the normalized text ready for hashing and
/// storage.
pub fn prepare_capture(raw: &str) -> Result<String, RejectReason> {
    let normalized = normalize_line_endings(raw);
    if normalized.trim().is_empty() {
        return Err(RejectReason::EmptyOrWhitespace);
    }
    if normalized.chars().count() > MAX_CHARACTERS {
        return Err(RejectReason::TooLarge);
    }
    Ok(normalized)
}

/// SHA-256 of the normalized text, hex-encoded. Used as the dedup key.
pub fn content_hash(normalized_text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(normalized_text.as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// First ~120 chars of `normalized_text`, collapsed onto a single line.
pub fn make_preview(normalized_text: &str) -> String {
    let single_line: String = normalized_text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let mut preview: String = single_line.chars().take(PREVIEW_LENGTH).collect();
    if single_line.chars().count() > PREVIEW_LENGTH {
        preview.push('\u{2026}'); // ellipsis
    }
    preview
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_crlf_and_lone_cr() {
        assert_eq!(normalize_line_endings("a\r\nb\rc\n"), "a\nb\nc\n");
    }

    #[test]
    fn rejects_whitespace_only() {
        assert_eq!(
            prepare_capture("   \n\t  "),
            Err(RejectReason::EmptyOrWhitespace)
        );
        assert_eq!(prepare_capture(""), Err(RejectReason::EmptyOrWhitespace));
    }

    #[test]
    fn rejects_oversized_content() {
        let huge = "a".repeat(MAX_CHARACTERS + 1);
        assert_eq!(prepare_capture(&huge), Err(RejectReason::TooLarge));
        let exactly_max = "a".repeat(MAX_CHARACTERS);
        assert!(prepare_capture(&exactly_max).is_ok());
    }

    #[test]
    fn crlf_and_lf_variants_share_a_hash() {
        let a = prepare_capture("hello\r\nworld").unwrap();
        let b = prepare_capture("hello\nworld").unwrap();
        assert_eq!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn preview_truncates_and_collapses_whitespace() {
        let text = format!("{}\nmore text", "a".repeat(200));
        let preview = make_preview(&text);
        assert_eq!(preview.chars().count(), PREVIEW_LENGTH + 1); // + ellipsis
        assert!(preview.ends_with('\u{2026}'));

        let short = make_preview("line one\nline two");
        assert_eq!(short, "line one line two");
    }
}
