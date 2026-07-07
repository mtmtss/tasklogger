//! Gemini API (Google Generative Language API) クライアント (AI 拡張仕様 §4)。
//! blocking reqwest 直叩き。responseSchema で出力 JSON を保証する。
//! キーは Google AI Studio (https://aistudio.google.com/) で無料発行できる。

use serde_json::{json, Value};
use std::time::Duration;

const API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";
pub const DEFAULT_MODEL: &str = "gemini-2.5-flash";
pub const ALLOWED_MODELS: [&str; 3] = [
    "gemini-2.5-flash",
    "gemini-2.5-flash-lite",
    "gemini-2.5-pro",
];

const KEYRING_SERVICE: &str = "TaskLogger";
const KEYRING_USER: &str = "gemini_api_key";

/// 「今日の作戦」の役割定義と出力ルール。
const PLAN_SYSTEM: &str = "\
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

/// 「日次レビュー」の役割定義と出力ルール (AI 拡張仕様 §10 第2弾)。
const REVIEW_SYSTEM: &str = "\
あなたはタスク管理アプリ TaskLogger の「振り返り」コーチである。\
その日の作戦(あれば)と実際の作業記録を突合し、責めずに次につなげる短い振り返りを書く。\n\
\n\
原則:\n\
- done は今日前進したことを 1〜5 件、具体的に(実績時間や完了に触れる)\n\
- incomplete は未達・中断を挙げ、reason で理由を推測する。自動中断(無操作・スリープ)が多い場合は集中の妨げや離席として扱い、本人を責めない\n\
- tomorrow は明日に回す候補を 0〜5 件。既存タスクに対応するなら taskListId / taskId を返す(なければ null)\n\
- research_progress は研究・制作面の前進を 1 文で(なければ空文字)\n\
- summary は 1〜3 文の総括。計画と実績の乖離があれば一言触れ、明日への現実的な一歩を添える\n\
- すべて日本語で、励ましつつ淡々と書く";

/// 出力スキーマ (AI 拡張仕様 §3.3)。Gemini の responseSchema 形式 (OpenAPI 風、型は大文字)。
fn plan_schema() -> Value {
    let task_item = json!({
        "type": "OBJECT",
        "properties": {
            "taskListId": {"type": "STRING", "nullable": true},
            "taskId": {"type": "STRING", "nullable": true},
            "title": {"type": "STRING"},
            "firstStep": {"type": "STRING"},
            "estimatedMinutes": {"type": "INTEGER"},
            "reason": {"type": "STRING"}
        },
        "required": ["title", "firstStep", "estimatedMinutes", "reason"]
    });
    json!({
        "type": "OBJECT",
        "properties": {
            "must_do": {"type": "ARRAY", "items": task_item},
            "if_possible": {"type": "ARRAY", "items": task_item},
            "five_minute": {"type": "ARRAY", "items": task_item},
            "not_today": {
                "type": "ARRAY",
                "items": {
                    "type": "OBJECT",
                    "properties": {
                        "taskListId": {"type": "STRING", "nullable": true},
                        "taskId": {"type": "STRING", "nullable": true},
                        "title": {"type": "STRING"},
                        "reason": {"type": "STRING"}
                    },
                    "required": ["title", "reason"]
                }
            },
            "advice": {"type": "STRING"}
        },
        "required": ["must_do", "if_possible", "five_minute", "not_today", "advice"]
    })
}

/// 日次レビューの出力スキーマ。
fn review_schema() -> Value {
    let tomorrow_item = json!({
        "type": "OBJECT",
        "properties": {
            "taskListId": {"type": "STRING", "nullable": true},
            "taskId": {"type": "STRING", "nullable": true},
            "title": {"type": "STRING"},
            "reason": {"type": "STRING"}
        },
        "required": ["title", "reason"]
    });
    json!({
        "type": "OBJECT",
        "properties": {
            "done": {"type": "ARRAY", "items": {"type": "STRING"}},
            "incomplete": {
                "type": "ARRAY",
                "items": {
                    "type": "OBJECT",
                    "properties": {
                        "title": {"type": "STRING"},
                        "reason": {"type": "STRING"}
                    },
                    "required": ["title", "reason"]
                }
            },
            "tomorrow": {"type": "ARRAY", "items": tomorrow_item},
            "research_progress": {"type": "STRING"},
            "summary": {"type": "STRING"}
        },
        "required": ["done", "incomplete", "tomorrow", "research_progress", "summary"]
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

/// リクエストボディの組立 (テスト対象)。role_system は用途別の役割定義、schema は出力スキーマ。
pub fn build_request_body(
    role_system: &str,
    user_context: &str,
    schema: Value,
    payload: &str,
) -> Value {
    let system = format!(
        "{role_system}\n\n# ユーザープロファイル\n{}",
        if user_context.is_empty() {
            "(未設定)"
        } else {
            user_context
        }
    );
    json!({
        "systemInstruction": {"parts": [{"text": system}]},
        "contents": [{"role": "user", "parts": [{"text": payload}]}],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseSchema": schema,
            "maxOutputTokens": 8192
        }
    })
}

/// レスポンスからプラン JSON を取り出す (テスト対象)。
pub fn extract_plan(response: &Value) -> Result<Value, String> {
    // 入力自体がブロックされた場合
    if let Some(reason) = response["promptFeedback"]["blockReason"].as_str() {
        return Err(format!(
            "AI が応答を生成できませんでした ({reason})。内容を変えて再生成してください。"
        ));
    }

    let candidate = response["candidates"]
        .as_array()
        .and_then(|c| c.first())
        .ok_or("応答が空です。再生成してください。")?;

    match candidate["finishReason"].as_str().unwrap_or("STOP") {
        "STOP" => {}
        "MAX_TOKENS" => return Err("応答が長すぎて途切れました。再生成してください。".into()),
        reason => {
            return Err(format!(
                "AI が応答を完了できませんでした ({reason})。再生成してください。"
            ))
        }
    }

    let text = candidate["content"]["parts"]
        .as_array()
        .and_then(|parts| {
            parts
                .iter()
                .filter_map(|p| p["text"].as_str())
                .last() // thought パートが先行する場合があるため最後のテキストを採る
        })
        .ok_or("応答にテキストが含まれていません。")?;

    serde_json::from_str(text).map_err(|e| format!("プランの解析に失敗しました: {e}"))
}

/// 今日の作戦を生成 (blocking)。
pub fn generate_plan(
    api_key: &str,
    model: &str,
    user_context: &str,
    payload: &str,
) -> Result<Value, String> {
    generate(api_key, model, PLAN_SYSTEM, user_context, plan_schema(), payload)
}

/// 日次レビューを生成 (blocking)。
pub fn generate_review(
    api_key: &str,
    model: &str,
    user_context: &str,
    payload: &str,
) -> Result<Value, String> {
    generate(api_key, model, REVIEW_SYSTEM, user_context, review_schema(), payload)
}

/// 汎用生成 (blocking)。429/5xx は 1 回だけリトライする。
fn generate(
    api_key: &str,
    model: &str,
    role_system: &str,
    user_context: &str,
    schema: Value,
    payload: &str,
) -> Result<Value, String> {
    let body = build_request_body(role_system, user_context, schema, payload);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;
    let url = format!("{API_BASE}/models/{model}:generateContent");

    let mut last_err = String::new();
    for attempt in 0..2 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_secs(5));
        }
        let response = client
            .post(&url)
            .header("x-goog-api-key", api_key)
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
                    400 | 401 | 403 => {
                        let message =
                            value["error"]["message"].as_str().unwrap_or("キーが無効です");
                        return Err(format!(
                            "Gemini API エラー ({status}): {message}。設定ページでキーを確認してください。"
                        ));
                    }
                    429 => {
                        last_err = "利用上限に達しました (無料枠の場合は時間をおいて再試行してください)。".into();
                        continue;
                    }
                    500..=599 => {
                        last_err = format!("Gemini API が混雑しています ({status})。しばらくして再試行してください。");
                        continue;
                    }
                    _ => {
                        let message = value["error"]["message"].as_str().unwrap_or("不明なエラー");
                        return Err(format!("Gemini API エラー ({status}): {message}"));
                    }
                }
            }
            Err(e) => {
                last_err = format!("Gemini API に接続できません: {e}");
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
        .header("x-goog-api-key", api_key)
        .send()
        .map_err(|e| format!("接続できません: {e}"))?;

    match response.status().as_u16() {
        200 => Ok(()),
        400 | 401 | 403 => Err("API キーが無効です。Google AI Studio で発行したキーを確認してください。".into()),
        404 => Err(format!("モデル {model} が見つかりません。")),
        status => Err(format!("接続テスト失敗 ({status})")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_body_shape() {
        let body = build_request_body(PLAN_SYSTEM, "研究者。", plan_schema(), "今日のタスク: なし");
        // JSON 出力の強制
        assert_eq!(
            body["generationConfig"]["responseMimeType"],
            "application/json"
        );
        assert_eq!(
            body["generationConfig"]["responseSchema"]["type"],
            "OBJECT"
        );
        // システム指示にプロファイルが含まれる
        let system = body["systemInstruction"]["parts"][0]["text"]
            .as_str()
            .unwrap();
        assert!(system.contains("研究者。"));
        assert!(system.contains("今日の作戦"));
        // ユーザーペイロード
        assert_eq!(
            body["contents"][0]["parts"][0]["text"],
            "今日のタスク: なし"
        );
    }

    #[test]
    fn review_body_uses_review_schema() {
        let body = build_request_body(REVIEW_SYSTEM, "", review_schema(), "実績: ...");
        let props = &body["generationConfig"]["responseSchema"]["properties"];
        assert!(props.get("done").is_some());
        assert!(props.get("summary").is_some());
        let system = body["systemInstruction"]["parts"][0]["text"]
            .as_str()
            .unwrap();
        assert!(system.contains("振り返り"));
    }

    #[test]
    fn extract_plan_happy_path() {
        let response = serde_json::json!({
            "candidates": [{
                "finishReason": "STOP",
                "content": {"parts": [
                    {"text": "{\"must_do\":[],\"if_possible\":[],\"five_minute\":[],\"not_today\":[],\"advice\":\"ok\"}"}
                ]}
            }]
        });
        let plan = extract_plan(&response).unwrap();
        assert_eq!(plan["advice"], "ok");
    }

    #[test]
    fn extract_plan_blocked() {
        let response = serde_json::json!({
            "promptFeedback": {"blockReason": "SAFETY"},
            "candidates": []
        });
        assert!(extract_plan(&response).is_err());
    }

    #[test]
    fn extract_plan_max_tokens() {
        let response = serde_json::json!({
            "candidates": [{"finishReason": "MAX_TOKENS", "content": {"parts": []}}]
        });
        assert!(extract_plan(&response).is_err());
    }
}
