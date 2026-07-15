//! Versioned schema migrations, applied inside a transaction on startup.
//!
//! AIDEV-NOTE: add new migrations by appending to `MIGRATIONS` with the next
//! sequential version. Never edit a migration that has already shipped.

use rusqlite::Connection;

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "init",
    sql: include_str!("../migrations/0001_init.sql"),
}];

#[derive(Debug)]
pub enum MigrationError {
    Sqlite(rusqlite::Error),
}

impl std::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationError::Sqlite(e) => write!(f, "migration failed: {e}"),
        }
    }
}

impl std::error::Error for MigrationError {}

impl From<rusqlite::Error> for MigrationError {
    fn from(e: rusqlite::Error) -> Self {
        MigrationError::Sqlite(e)
    }
}

/// Applies every migration that has not yet been recorded in
/// `schema_migrations`, each inside its own transaction.
pub fn run(conn: &mut Connection) -> Result<(), MigrationError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at INTEGER NOT NULL
        );",
    )?;

    let applied: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;

    for migration in MIGRATIONS.iter().filter(|m| m.version > applied) {
        let tx = conn.transaction()?;
        tx.execute_batch(migration.sql)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![migration.version, migration.name, current_epoch_millis()],
        )?;
        tx.commit()?;
        log::info!(
            "applied migration {} ({})",
            migration.version,
            migration.name
        );
    }

    Ok(())
}

fn current_epoch_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_migrations_once() {
        let mut conn = Connection::open_in_memory().unwrap();
        run(&mut conn).unwrap();
        run(&mut conn).unwrap(); // idempotent, no double-apply

        let version: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 1);

        // Table + FTS + triggers exist.
        conn.execute(
            "INSERT INTO clipboard_items
                (content_hash, text, preview, character_count, duplicate_count,
                 first_seen_at, last_seen_at, pinned, created_at, updated_at)
             VALUES ('h', 't', 'p', 1, 1, 0, 0, 0, 0, 0)",
            [],
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM clipboard_items_fts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
