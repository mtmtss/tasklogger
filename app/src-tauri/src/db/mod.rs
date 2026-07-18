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
];

/// 環境によって user_version=2 の中身が食い違うため、通し番号ではなく
/// 実スキーマの存在確認で冪等に適用する。
fn migrate_today_flag_column(conn: &Connection) -> rusqlite::Result<()> {
    let exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('tasks') WHERE name = 'today_flag'",
        [],
        |row| row.get(0),
    )?;
    if !exists {
        conn.execute_batch("ALTER TABLE tasks ADD COLUMN today_flag INTEGER NOT NULL DEFAULT 0;")?;
    }
    Ok(())
}

fn migrate_daily_plans_table(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS daily_plans (
          id           INTEGER PRIMARY KEY AUTOINCREMENT,
          plan_date    TEXT NOT NULL,
          generated_at TEXT NOT NULL,
          input_note   TEXT NOT NULL DEFAULT '',
          model        TEXT NOT NULL,
          plan_json    TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_daily_plans_date ON daily_plans(plan_date);
        "#,
    )
}

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
    let mut version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate() {
        let target = (i + 1) as i64;
        if version < target {
            conn.execute_batch(sql)?;
            version = target;
            conn.pragma_update(None, "user_version", target)?;
        }
    }
    migrate_today_flag_column(conn)?;
    migrate_daily_plans_table(conn)?;
    if version < 2 {
        conn.pragma_update(None, "user_version", 2)?;
    }
    Ok(())
}
