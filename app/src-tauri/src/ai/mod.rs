//! AI 拡張「今日の作戦」(docs/ai-extension-specification.md)。
//! ローカルファースト原則の外側にある付加機能: 失敗しても他機能に影響しない。
//! LLM は Gemini API (Google AI Studio キー、無料枠で運用可)。

pub mod context_builder;
pub mod gemini_api;
pub mod plan_store;

use serde::Serialize;
use tauri::{Manager, State};

use crate::commands::{db_err, CmdResult};
use crate::db::repos;
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiStatus {
    pub configured: bool,
    pub model: String,
}

fn current_model(conn: &rusqlite::Connection) -> String {
    repos::get_setting(conn, "ai_model")
        .ok()
        .flatten()
        .filter(|m| gemini_api::ALLOWED_MODELS.contains(&m.as_str()))
        .unwrap_or_else(|| gemini_api::DEFAULT_MODEL.to_string())
}

#[tauri::command]
pub fn get_ai_status(state: State<'_, AppState>) -> CmdResult<AiStatus> {
    let conn = state.db.lock().unwrap();
    Ok(AiStatus {
        configured: gemini_api::load_api_key().is_some(),
        model: current_model(&conn),
    })
}

/// API キーを保存し、接続テストまで行う (キー設定 = AI 機能へのオプトイン, 仕様 §6)。
#[tauri::command]
pub async fn set_ai_api_key(app: tauri::AppHandle, key: String) -> CmdResult<()> {
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err("API キーを入力してください。".into());
    }
    let model = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        current_model(&conn)
    };
    let test_key = key.clone();
    tauri::async_runtime::spawn_blocking(move || gemini_api::test_connection(&test_key, &model))
        .await
        .map_err(|e| e.to_string())??;
    gemini_api::save_api_key(&key)
}

#[tauri::command]
pub fn clear_ai_api_key() -> CmdResult<()> {
    gemini_api::delete_api_key();
    Ok(())
}

/// 今日の作戦を生成して保存する (async: UI をブロックしない, 仕様 §2.1)。
#[tauri::command]
pub async fn generate_daily_plan(
    app: tauri::AppHandle,
    note: String,
) -> CmdResult<plan_store::StoredPlan> {
    let api_key = gemini_api::load_api_key()
        .ok_or("API キーが未設定です。設定ページで入力してください。")?;

    // コンテキスト収集 (ロックは収集の間だけ保持し、API 呼出中は解放する)
    let (model, user_context, payload) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        let model = current_model(&conn);
        let user_context = repos::get_setting(&conn, "ai_user_context")
            .map_err(db_err)?
            .unwrap_or_default();
        let payload = context_builder::build_payload(&conn, &note)?;
        (model, user_context, payload)
    };

    let plan_model = model.clone();
    let plan = tauri::async_runtime::spawn_blocking(move || {
        gemini_api::generate_plan(&api_key, &plan_model, &user_context, &payload)
    })
    .await
    .map_err(|e| format!("生成処理に失敗しました: {e}"))??;

    {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        plan_store::save_plan(&conn, &note, &model, &plan)?;
    }

    let state = app.state::<AppState>();
    let conn = state.db.lock().unwrap();
    plan_store::latest_plan_today(&conn)?
        .ok_or("プランの保存に失敗しました。".into())
}

/// 今日の保存済みプラン (ローカル、即時)。
#[tauri::command]
pub fn get_daily_plan(state: State<'_, AppState>) -> CmdResult<Option<plan_store::StoredPlan>> {
    let conn = state.db.lock().unwrap();
    plan_store::latest_plan_today(&conn)
}
