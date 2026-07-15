//! Single managed SQLite connection: WAL mode, foreign keys, busy timeout,
//! prepared statements, and the insert-or-update-in-a-transaction dedup path.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};

use crate::migrations;
use crate::models::{content_hash, make_preview, ClipboardItem, StorageStats};

const SETTING_PAUSED: &str = "collector_paused";

pub struct Database {
    conn: Mutex<Connection>,
    path: PathBuf,
}

#[derive(Debug)]
pub enum DbError {
    Sqlite(rusqlite::Error),
    Migration(migrations::MigrationError),
    Io(std::io::Error),
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::Sqlite(e) => write!(f, "database error: {e}"),
            DbError::Migration(e) => write!(f, "migration error: {e}"),
            DbError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for DbError {}

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        DbError::Sqlite(e)
    }
}

impl From<migrations::MigrationError> for DbError {
    fn from(e: migrations::MigrationError) -> Self {
        DbError::Migration(e)
    }
}

pub enum UpsertOutcome {
    Created(ClipboardItem),
    DuplicateUpdated(ClipboardItem),
}

impl Database {
    /// Opens (creating if needed) the database at `path`, configures it, and
    /// applies any pending migrations.
    pub fn open(path: &Path) -> Result<Self, DbError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(DbError::Io)?;
        }

        let mut conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.pragma_update(None, "busy_timeout", 5000)?;

        migrations::run(&mut conn)?;

        Ok(Database {
            conn: Mutex::new(conn),
            path: path.to_path_buf(),
        })
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self, DbError> {
        let mut conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", true)?;
        migrations::run(&mut conn)?;
        Ok(Database {
            conn: Mutex::new(conn),
            path: PathBuf::new(),
        })
    }

    /// Inserts a freshly-captured (already normalized) text, or, if a row
    /// with the same content hash exists, bumps its duplicate count and
    /// `last_seen_at`. Runs entirely inside one transaction.
    pub fn upsert_capture(
        &self,
        normalized_text: &str,
        now_ms: i64,
    ) -> Result<UpsertOutcome, DbError> {
        let hash = content_hash(normalized_text);
        let preview = make_preview(normalized_text);
        let char_count = normalized_text.chars().count() as i64;

        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        let existing_id: Option<i64> = tx
            .query_row(
                "SELECT id FROM clipboard_items WHERE content_hash = ?1",
                params![hash],
                |row| row.get(0),
            )
            .optional()?;

        let outcome = if let Some(id) = existing_id {
            tx.execute(
                "UPDATE clipboard_items
                 SET duplicate_count = duplicate_count + 1, last_seen_at = ?1, updated_at = ?1
                 WHERE id = ?2",
                params![now_ms, id],
            )?;
            UpsertOutcome::DuplicateUpdated(fetch_item(&tx, id)?)
        } else {
            tx.execute(
                "INSERT INTO clipboard_items
                    (content_hash, text, preview, character_count, duplicate_count,
                     first_seen_at, last_seen_at, pinned, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5, 0, ?5, ?5)",
                params![hash, normalized_text, preview, char_count, now_ms],
            )?;
            let id = tx.last_insert_rowid();
            UpsertOutcome::Created(fetch_item(&tx, id)?)
        };

        tx.commit()?;
        Ok(outcome)
    }

    pub fn recent_items(&self, limit: i64) -> Result<Vec<ClipboardItem>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, content_hash, text, preview, character_count, duplicate_count,
                    first_seen_at, last_seen_at, pinned, created_at, updated_at
             FROM clipboard_items
             ORDER BY last_seen_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit], row_to_item)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// FTS5 `MATCH` search, falling back to a safe `LIKE` scan if the query
    /// string is not valid FTS5 syntax.
    pub fn search_items(&self, query: &str, limit: i64) -> Result<Vec<ClipboardItem>, DbError> {
        let conn = self.conn.lock().unwrap();

        let fts_result = (|| -> Result<Vec<ClipboardItem>, rusqlite::Error> {
            let mut stmt = conn.prepare(
                "SELECT c.id, c.content_hash, c.text, c.preview, c.character_count,
                        c.duplicate_count, c.first_seen_at, c.last_seen_at, c.pinned,
                        c.created_at, c.updated_at
                 FROM clipboard_items_fts f
                 JOIN clipboard_items c ON c.id = f.rowid
                 WHERE clipboard_items_fts MATCH ?1
                 ORDER BY rank
                 LIMIT ?2",
            )?;
            let rows = stmt
                .query_map(params![query, limit], row_to_item)?
                .collect();
            rows
        })();

        match fts_result {
            Ok(items) => Ok(items),
            Err(_) => {
                let pattern = format!("%{}%", like_escape(query));
                let mut stmt = conn.prepare(
                    "SELECT id, content_hash, text, preview, character_count, duplicate_count,
                            first_seen_at, last_seen_at, pinned, created_at, updated_at
                     FROM clipboard_items
                     WHERE text LIKE ?1 ESCAPE '\\' OR preview LIKE ?1 ESCAPE '\\'
                     ORDER BY last_seen_at DESC
                     LIMIT ?2",
                )?;
                let rows = stmt
                    .query_map(params![pattern, limit], row_to_item)?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            }
        }
    }

    pub fn item_text(&self, id: i64) -> Result<Option<String>, DbError> {
        let conn = self.conn.lock().unwrap();
        let text = conn
            .query_row(
                "SELECT text FROM clipboard_items WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(text)
    }

    pub fn delete_all(&self) -> Result<(), DbError> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM clipboard_items", [])?;
        tx.commit()?;
        Ok(())
    }

    pub fn is_paused(&self) -> Result<bool, DbError> {
        let conn = self.conn.lock().unwrap();
        let value: Option<String> = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![SETTING_PAUSED],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.as_deref() == Some("1"))
    }

    pub fn set_paused(&self, paused: bool) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![SETTING_PAUSED, if paused { "1" } else { "0" }],
        )?;
        Ok(())
    }

    pub fn storage_stats(&self) -> Result<StorageStats, DbError> {
        let conn = self.conn.lock().unwrap();
        let (total_items, total_duplicates, oldest_item_at, newest_item_at): (
            i64,
            i64,
            Option<i64>,
            Option<i64>,
        ) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(duplicate_count - 1), 0),
                    MIN(first_seen_at), MAX(last_seen_at)
             FROM clipboard_items",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;

        let db_size_bytes = std::fs::metadata(&self.path)
            .map(|m| m.len() as i64)
            .unwrap_or(0);

        Ok(StorageStats {
            total_items,
            total_duplicates,
            db_size_bytes,
            oldest_item_at,
            newest_item_at,
        })
    }
}

fn fetch_item(tx: &rusqlite::Transaction, id: i64) -> Result<ClipboardItem, rusqlite::Error> {
    tx.query_row(
        "SELECT id, content_hash, text, preview, character_count, duplicate_count,
                first_seen_at, last_seen_at, pinned, created_at, updated_at
         FROM clipboard_items WHERE id = ?1",
        params![id],
        row_to_item,
    )
}

fn row_to_item(row: &rusqlite::Row) -> Result<ClipboardItem, rusqlite::Error> {
    Ok(ClipboardItem {
        id: row.get(0)?,
        content_hash: row.get(1)?,
        text: row.get(2)?,
        preview: row.get(3)?,
        character_count: row.get(4)?,
        duplicate_count: row.get(5)?,
        first_seen_at: row.get(6)?,
        last_seen_at: row.get(7)?,
        pinned: row.get::<_, i64>(8)? != 0,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn like_escape(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn stores_plain_text() {
        let db = db();
        match db.upsert_capture("hello world", 100).unwrap() {
            UpsertOutcome::Created(item) => {
                assert_eq!(item.text, "hello world");
                assert_eq!(item.duplicate_count, 1);
            }
            _ => panic!("expected Created"),
        }
        assert_eq!(db.recent_items(10).unwrap().len(), 1);
    }

    #[test]
    fn duplicate_increments_count_and_updates_last_seen() {
        let db = db();
        db.upsert_capture("same text", 100).unwrap();
        let outcome = db.upsert_capture("same text", 200).unwrap();
        match outcome {
            UpsertOutcome::DuplicateUpdated(item) => {
                assert_eq!(item.duplicate_count, 2);
                assert_eq!(item.last_seen_at, 200);
                assert_eq!(item.first_seen_at, 100);
            }
            _ => panic!("expected DuplicateUpdated"),
        }
        assert_eq!(db.recent_items(10).unwrap().len(), 1);
    }

    #[test]
    fn duplicate_moves_to_top_of_recent_list() {
        let db = db();
        db.upsert_capture("first", 100).unwrap();
        db.upsert_capture("second", 200).unwrap();
        db.upsert_capture("first", 300).unwrap();

        let recent = db.recent_items(10).unwrap();
        assert_eq!(recent[0].text, "first");
        assert_eq!(recent[1].text, "second");
    }

    #[test]
    fn delete_all_clears_rows_and_fts_index() {
        let db = db();
        db.upsert_capture("alpha searchable text", 100).unwrap();
        db.upsert_capture("beta text", 200).unwrap();
        db.delete_all().unwrap();

        assert_eq!(db.recent_items(10).unwrap().len(), 0);

        let conn = db.conn.lock().unwrap();
        let fts_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM clipboard_items_fts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fts_count, 0);
    }

    #[test]
    fn fts_search_finds_newly_inserted_row() {
        let db = db();
        db.upsert_capture("the quick brown fox jumps", 100).unwrap();
        db.upsert_capture("an unrelated line", 200).unwrap();

        let results = db.search_items("brown", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].text.contains("brown"));
    }

    #[test]
    fn fts_stays_in_sync_after_update_and_delete() {
        let db = db();
        db.upsert_capture("original wording here", 100).unwrap();
        // Duplicate capture updates the row (via UPDATE), not a fresh INSERT.
        db.upsert_capture("original wording here", 200).unwrap();
        assert_eq!(db.search_items("wording", 10).unwrap().len(), 1);

        db.delete_all().unwrap();
        assert_eq!(db.search_items("wording", 10).unwrap().len(), 0);
    }

    #[test]
    fn search_falls_back_to_like_on_fts_syntax_error() {
        let db = db();
        db.upsert_capture("weird \"unterminated quote example", 100)
            .unwrap();

        // `"` starts an unterminated FTS5 string literal, which is a syntax
        // error at the FTS layer; the LIKE fallback should still find it.
        let results = db.search_items("\"unterminated", 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn pause_state_round_trips() {
        let db = db();
        assert!(!db.is_paused().unwrap());
        db.set_paused(true).unwrap();
        assert!(db.is_paused().unwrap());
        db.set_paused(false).unwrap();
        assert!(!db.is_paused().unwrap());
    }

    #[test]
    fn storage_stats_reflect_duplicates() {
        let db = db();
        db.upsert_capture("one", 100).unwrap();
        db.upsert_capture("one", 200).unwrap();
        db.upsert_capture("two", 300).unwrap();

        let stats = db.storage_stats().unwrap();
        assert_eq!(stats.total_items, 2);
        assert_eq!(stats.total_duplicates, 1);
        assert_eq!(stats.oldest_item_at, Some(100));
        assert_eq!(stats.newest_item_at, Some(300));
    }

    #[test]
    fn data_survives_close_and_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("recallpet.db");

        {
            let db = Database::open(&db_path).unwrap();
            db.upsert_capture("persisted across restarts", 100).unwrap();
            db.set_paused(true).unwrap();
        } // `db` (and its connection) dropped here, simulating app quit.

        let reopened = Database::open(&db_path).unwrap();
        let items = reopened.recent_items(10).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].text, "persisted across restarts");
        assert!(reopened.is_paused().unwrap());
    }
}
