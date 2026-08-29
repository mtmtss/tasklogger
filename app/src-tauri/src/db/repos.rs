use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::domain::models::{ActiveSessionRow, WorkLogRow};
use crate::domain::time;

pub struct TaskRow {
    pub id: String,
    pub task_list_id: String,
    pub task_list_title: String,
    pub title: String,
    pub notes: String,
    pub due: Option<String>,
    pub status: String,
    /// 「今日やる」ローカル専用フラグ (Google Tasks へは同期しない、spec拡張)。
    pub today_flag: bool,
}

/// 未完了 (needsAction) のタスクをタスクリスト名込みで全件取得。
pub fn fetch_open_tasks(conn: &Connection) -> rusqlite::Result<Vec<TaskRow>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.task_list_id, l.title, t.title, COALESCE(t.notes, ''), t.due, t.status, t.today_flag
         FROM tasks t
         JOIN task_lists l ON l.id = t.task_list_id
         WHERE t.deleted = 0 AND l.deleted = 0 AND t.status != 'completed'
         ORDER BY l.title, t.position, t.title",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(TaskRow {
                id: row.get(0)?,
                task_list_id: row.get(1)?,
                task_list_title: row.get(2)?,
                title: row.get(3)?,
                notes: row.get(4)?,
                due: row.get(5)?,
                status: row.get(6)?,
                today_flag: row.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn get_task(conn: &Connection, task_list_id: &str, task_id: &str) -> rusqlite::Result<Option<TaskRow>> {
    conn.query_row(
        "SELECT t.id, t.task_list_id, l.title, t.title, COALESCE(t.notes, ''), t.due, t.status, t.today_flag
         FROM tasks t JOIN task_lists l ON l.id = t.task_list_id
         WHERE t.task_list_id = ?1 AND t.id = ?2",
        params![task_list_id, task_id],
        |row| {
            Ok(TaskRow {
                id: row.get(0)?,
                task_list_id: row.get(1)?,
                task_list_title: row.get(2)?,
                title: row.get(3)?,
                notes: row.get(4)?,
                due: row.get(5)?,
                status: row.get(6)?,
                today_flag: row.get(7)?,
            })
        },
    )
    .optional()
}

/// タスクリストの選択肢一覧 (spec拡張: その場でタスク追加する際の追加先選択用)。
pub fn fetch_task_lists(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, title FROM task_lists WHERE deleted = 0 ORDER BY title",
    )?;
    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// ローカルでその場で追加したタスクを挿入する。id は `local-` prefix
/// (Google Tasks には存在しない) とし、Google 側へは同期しない (spec拡張、
/// [[今日やるフラグ]]と同様にローカル専用の拡張として扱う)。
/// 追加直後から「今日やる」リストに出したいので today_flag=1 で作成する。
pub fn insert_local_task(
    conn: &Connection,
    task_list_id: &str,
    title: &str,
) -> rusqlite::Result<String> {
    let task_id = format!("local-{}", Uuid::new_v4());
    let now = time::to_iso(&time::now_utc());
    conn.execute(
        "INSERT INTO tasks (id, task_list_id, title, notes, due, status, fetched_at, today_flag)
         VALUES (?1, ?2, ?3, '', NULL, 'needsAction', ?4, 1)",
        params![task_id, task_list_id, title, now],
    )?;
    Ok(task_id)
}

pub fn get_active_session(conn: &Connection) -> rusqlite::Result<Option<ActiveSessionRow>> {
    conn.query_row(
        "SELECT task_list_id, task_list_name, task_id, task_title, start_at
         FROM active_session WHERE id = 1",
        [],
        |row| {
            Ok(ActiveSessionRow {
                task_list_id: row.get(0)?,
                task_list_name: row.get(1)?,
                task_id: row.get(2)?,
                task_title: row.get(3)?,
                start_at: row.get(4)?,
            })
        },
    )
    .optional()
}

pub fn create_active_session(
    conn: &Connection,
    task_list_id: &str,
    task_list_name: &str,
    task_id: &str,
    task_title: &str,
) -> rusqlite::Result<()> {
    let now = time::to_iso(&time::now_utc());
    conn.execute(
        "INSERT INTO active_session
           (id, task_list_id, task_list_name, task_id, task_title, start_at, last_heartbeat_at)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?5)",
        params![task_list_id, task_list_name, task_id, task_title, now],
    )?;
    Ok(())
}

/// active_session の last_heartbeat_at (セッションが無ければ None)。
pub fn get_session_heartbeat(conn: &Connection) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT last_heartbeat_at FROM active_session WHERE id = 1",
        [],
        |row| row.get(0),
    )
    .optional()
}

pub fn clear_active_session(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM active_session WHERE id = 1", [])?;
    Ok(())
}

pub struct NewWorkLog<'a> {
    pub task_list_id: &'a str,
    pub task_list_name: &'a str,
    pub task_id: &'a str,
    pub task_title: &'a str,
    pub action_type: &'a str,
    pub start_time: String,
    pub end_time: String,
    pub duration_seconds: i64,
    pub memo: &'a str,
    pub end_reason: &'a str,
}

pub fn append_work_log(conn: &Connection, log: &NewWorkLog) -> rusqlite::Result<String> {
    let log_id = Uuid::new_v4().to_string();
    let created_at = time::to_iso(&time::now_utc());
    let user_id: String = get_setting(conn, "user_email")?.unwrap_or_default();
    let log_date = time::parse_iso(&log.start_time)
        .map(|dt| time::jst_date_text(&dt))
        .unwrap_or_else(time::today_jst);
    conn.execute(
        "INSERT INTO work_logs
           (log_id, user_id, task_list_id, task_list_name, task_id, task_title,
            action_type, start_time, end_time, duration_seconds, duration_minutes,
            log_date, memo, created_at, end_reason, source)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 'app')",
        params![
            log_id,
            user_id,
            log.task_list_id,
            log.task_list_name,
            log.task_id,
            log.task_title,
            log.action_type,
            log.start_time,
            log.end_time,
            log.duration_seconds,
            time::ceil_minutes(log.duration_seconds),
            log_date,
            log.memo,
            created_at,
            log.end_reason,
        ],
    )?;
    Ok(log_id)
}

/// アーカイブ集計・エクスポート用の完全なログ行。
pub struct FullLogRow {
    pub log_id: String,
    pub user_id: String,
    pub task_list_id: String,
    pub task_list_name: String,
    pub task_id: String,
    pub task_title: String,
    pub action_type: String,
    pub start_time: String,
    pub end_time: String,
    pub duration_seconds: i64,
    pub duration_minutes: i64,
    pub log_date: String,
    pub memo: String,
    pub created_at: String,
    pub end_reason: String,
    pub source: String,
}

pub fn fetch_logs_by_range(
    conn: &Connection,
    start_date: &str,
    end_date: &str,
) -> rusqlite::Result<Vec<FullLogRow>> {
    let mut stmt = conn.prepare(
        "SELECT log_id, user_id, task_list_id, task_list_name, task_id, task_title,
                action_type, start_time, end_time, duration_seconds, duration_minutes,
                log_date, memo, created_at, end_reason, source
         FROM work_logs
         WHERE log_date >= ?1 AND log_date <= ?2
         ORDER BY log_date, start_time",
    )?;
    let rows = stmt
        .query_map(params![start_date, end_date], |row| {
            Ok(FullLogRow {
                log_id: row.get(0)?,
                user_id: row.get(1)?,
                task_list_id: row.get(2)?,
                task_list_name: row.get(3)?,
                task_id: row.get(4)?,
                task_title: row.get(5)?,
                action_type: row.get(6)?,
                start_time: row.get(7)?,
                end_time: row.get(8)?,
                duration_seconds: row.get(9)?,
                duration_minutes: row.get(10)?,
                log_date: row.get(11)?,
                memo: row.get(12)?,
                created_at: row.get(13)?,
                end_reason: row.get(14)?,
                source: row.get(15)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// インポート用: log_id が既存なら何もしない (重複防止, spec §5.6)。true = 挿入した。
pub fn insert_imported_log(conn: &Connection, log: &FullLogRow) -> rusqlite::Result<bool> {
    let changed = conn.execute(
        "INSERT OR IGNORE INTO work_logs
           (log_id, user_id, task_list_id, task_list_name, task_id, task_title,
            action_type, start_time, end_time, duration_seconds, duration_minutes,
            log_date, memo, created_at, end_reason, source)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            log.log_id,
            log.user_id,
            log.task_list_id,
            log.task_list_name,
            log.task_id,
            log.task_title,
            log.action_type,
            log.start_time,
            log.end_time,
            log.duration_seconds,
            log.duration_minutes,
            log.log_date,
            log.memo,
            log.created_at,
            log.end_reason,
            log.source,
        ],
    )?;
    Ok(changed == 1)
}

pub fn fetch_logs_by_date(conn: &Connection, date_text: &str) -> rusqlite::Result<Vec<WorkLogRow>> {
    let mut stmt = conn.prepare(
        "SELECT task_list_id, task_list_name, task_id, action_type, end_time,
                duration_seconds, end_reason
         FROM work_logs WHERE log_date = ?1",
    )?;
    let rows = stmt
        .query_map(params![date_text], |row| {
            Ok(WorkLogRow {
                task_list_id: row.get(0)?,
                task_list_name: row.get(1)?,
                task_id: row.get(2)?,
                action_type: row.get(3)?,
                end_time: row.get(4)?,
                duration_seconds: row.get(5)?,
                end_reason: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn mark_task_completed_locally(
    conn: &Connection,
    task_list_id: &str,
    task_id: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE tasks SET status = 'completed', dirty = 1, today_flag = 0
         WHERE task_list_id = ?1 AND id = ?2",
        params![task_list_id, task_id],
    )?;
    Ok(())
}

/// タスク削除 (spec拡張)。ソフトデリート + dirty=1 で、Google Tasks 側へは
/// push_queue 経由の "delete_task" op で同期される (ローカル専用タスクは
/// push_queue 側で id prefix "local-" によりスキップされ、DB上のソフトデリートのみで完結する)。
pub fn mark_task_deleted_locally(
    conn: &Connection,
    task_list_id: &str,
    task_id: &str,
) -> rusqlite::Result<()> {
    // today_flag は温存する (削除は deleted=1 だけで一覧から隠れる)。undo 時に
    // 元の「今日やる / 候補」区分をそのまま復元できるようにするため。
    conn.execute(
        "UPDATE tasks SET deleted = 1, dirty = 1
         WHERE task_list_id = ?1 AND id = ?2",
        params![task_list_id, task_id],
    )?;
    Ok(())
}

/// 未 push の delete_task op が sync_queue に残っているか (undo 可能かの判定用)。
/// 残っていれば「まだ Google に反映されていない」とみなせる。
pub fn has_pending_delete_op(
    conn: &Connection,
    task_list_id: &str,
    task_id: &str,
) -> rusqlite::Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sync_queue
         WHERE op_type = 'delete_task' AND task_list_id = ?1 AND task_id = ?2",
        params![task_list_id, task_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// delete_task op を sync_queue から取り除く (undo 用)。
pub fn remove_pending_delete_op(
    conn: &Connection,
    task_list_id: &str,
    task_id: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM sync_queue
         WHERE op_type = 'delete_task' AND task_list_id = ?1 AND task_id = ?2",
        params![task_list_id, task_id],
    )?;
    Ok(())
}

/// ソフトデリートを取り消す (undo)。まだ Google に push されていないケース専用
/// なので dirty=0 (クリーンな同期状態) に戻す。today_flag は削除時に温存して
/// あるのでここでは触らない。
pub fn undo_task_delete_locally(
    conn: &Connection,
    task_list_id: &str,
    task_id: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE tasks SET deleted = 0, dirty = 0
         WHERE task_list_id = ?1 AND id = ?2",
        params![task_list_id, task_id],
    )?;
    Ok(())
}

/// due の更新 (Google Tasks へ同期される)。`None` で期限をクリアする。
pub fn set_task_due_locally(
    conn: &Connection,
    task_list_id: &str,
    task_id: &str,
    due: Option<&str>,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE tasks SET due = ?3, dirty = 1
         WHERE task_list_id = ?1 AND id = ?2",
        params![task_list_id, task_id, due],
    )?;
    Ok(())
}

/// 「今日やる」ローカル専用フラグの更新。Google Tasks へは同期しない (dirty/sync_queue 対象外)。
pub fn set_today_flag(
    conn: &Connection,
    task_list_id: &str,
    task_id: &str,
    flag: bool,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE tasks SET today_flag = ?3
         WHERE task_list_id = ?1 AND id = ?2",
        params![task_list_id, task_id, flag],
    )?;
    Ok(())
}

pub fn enqueue_sync_op(
    conn: &Connection,
    op_type: &str,
    task_list_id: &str,
    task_id: &str,
    payload: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO sync_queue (op_type, task_list_id, task_id, payload, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            op_type,
            task_list_id,
            task_id,
            payload,
            time::to_iso(&time::now_utc())
        ],
    )?;
    Ok(())
}

pub fn sync_queue_count(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("SELECT COUNT(*) FROM sync_queue", [], |row| row.get(0))
}

pub fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn delete_setting(conn: &Connection, key: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::AppStatus;
    use crate::domain::status;

    /// 開始→中断→再開→完了のフローが DB 上で正しく回ることを確認する統合テスト。
    #[test]
    fn full_session_flow() {
        let conn = crate::db::open_in_memory().unwrap();
        seed_sample_data(&conn).unwrap();

        // 開始
        create_active_session(&conn, "sample-list-1", "研究", "sample-task-1", "論文").unwrap();
        let session = get_active_session(&conn).unwrap().unwrap();
        assert_eq!(session.task_id, "sample-task-1");

        // 中断 (paused ログ)
        append_work_log(
            &conn,
            &NewWorkLog {
                task_list_id: "sample-list-1",
                task_list_name: "研究",
                task_id: "sample-task-1",
                task_title: "論文",
                action_type: "paused",
                start_time: session.start_at.clone(),
                end_time: time::to_iso(&time::now_utc()),
                duration_seconds: 90,
                memo: "",
                end_reason: "user",
            },
        )
        .unwrap();
        clear_active_session(&conn).unwrap();

        let logs = fetch_logs_by_date(&conn, &time::today_jst()).unwrap();
        let stats = status::build_today_stats(&logs, None, 0);
        let (secs, st, _) = status::lookup(&stats, "sample-list-1", "sample-task-1");
        assert_eq!(secs, 90);
        assert_eq!(st, AppStatus::Paused);

        // 直接完了 (duration=0)
        let now = time::to_iso(&time::now_utc());
        append_work_log(
            &conn,
            &NewWorkLog {
                task_list_id: "sample-list-1",
                task_list_name: "研究",
                task_id: "sample-task-1",
                task_title: "論文",
                action_type: "completed",
                start_time: now.clone(),
                end_time: now,
                duration_seconds: 0,
                memo: "",
                end_reason: "direct_complete",
            },
        )
        .unwrap();
        mark_task_completed_locally(&conn, "sample-list-1", "sample-task-1").unwrap();
        enqueue_sync_op(&conn, "complete_task", "sample-list-1", "sample-task-1", "{}").unwrap();

        // 状態は completed、合計時間は据え置き、キューに 1 件
        let logs = fetch_logs_by_date(&conn, &time::today_jst()).unwrap();
        let stats = status::build_today_stats(&logs, None, 0);
        let (secs, st, sessions) = status::lookup(&stats, "sample-list-1", "sample-task-1");
        assert_eq!(secs, 90);
        assert_eq!(st, AppStatus::Completed);
        assert_eq!(sessions, 1);
        assert_eq!(sync_queue_count(&conn).unwrap(), 1);

        // 完了タスクは未完了一覧から消える
        let open = fetch_open_tasks(&conn).unwrap();
        assert!(open.iter().all(|t| t.id != "sample-task-1"));

        // 期限が今日のタスクの due 判定
        let due_today = open
            .iter()
            .find(|t| t.id == "sample-task-2")
            .map(|t| time::is_due_today(&t.due));
        assert_eq!(due_today, Some(true));
    }

    /// today_flag: 既定値0、set_today_flagで立つ、完了で自動的に0へ戻る。
    #[test]
    fn today_flag_lifecycle() {
        let conn = crate::db::open_in_memory().unwrap();
        seed_sample_data(&conn).unwrap();

        let task = get_task(&conn, "sample-list-1", "sample-task-1")
            .unwrap()
            .unwrap();
        assert!(!task.today_flag);

        set_today_flag(&conn, "sample-list-1", "sample-task-1", true).unwrap();
        let task = get_task(&conn, "sample-list-1", "sample-task-1")
            .unwrap()
            .unwrap();
        assert!(task.today_flag);

        mark_task_completed_locally(&conn, "sample-list-1", "sample-task-1").unwrap();
        let task = get_task(&conn, "sample-list-1", "sample-task-1")
            .unwrap()
            .unwrap();
        assert!(!task.today_flag);
    }

    /// set_task_due_locally: Some/Noneどちらでも正しく更新できる。
    #[test]
    fn set_task_due_locally_updates_or_clears() {
        let conn = crate::db::open_in_memory().unwrap();
        seed_sample_data(&conn).unwrap();

        set_task_due_locally(&conn, "sample-list-1", "sample-task-1", Some("2030-01-01T00:00:00.000Z"))
            .unwrap();
        let task = get_task(&conn, "sample-list-1", "sample-task-1")
            .unwrap()
            .unwrap();
        assert_eq!(task.due.as_deref(), Some("2030-01-01T00:00:00.000Z"));

        set_task_due_locally(&conn, "sample-list-1", "sample-task-1", None).unwrap();
        let task = get_task(&conn, "sample-list-1", "sample-task-1")
            .unwrap()
            .unwrap();
        assert_eq!(task.due, None);
    }

    /// insert_local_task: local- prefix の id で作成され、today_flag が立った状態で
    /// 未完了一覧に現れる。
    #[test]
    fn insert_local_task_creates_today_task() {
        let conn = crate::db::open_in_memory().unwrap();
        seed_sample_data(&conn).unwrap();

        let task_id = insert_local_task(&conn, "sample-list-1", "買い物リストを作る").unwrap();
        assert!(task_id.starts_with("local-"));

        let task = get_task(&conn, "sample-list-1", &task_id).unwrap().unwrap();
        assert_eq!(task.title, "買い物リストを作る");
        assert_eq!(task.status, "needsAction");
        assert!(task.today_flag);

        let open = fetch_open_tasks(&conn).unwrap();
        assert!(open.iter().any(|t| t.id == task_id));
    }

    /// mark_task_deleted_locally: 未完了一覧から消え、dirty=1 になって sync_queue に積める。
    #[test]
    fn mark_task_deleted_locally_removes_from_open_tasks() {
        let conn = crate::db::open_in_memory().unwrap();
        seed_sample_data(&conn).unwrap();

        mark_task_deleted_locally(&conn, "sample-list-1", "sample-task-1").unwrap();
        enqueue_sync_op(&conn, "delete_task", "sample-list-1", "sample-task-1", "{}").unwrap();

        let open = fetch_open_tasks(&conn).unwrap();
        assert!(open.iter().all(|t| t.id != "sample-task-1"));
        assert_eq!(sync_queue_count(&conn).unwrap(), 1);
    }

    /// undo: 未 push の delete_task op を取り消すと、キューが空になり
    /// タスクが未完了一覧へ戻る。
    #[test]
    fn undo_task_delete_restores_open_task_and_clears_queue() {
        let conn = crate::db::open_in_memory().unwrap();
        seed_sample_data(&conn).unwrap();

        mark_task_deleted_locally(&conn, "sample-list-1", "sample-task-1").unwrap();
        enqueue_sync_op(&conn, "delete_task", "sample-list-1", "sample-task-1", "{}").unwrap();
        assert!(has_pending_delete_op(&conn, "sample-list-1", "sample-task-1").unwrap());

        remove_pending_delete_op(&conn, "sample-list-1", "sample-task-1").unwrap();
        undo_task_delete_locally(&conn, "sample-list-1", "sample-task-1").unwrap();

        assert!(!has_pending_delete_op(&conn, "sample-list-1", "sample-task-1").unwrap());
        assert_eq!(sync_queue_count(&conn).unwrap(), 0);
        let open = fetch_open_tasks(&conn).unwrap();
        assert!(open.iter().any(|t| t.id == "sample-task-1"));
    }
}

/// 開発用: オフラインでも UI を確認できるサンプルデータを投入する。
pub fn seed_sample_data(conn: &Connection) -> rusqlite::Result<()> {
    let now = time::to_iso(&time::now_utc());
    let today_due = time::today_due_value();
    conn.execute(
        "INSERT OR IGNORE INTO task_lists (id, title, fetched_at) VALUES
           ('sample-list-1', '研究', ?1),
           ('sample-list-2', '雑務', ?1)",
        params![now],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO tasks (id, task_list_id, title, notes, due, status, fetched_at) VALUES
           ('sample-task-1', 'sample-list-1', '論文の実験セクションを書く', '図3を差し替える', ?2, 'needsAction', ?1),
           ('sample-task-2', 'sample-list-1', '先行研究サーベイ', '', ?2, 'needsAction', ?1),
           ('sample-task-3', 'sample-list-2', '経費精算', '', NULL, 'needsAction', ?1),
           ('sample-task-4', 'sample-list-2', 'メール返信', '', '2099-01-01T00:00:00.000Z', 'needsAction', ?1)",
        params![now, today_due],
    )?;
    Ok(())
}
