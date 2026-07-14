-- RecallPet Phase 0 initial schema.
-- AIDEV-NOTE: Phase 4 will add a redaction/encryption hook; do not add detection logic here.

CREATE TABLE clipboard_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content_hash TEXT NOT NULL UNIQUE,
    text TEXT NOT NULL,
    preview TEXT NOT NULL,
    character_count INTEGER NOT NULL,
    duplicate_count INTEGER NOT NULL DEFAULT 1,
    first_seen_at INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL,
    pinned INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_clipboard_last_seen ON clipboard_items(last_seen_at DESC);

CREATE VIRTUAL TABLE clipboard_items_fts USING fts5(
    text, preview,
    content='clipboard_items', content_rowid='id'
);

CREATE TRIGGER clipboard_ai AFTER INSERT ON clipboard_items BEGIN
  INSERT INTO clipboard_items_fts(rowid, text, preview) VALUES (new.id, new.text, new.preview);
END;

CREATE TRIGGER clipboard_ad AFTER DELETE ON clipboard_items BEGIN
  INSERT INTO clipboard_items_fts(clipboard_items_fts, rowid, text, preview) VALUES ('delete', old.id, old.text, old.preview);
END;

CREATE TRIGGER clipboard_au AFTER UPDATE ON clipboard_items BEGIN
  INSERT INTO clipboard_items_fts(clipboard_items_fts, rowid, text, preview) VALUES ('delete', old.id, old.text, old.preview);
  INSERT INTO clipboard_items_fts(rowid, text, preview) VALUES (new.id, new.text, new.preview);
END;

-- Small key/value table for app-level settings that must survive restarts
-- (e.g. the paused/resumed collection state).
CREATE TABLE app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
