pub mod repos;

use rusqlite::Connection;
use std::path::Path;

/// PRAGMA user_version ベースの手書きマイグレーション。
/// 末尾に SQL を足して MIGRATIONS を増やすことでスキーマを進化させる。
const MIGRATIONS: &[&str] = &[
    // v1: 初期スキーマ (docs/specification.md §3)
    r#"
    CREATE TABLE task_lists (
      id            TEXT PRIMARY KEY,
      title         TEXT NOT NULL,
      updated       TEXT,
      deleted       INTEGER NOT NULL DEFAULT 0,
      fetched_at    TEXT NOT NULL
    );

    CREATE TABLE tasks (
      id            TEXT PRIMARY KEY,
      task_list_id  TEXT NOT NULL REFERENCES task_lists(id),
      title         TEXT NOT NULL,
      notes         TEXT,
      due           TEXT,
      status        TEXT NOT NULL,
      position      TEXT,
      updated       TEXT,
      deleted       INTEGER NOT NULL DEFAULT 0,
      dirty         INTEGER NOT NULL DEFAULT 0,
      fetched_at    TEXT NOT NULL
    );
    CREATE INDEX idx_tasks_due ON tasks(due, status);

    CREATE TABLE work_logs (
      log_id           TEXT PRIMARY KEY,
      user_id          TEXT NOT NULL DEFAULT '',
      task_list_id     TEXT NOT NULL,
      task_list_name   TEXT NOT NULL,
      task_id          TEXT NOT NULL,
      task_title       TEXT NOT NULL,
      action_type      TEXT NOT NULL CHECK (action_type IN ('paused','completed')),
      start_time       TEXT NOT NULL,
      end_time         TEXT NOT NULL,
      duration_seconds INTEGER NOT NULL,
      duration_minutes INTEGER NOT NULL,
      log_date         TEXT NOT NULL,
      memo             TEXT NOT NULL DEFAULT '',
      created_at       TEXT NOT NULL,
      end_reason       TEXT NOT NULL DEFAULT 'user',
      source           TEXT NOT NULL DEFAULT 'app'
    );
    CREATE INDEX idx_work_logs_log_date ON work_logs(log_date);
    CREATE INDEX idx_work_logs_task ON work_logs(task_list_id, task_id);

    CREATE TABLE active_session (
      id                INTEGER PRIMARY KEY CHECK (id = 1),
      task_list_id      TEXT NOT NULL,
      task_list_name    TEXT NOT NULL,
      task_id           TEXT NOT NULL,
      task_title        TEXT NOT NULL,
      start_at          TEXT NOT NULL,
      last_heartbeat_at TEXT NOT NULL
    );

    CREATE TABLE sync_queue (
      id           INTEGER PRIMARY KEY AUTOINCREMENT,
      op_type      TEXT NOT NULL,
      task_list_id TEXT NOT NULL,
      task_id      TEXT NOT NULL,
      payload      TEXT NOT NULL,
      created_at   TEXT NOT NULL,
      attempts     INTEGER NOT NULL DEFAULT 0,
      last_error   TEXT
    );

    CREATE TABLE settings (
      key   TEXT PRIMARY KEY,
      value TEXT NOT NULL
    );
    "#,
    // v2: AI 拡張「今日の作戦」の履歴 (docs/ai-extension-specification.md §5.1)
    r#"
    CREATE TABLE daily_plans (
      id           INTEGER PRIMARY KEY AUTOINCREMENT,
      plan_date    TEXT NOT NULL,
      generated_at TEXT NOT NULL,
      input_note   TEXT NOT NULL DEFAULT '',
      model        TEXT NOT NULL,
      plan_json    TEXT NOT NULL
    );
    CREATE INDEX idx_daily_plans_date ON daily_plans(plan_date);
    "#,
];

pub fn open(db_path: &Path) -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn open_in_memory() -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<(), rusqlite::Error> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate() {
        let target = (i + 1) as i64;
        if version < target {
            conn.execute_batch(sql)?;
            conn.pragma_update(None, "user_version", target)?;
        }
    }
    Ok(())
}
