//! Claude API (Anthropic Messages API) クライアント (AI 拡張仕様 §4)。
//! blocking reqwest 直叩き。structured outputs で出力 JSON を保証する。

use serde_json::{json, Value};
use std::time::Duration;

const API_BASE: &str = "https://api.anthropic.com/v1";
const API_VERSION: &str = "2023-06-01";
pub const DEFAULT_MODEL: &str = "claude-opus-4-8";
pub const ALLOWED_MODELS: [&str; 3] = ["claude-opus-4-8", "claude-sonnet-5", "claude-haiku-4-5"];

const KEYRING_SERVICE: &str = "TaskLogger";
const KEYRING_USER: &str = "anthropic_api_key";

/// 役割定義と出力ルール (安定プレフィックス。変更するとキャッシュが切れる点に注意)。
const SYSTEM_FIXED: &str = "\
あなたはタスク管理アプリ TaskLogger の「今日の作戦」コーチである。\
ユーザーの実際の作業記録(実績時間・未割当時間・中断パターン)に基づき、\
今日を現実的なサイズに圧縮した実行計画を立てる。\n\
\n\
原則:\n\
- 過剰計画を戒める。「必ずやる」は 1〜3 件に絞る。実績ペース(日別実働時間)を超える計画を立てない\n\
- 重いタスクはそのまま置かず、今日踏み出せる「最初の一手」(firstStep) に分解する\n\
- 見積もり (estimatedMinutes) は過去実績があればそれを優先し、なければ控えめに推定する\n\
- 「今日やらない」(not_today) には必ず安心できる理由を付け、退避を正当化する\n\
- 締切・停滞日数・当日の状態メモを考慮する。停滞している重要タスクは最初の一手で再起動を促す\n\
- 提案が既存タスクに対応する場合は taskListId / taskId をそのまま返す。分解した一手など対応タスクがない場合は null にする\n\
- advice は 1〜3 文で、実績データに基づく具体的な根拠を含める\n\
- すべて日本語で書く";

/// 出力 JSON スキーマ (AI 拡張仕様 §3.3)。
fn plan_schema() -> Value {
    let nullable_string = json!({ "anyOf": [ {"type": "string"}, {"type": "null"} ] });
    let task_item = json!({
        "type": "object",
        "properties": {
            "taskListId": nullable_string,
            "taskId": nullable_string,
            "title": {"type": "string"},
            "firstStep": {"type": "string"},
            "estimatedMinutes": {"type": "integer"},
            "reason": {"type": "string"}
        },
        "required": ["taskListId", "taskId", "title", "firstStep", "estimatedMinutes", "reason"],
        "additionalProperties": false
    });
    json!({
        "type": "object",
        "properties": {
            "must_do": {"type": "array", "items": task_item},
            "if_possible": {"type": "array", "items": task_item},
            "five_minute": {"type": "array", "items": task_item},
            "not_today": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "taskListId": nullable_string,
                        "taskId": nullable_string,
                        "title": {"type": "string"},
                        "reason": {"type": "string"}
                    },
                    "required": ["taskListId", "taskId", "title", "reason"],
                    "additionalProperties": false
                }
            },
            "advice": {"type": "string"}
        },
        "required": ["must_do", "if_possible", "five_minute", "not_today", "advice"],
        "additionalProperties": false
    })
}

pub fn save_api_key(key: &str) -> Result<(), String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .and_then(|e| e.set_password(key))
        .map_err(|e| format!("API キーの保存に失敗しました: {e}"))
}

pub fn load_api_key() -> Option<String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .ok()?
        .get_password()
        .ok()
}

pub fn delete_api_key() {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        let _ = entry.delete_credential();
    }
}

/// リクエストボディの組立 (テスト対象)。
/// user_context (準固定プロファイル) の末尾にキャッシュ境界を置く (仕様 §4.2)。
pub fn build_request_body(model: &str, user_context: &str, payload: &str) -> Value {
    json!({
        "model": model,
        "max_tokens": 4096,
        "thinking": {"type": "adaptive"},
        "system": [
            {"type": "text", "text": SYSTEM_FIXED},
            {
                "type": "text",
                "text": if user_context.is_empty() { "(ユーザープロファイル未設定)" } else { user_context },
                "cache_control": {"type": "ephemeral"}
            }
        ],
        "messages": [{"role": "user", "content": payload}],
        "output_config": {"format": {"type": "json_schema", "schema": plan_schema()}}
    })
}

/// レスポンスからプラン JSON を取り出す (テスト対象)。
pub fn extract_plan(response: &Value) -> Result<Value, String> {
    let stop_reason = response["stop_reason"].as_str().unwrap_or("");
    if stop_reason == "refusal" {
        return Err("AI が応答を生成できませんでした。内容を変えて再生成してください。".into());
    }
    if stop_reason == "max_tokens" {
        return Err("応答が長すぎて途切れました。再生成してください。".into());
    }

    let text = response["content"]
        .as_array()
        .and_then(|blocks| {
            blocks
                .iter()
                .find(|b| b["type"] == "text")
                .and_then(|b| b["text"].as_str())
        })
        .ok_or("応答にテキストが含まれていません。")?;

    serde_json::from_str(text).map_err(|e| format!("プランの解析に失敗しました: {e}"))
}

/// プラン生成 (blocking)。429/5xx は 1 回だけリトライする。
pub fn generate_plan(
    api_key: &str,
    model: &str,
    user_context: &str,
    payload: &str,
) -> Result<Value, String> {
    let body = build_request_body(model, user_context, payload);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;

    let mut last_err = String::new();
    for attempt in 0..2 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_secs(3));
        }
        let response = client
            .post(format!("{API_BASE}/messages"))
            .header("x-api-key", api_key)
            .header("anthropic-version", API_VERSION)
            .json(&body)
            .send();

        match response {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let value: Value = resp
                    .json()
                    .map_err(|e| format!("応答の読み取りに失敗しました: {e}"))?;
                match status {
                    200 => return extract_plan(&value),
                    401 => return Err("API キーが無効です。設定ページで確認してください。".into()),
                    429 | 500..=599 => {
                        last_err = format!(
                            "Anthropic API が混雑しています ({status})。しばらくして再試行してください。"
                        );
                        continue;
                    }
                    _ => {
                        let message = value["error"]["message"].as_str().unwrap_or("不明なエラー");
                        return Err(format!("Anthropic API エラー ({status}): {message}"));
                    }
                }
            }
            Err(e) => {
                last_err = format!("Anthropic API に接続できません: {e}");
                continue;
            }
        }
    }
    Err(last_err)
}

/// 接続テスト: models API でキーとモデルの疎通を確認する。
pub fn test_connection(api_key: &str, model: &str) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(format!("{API_BASE}/models/{model}"))
        .header("x-api-key", api_key)
        .header("anthropic-version", API_VERSION)
        .send()
        .map_err(|e| format!("接続できません: {e}"))?;

    match response.status().as_u16() {
        200 => Ok(()),
        401 => Err("API キーが無効です。".into()),
        404 => Err(format!("モデル {model} が見つかりません。")),
        status => Err(format!("接続テスト失敗 ({status})")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_body_shape() {
        let body = build_request_body("claude-opus-4-8", "研究者。", "今日のタスク: なし");
        assert_eq!(body["model"], "claude-opus-4-8");
        assert_eq!(body["thinking"]["type"], "adaptive");
        // サンプリングパラメータを含めない (Opus 4.8 では 400 になる)
        assert!(body.get("temperature").is_none());
        assert!(body.get("top_p").is_none());
        // キャッシュ境界はプロファイル側 (2 番目の system ブロック) にある
        assert_eq!(body["system"][1]["cache_control"]["type"], "ephemeral");
        assert!(body["system"][0].get("cache_control").is_none());
        // structured outputs
        assert_eq!(body["output_config"]["format"]["type"], "json_schema");
    }

    #[test]
    fn extract_plan_happy_path() {
        let response = serde_json::json!({
            "stop_reason": "end_turn",
            "content": [
                {"type": "thinking", "thinking": ""},
                {"type": "text", "text": "{\"must_do\":[],\"if_possible\":[],\"five_minute\":[],\"not_today\":[],\"advice\":\"ok\"}"}
            ]
        });
        let plan = extract_plan(&response).unwrap();
        assert_eq!(plan["advice"], "ok");
    }

    #[test]
    fn extract_plan_refusal() {
        let response = serde_json::json!({ "stop_reason": "refusal", "content": [] });
        assert!(extract_plan(&response).is_err());
    }
}
