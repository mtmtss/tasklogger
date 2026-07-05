//! Google Sheets API v4 の薄いクライアント (blocking)。ログ同期専用の最小限。

use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

use super::tasks_api::ApiError;

const BASE: &str = "https://sheets.googleapis.com/v4/spreadsheets";

fn client() -> Result<reqwest::blocking::Client, ApiError> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| ApiError::Other(e.to_string()))
}

fn check(response: reqwest::blocking::Response) -> Result<reqwest::blocking::Response, ApiError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    match status.as_u16() {
        404 | 410 => Err(ApiError::Gone),
        401 => Err(ApiError::Unauthorized),
        403 => {
            let text = response.text().unwrap_or_default();
            if text.contains("insufficient") || text.contains("ACCESS_TOKEN_SCOPE_INSUFFICIENT") {
                Err(ApiError::Other(
                    "Sheets への権限がありません。設定ページで Google 接続をやり直してください (権限追加のため)。".into(),
                ))
            } else {
                Err(ApiError::Other(format!("Sheets API エラー (403): {text}")))
            }
        }
        _ => {
            let text = response.text().unwrap_or_default();
            Err(ApiError::Other(format!("Sheets API エラー ({status}): {text}")))
        }
    }
}

#[derive(Deserialize)]
struct ValueRange {
    values: Option<Vec<Vec<serde_json::Value>>>,
}

/// 範囲の値を文字列の 2 次元配列で取得する。
pub fn values_get(
    token: &str,
    spreadsheet_id: &str,
    range: &str,
) -> Result<Vec<Vec<String>>, ApiError> {
    let client = client()?;
    let response = client
        .get(format!("{BASE}/{spreadsheet_id}/values/{range}"))
        .bearer_auth(token)
        .query(&[("valueRenderOption", "UNFORMATTED_VALUE")])
        .send()
        .map_err(net_err)?;
    let body: ValueRange = check(response)?
        .json()
        .map_err(|e| ApiError::Other(e.to_string()))?;
    Ok(body
        .values
        .unwrap_or_default()
        .into_iter()
        .map(|row| row.into_iter().map(cell_to_string).collect())
        .collect())
}

/// 行を末尾に追記する (RAW: 値をそのまま書く)。
pub fn values_append(
    token: &str,
    spreadsheet_id: &str,
    range: &str,
    rows: &[Vec<String>],
) -> Result<(), ApiError> {
    if rows.is_empty() {
        return Ok(());
    }
    let client = client()?;
    let response = client
        .post(format!("{BASE}/{spreadsheet_id}/values/{range}:append"))
        .bearer_auth(token)
        .query(&[
            ("valueInputOption", "RAW"),
            ("insertDataOption", "INSERT_ROWS"),
        ])
        .json(&json!({ "values": rows }))
        .send()
        .map_err(net_err)?;
    check(response)?;
    Ok(())
}

/// 範囲の値を上書きする (ヘッダ行の整備用)。
pub fn values_update(
    token: &str,
    spreadsheet_id: &str,
    range: &str,
    rows: &[Vec<String>],
) -> Result<(), ApiError> {
    let client = client()?;
    let response = client
        .put(format!("{BASE}/{spreadsheet_id}/values/{range}"))
        .bearer_auth(token)
        .query(&[("valueInputOption", "RAW")])
        .json(&json!({ "values": rows }))
        .send()
        .map_err(net_err)?;
    check(response)?;
    Ok(())
}

/// シート名一覧を取得する。
pub fn list_sheet_titles(token: &str, spreadsheet_id: &str) -> Result<Vec<String>, ApiError> {
    #[derive(Deserialize)]
    struct Meta {
        sheets: Option<Vec<SheetEntry>>,
    }
    #[derive(Deserialize)]
    struct SheetEntry {
        properties: SheetProps,
    }
    #[derive(Deserialize)]
    struct SheetProps {
        title: String,
    }

    let client = client()?;
    let response = client
        .get(format!("{BASE}/{spreadsheet_id}"))
        .bearer_auth(token)
        .query(&[("fields", "sheets.properties.title")])
        .send()
        .map_err(net_err)?;
    let meta: Meta = check(response)?
        .json()
        .map_err(|e| ApiError::Other(e.to_string()))?;
    Ok(meta
        .sheets
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.properties.title)
        .collect())
}

/// シートを追加する。
pub fn add_sheet(token: &str, spreadsheet_id: &str, title: &str) -> Result<(), ApiError> {
    let client = client()?;
    let response = client
        .post(format!("{BASE}/{spreadsheet_id}:batchUpdate"))
        .bearer_auth(token)
        .json(&json!({
            "requests": [{ "addSheet": { "properties": { "title": title } } }]
        }))
        .send()
        .map_err(net_err)?;
    check(response)?;
    Ok(())
}

/// 新規スプレッドシートを作成して ID を返す。
pub fn create_spreadsheet(token: &str, title: &str, sheet_title: &str) -> Result<String, ApiError> {
    #[derive(Deserialize)]
    struct Created {
        #[serde(rename = "spreadsheetId")]
        spreadsheet_id: String,
    }

    let client = client()?;
    let response = client
        .post(BASE)
        .bearer_auth(token)
        .json(&json!({
            "properties": { "title": title },
            "sheets": [{ "properties": { "title": sheet_title } }]
        }))
        .send()
        .map_err(net_err)?;
    let created: Created = check(response)?
        .json()
        .map_err(|e| ApiError::Other(e.to_string()))?;
    Ok(created.spreadsheet_id)
}

/// UNFORMATTED_VALUE は数値/真偽値が JSON 型で返るため文字列へ正規化する。
fn cell_to_string(value: serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s,
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn net_err(e: reqwest::Error) -> ApiError {
    ApiError::Other(format!("ネットワークエラー: {e}"))
}
