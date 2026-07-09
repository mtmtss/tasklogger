//! クイックキャプチャ + Inbox (AI 拡張仕様 §13)。
//! 保存 (ローカル即時・オフライン可) と整理 (AI 分類 → 承認 → Google 登録) を分離する。
//! Google へのタスク新規作成はここが唯一の経路 (sync_queue を通さず直接 tasks.insert)。

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{Emitter, Manager, State};

use super::gemini_api;
use crate::commands::{db_err, emit_tasks_changed, CmdResult};
use crate::db::repos;
use crate::domain::time;
use crate::google::{self, tasks_api};
use crate::state::AppState;

/// 非タスク項目の登録先。存在しなければ初回登録時に自動作成する (仕様 §13.5)。
pub const RESEARCH_IDEA_LIST: &str = "研究アイデア";
pub const SOMEDAY_LIST: &str = "Someday";

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureView {
    pub id: String,
    pub text: String,
    pub status: String,
    pub ai_result: Option<Value>,
    pub created_at: String,
}

fn to_view(row: repos::CaptureRow) -> CaptureView {
    CaptureView {
        ai_result: row
            .ai_result
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        id: row.id,
        text: row.text,
        status: row.status,
        created_at: row.created_at,
    }
}

pub fn emit_captures_changed(app: &tauri::AppHandle) {
    let _ = app.emit("captures-changed", ());
}

/// キャプチャ保存 (ローカル完結・即 return, 仕様 §13.3)。分類はバックグラウンドで行う。
#[tauri::command]
pub fn add_capture(app: tauri::AppHandle, text: String) -> CmdResult<String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("メモが空です。".into());
    }
    let id = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        repos::insert_capture(&conn, &text).map_err(db_err)?
    };
    emit_captures_changed(&app);

    // 自動分類 (API キー未設定・失敗時は pending のまま残す, 仕様 §13.4)
    let handle = app.clone();
    let capture_id = id.clone();
    std::thread::spawn(move || {
        let _ = classify(&handle, &capture_id, false);
        emit_captures_changed(&handle);
    });
    Ok(id)
}

#[tauri::command]
pub fn get_captures(state: State<'_, AppState>) -> CmdResult<Vec<CaptureView>> {
    let conn = state.db.lock().unwrap();
    let rows = repos::fetch_inbox_captures(&conn).map_err(db_err)?;
    Ok(rows.into_iter().map(to_view).collect())
}

#[tauri::command]
pub fn get_inbox_count(state: State<'_, AppState>) -> CmdResult<i64> {
    let conn = state.db.lock().unwrap();
    repos::inbox_count(&conn).map_err(db_err)
}

/// 分類の手動再試行 (Inbox の「再試行」ボタン)。
#[tauri::command]
pub async fn classify_capture(app: tauri::AppHandle, capture_id: String) -> CmdResult<()> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || classify(&handle, &capture_id, true))
        .await
        .map_err(|e| format!("分類処理に失敗しました: {e}"))??;
    emit_captures_changed(&app);
    Ok(())
}

/// 分類本体 (blocking)。strict=false では API キー未設定を成功扱いにする (pending 残留)。
fn classify(app: &tauri::AppHandle, capture_id: &str, strict: bool) -> Result<(), String> {
    let api_key = match gemini_api::load_api_key() {
        Some(key) => key,
        None if strict => {
            return Err("API キーが未設定です。設定ページで入力してください。".into())
        }
        None => return Ok(()),
    };

    let (model, user_context, payload) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        let capture = repos::get_capture(&conn, capture_id)
            .map_err(db_err)?
            .ok_or("キャプチャが見つかりません。")?;
        if capture.status != "pending" && capture.status != "classified" {
            return Ok(()); // 登録/破棄済みは対象外
        }
        let lists = repos::fetch_task_lists(&conn).map_err(db_err)?;
        let model = super::current_model(&conn);
        let user_context = repos::get_setting(&conn, "ai_user_context")
            .map_err(db_err)?
            .unwrap_or_default();
        (model, user_context, build_classify_payload(&capture.text, &lists))
    };

    let result = gemini_api::generate_classification(&api_key, &model, &user_context, &payload)?;

    let state = app.state::<AppState>();
    let conn = state.db.lock().unwrap();
    repos::set_capture_classification(&conn, capture_id, &result.to_string()).map_err(db_err)
}

fn build_classify_payload(text: &str, lists: &[repos::TaskListRow]) -> String {
    let names: Vec<&str> = lists
        .iter()
        .filter(|l| !l.id.starts_with("sample-"))
        .map(|l| l.title.as_str())
        .collect();
    format!(
        "今日の日付: {}\n既存のタスクリスト: {}\n\n# メモ\n{}",
        time::today_jst(),
        if names.is_empty() {
            "(なし)".to_string()
        } else {
            names.join(", ")
        },
        text
    )
}

/// キャプチャを破棄する (行は残す, 仕様 §13.7)。
#[tauri::command]
pub fn dismiss_capture(app: tauri::AppHandle, capture_id: String) -> CmdResult<()> {
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        let ids = registered_ids_of(&conn, &capture_id)?;
        repos::finish_capture(&conn, &capture_id, "dismissed", &ids).map_err(db_err)?;
    }
    emit_captures_changed(&app);
    Ok(())
}

fn registered_ids_of(conn: &rusqlite::Connection, capture_id: &str) -> Result<Vec<String>, String> {
    let capture = repos::get_capture(conn, capture_id)
        .map_err(db_err)?
        .ok_or("キャプチャが見つかりません。")?;
    let ai: Value = capture
        .ai_result
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(Value::Null);
    Ok(collect_registered_ids(&ai))
}

/// Inbox の項目 (AI 提案をユーザーが編集したもの) を Google Tasks に登録する。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterItem {
    pub title: String,
    /// 既存リストを選択した場合の ID (優先)
    pub list_id: Option<String>,
    /// リスト名 (「研究アイデア」「Someday」は無ければ自動作成)
    pub list_name: Option<String>,
    /// YYYY-MM-DD
    pub due: Option<String>,
    pub first_step: Option<String>,
    pub note: Option<String>,
    /// ai_result.items 内の位置 (登録済みマーク用)
    pub item_index: Option<usize>,
}

#[tauri::command]
pub async fn register_capture_item(
    app: tauri::AppHandle,
    capture_id: String,
    item: RegisterItem,
) -> CmdResult<CaptureView> {
    let title = item.title.trim().to_string();
    if title.is_empty() {
        return Err("タイトルを入力してください。".into());
    }

    let (capture_text, capture_created_at) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        let capture = repos::get_capture(&conn, &capture_id)
            .map_err(db_err)?
            .ok_or("キャプチャが見つかりません。")?;
        (capture.text, capture.created_at)
    };
    let notes = build_notes(
        item.first_step.as_deref(),
        item.note.as_deref(),
        &capture_text,
        &capture_created_at,
    );

    // Google へ作成 (ネットワーク: token refresh / tasklists.insert / tasks.insert)
    let handle = app.clone();
    let due = item.due.clone();
    let list_id = item.list_id.clone();
    let list_name = item.list_name.clone();
    let task_title = title.clone();
    let google_task_id = tauri::async_runtime::spawn_blocking(move || {
        create_google_task(&handle, list_id, list_name, &task_title, &notes, due.as_deref())
    })
    .await
    .map_err(|e| format!("登録処理に失敗しました: {e}"))??;

    // キャプチャ側に登録済みマークを付け、全項目登録済みなら registered にする
    let view = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        let capture = repos::get_capture(&conn, &capture_id)
            .map_err(db_err)?
            .ok_or("キャプチャが見つかりません。")?;
        let mut ai: Value = capture
            .ai_result
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_else(|| json!({ "items": [] }));
        let all_done = mark_item_registered(&mut ai, item.item_index, &google_task_id);
        repos::update_capture_ai_result(&conn, &capture_id, &ai.to_string()).map_err(db_err)?;
        if all_done {
            let ids = collect_registered_ids(&ai);
            repos::finish_capture(&conn, &capture_id, "registered", &ids).map_err(db_err)?;
        }
        repos::get_capture(&conn, &capture_id)
            .map_err(db_err)?
            .map(to_view)
            .ok_or("キャプチャが見つかりません。")?
    };

    emit_tasks_changed(&app);
    emit_captures_changed(&app);
    Ok(view)
}

/// AI を通さない手動クイック追加 (仕様 §13.6)。
#[tauri::command]
pub async fn quick_add_task(
    app: tauri::AppHandle,
    list_id: String,
    title: String,
    due: Option<String>,
) -> CmdResult<()> {
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err("タイトルを入力してください。".into());
    }
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        create_google_task(&handle, Some(list_id), None, &title, "", due.as_deref())
    })
    .await
    .map_err(|e| format!("登録処理に失敗しました: {e}"))??;
    emit_tasks_changed(&app);
    Ok(())
}

/// Google Tasks へタスクを作成し、ローカルキャッシュへ反映する (blocking, 仕様 §13.9)。
/// 戻り値は Google 発番のタスク ID。
fn create_google_task(
    app: &tauri::AppHandle,
    list_id: Option<String>,
    list_name: Option<String>,
    title: &str,
    notes: &str,
    due: Option<&str>,
) -> Result<String, String> {
    let token = google::get_access_token(app)?
        .ok_or("Google と未接続です。設定ページから接続してください。")?;

    // 登録先リストの解決 (仕様 §13.5)
    let list_id = match list_id.filter(|id| !id.is_empty()) {
        Some(id) => id,
        None => {
            let name = list_name
                .filter(|n| !n.trim().is_empty())
                .ok_or("登録先リストを選択してください。")?;
            let existing = {
                let state = app.state::<AppState>();
                let conn = state.db.lock().unwrap();
                repos::find_task_list_by_title(&conn, &name).map_err(db_err)?
            };
            match existing {
                Some(id) => id,
                None => {
                    // 自動作成は専用リストのみ。それ以外は選び直してもらう
                    if name != RESEARCH_IDEA_LIST && name != SOMEDAY_LIST {
                        return Err(format!(
                            "リスト「{name}」が存在しません。登録先を選択してください。"
                        ));
                    }
                    let list =
                        tasks_api::insert_tasklist(&token, &name).map_err(|e| e.to_string())?;
                    let state = app.state::<AppState>();
                    let conn = state.db.lock().unwrap();
                    repos::insert_task_list_row(&conn, &list.id, &list.title, list.updated.as_deref())
                        .map_err(db_err)?;
                    list.id
                }
            }
        }
    };

    if list_id.starts_with("sample-") {
        return Err("ローカルのサンプルリストには登録できません。".into());
    }

    let mut body = json!({ "title": title, "status": "needsAction" });
    if !notes.is_empty() {
        body["notes"] = json!(notes);
    }
    if let Some(value) = due.and_then(due_value) {
        body["due"] = json!(value);
    }

    let task = tasks_api::insert_task(&token, &list_id, &body).map_err(|e| e.to_string())?;

    let state = app.state::<AppState>();
    let conn = state.db.lock().unwrap();
    repos::insert_task_row(
        &conn,
        &repos::NewTaskRow {
            id: &task.id,
            task_list_id: &list_id,
            title: task.title.as_deref().unwrap_or(title),
            notes: task.notes.as_deref(),
            due: task.due.as_deref(),
            status: &task.status,
            position: task.position.as_deref(),
            updated: task.updated.as_deref(),
        },
    )
    .map_err(db_err)?;
    Ok(task.id)
}

/// タスクの notes: 最初の一手 + AI 補足 + 元メモ全文 (仕様 §13.5)。
fn build_notes(
    first_step: Option<&str>,
    note: Option<&str>,
    capture_text: &str,
    captured_at: &str,
) -> String {
    let mut notes = String::new();
    if let Some(step) = first_step.filter(|s| !s.trim().is_empty()) {
        notes.push_str(&format!("最初の一手: {step}\n"));
    }
    if let Some(n) = note.filter(|s| !s.trim().is_empty()) {
        notes.push_str(&format!("{n}\n"));
    }
    let date = if captured_at.len() >= 10 { &captured_at[0..10] } else { captured_at };
    notes.push_str(&format!("---\n元メモ ({date}):\n{capture_text}"));
    notes
}

/// YYYY-MM-DD を Google Tasks の due (RFC3339) にする。
fn due_value(due: &str) -> Option<String> {
    let due = due.trim();
    if due.len() >= 10 {
        Some(format!("{}T00:00:00.000Z", &due[0..10]))
    } else {
        None
    }
}

/// ai_result の items[index] に registeredTaskId を記録する。
/// 戻り値 = 全項目が登録済みになったか (item_index が無い場合は false)。
fn mark_item_registered(ai: &mut Value, item_index: Option<usize>, task_id: &str) -> bool {
    let Some(index) = item_index else {
        return false;
    };
    let Some(items) = ai["items"].as_array_mut() else {
        return false;
    };
    if let Some(item) = items.get_mut(index) {
        item["registeredTaskId"] = json!(task_id);
    }
    !items.is_empty()
        && items
            .iter()
            .all(|item| item["registeredTaskId"].as_str().is_some())
}

fn collect_registered_ids(ai: &Value) -> Vec<String> {
    ai["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item["registeredTaskId"].as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_include_first_step_and_original_text() {
        let notes = build_notes(
            Some("Scholar で検索する"),
            Some("視覚秘密分散との組み合わせ"),
            "coded rolling shutter と組み合わせたら面白いかも。関連研究3本探したい",
            "2026-07-09T01:23:45.000Z",
        );
        assert!(notes.starts_with("最初の一手: Scholar で検索する\n"));
        assert!(notes.contains("視覚秘密分散との組み合わせ"));
        assert!(notes.contains("元メモ (2026-07-09):"));
        assert!(notes.ends_with("関連研究3本探したい"));
    }

    #[test]
    fn notes_without_optional_parts() {
        let notes = build_notes(None, Some("  "), "経費精算やる", "2026-07-09T01:00:00.000Z");
        assert_eq!(notes, "---\n元メモ (2026-07-09):\n経費精算やる");
    }

    #[test]
    fn due_value_formats_date_only() {
        assert_eq!(
            due_value("2026-07-10").as_deref(),
            Some("2026-07-10T00:00:00.000Z")
        );
        assert_eq!(due_value(""), None);
    }

    #[test]
    fn mark_registered_detects_completion() {
        let mut ai = json!({ "items": [
            { "kind": "TASK", "title": "a" },
            { "kind": "RESEARCH_IDEA", "title": "b" }
        ]});
        assert!(!mark_item_registered(&mut ai, Some(0), "task-1"));
        assert_eq!(ai["items"][0]["registeredTaskId"], "task-1");
        assert!(mark_item_registered(&mut ai, Some(1), "task-2"));
        assert_eq!(collect_registered_ids(&ai), vec!["task-1", "task-2"]);
    }

    #[test]
    fn mark_registered_without_index_never_finishes() {
        let mut ai = json!({ "items": [] });
        assert!(!mark_item_registered(&mut ai, None, "task-1"));
        assert!(!mark_item_registered(&mut ai, Some(0), "task-1"));
    }

    /// captures テーブルのライフサイクル: pending → classified → registered / dismissed。
    #[test]
    fn capture_lifecycle_in_db() {
        let conn = crate::db::open_in_memory().unwrap();
        let id = repos::insert_capture(&conn, "論文の図を差し替える").unwrap();
        assert_eq!(repos::inbox_count(&conn).unwrap(), 1);

        let ai = json!({ "items": [{ "kind": "TASK", "title": "図を差し替える" }] });
        repos::set_capture_classification(&conn, &id, &ai.to_string()).unwrap();
        let capture = repos::get_capture(&conn, &id).unwrap().unwrap();
        assert_eq!(capture.status, "classified");

        repos::finish_capture(&conn, &id, "registered", &["task-1".into()]).unwrap();
        assert_eq!(repos::inbox_count(&conn).unwrap(), 0);
        // 行は残る (仕様 §13.7)
        let capture = repos::get_capture(&conn, &id).unwrap().unwrap();
        assert_eq!(capture.status, "registered");

        // 終了後は分類結果を上書きしない
        repos::set_capture_classification(&conn, &id, "{}").unwrap();
        let capture = repos::get_capture(&conn, &id).unwrap().unwrap();
        assert_eq!(capture.ai_result.unwrap(), ai.to_string());
    }
}
