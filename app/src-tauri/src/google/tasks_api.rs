//! Google Tasks REST API v1 の薄いクライアント (blocking)。

use serde::Deserialize;
use std::time::Duration;

const BASE: &str = "https://tasks.googleapis.com/tasks/v1";

#[derive(Debug, Deserialize)]
pub struct GTaskList {
    pub id: String,
    pub title: String,
    pub updated: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GTask {
    pub id: String,
    pub title: Option<String>,
    pub notes: Option<String>,
    pub due: Option<String>,
    pub status: String,
    pub position: Option<String>,
    pub updated: Option<String>,
}

#[derive(Deserialize)]
struct Page<T> {
    items: Option<Vec<T>>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Debug)]
pub enum ApiError {
    /// 404/410: リモートに存在しない
    Gone,
    /// 401: トークン失効
    Unauthorized,
    Other(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Gone => write!(f, "リモートに存在しません (404)"),
            ApiError::Unauthorized => write!(f, "認証が失効しています (401)"),
            ApiError::Other(msg) => write!(f, "{msg}"),
        }
    }
}

fn client() -> Result<reqwest::blocking::Client, ApiError> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
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
        _ => {
            let text = response.text().unwrap_or_default();
            Err(ApiError::Other(format!("Google Tasks API エラー ({status}): {text}")))
        }
    }
}

pub fn list_tasklists(token: &str) -> Result<Vec<GTaskList>, ApiError> {
    let client = client()?;
    let mut items = Vec::new();
    let mut page_token: Option<String> = None;
    loop {
        let mut request = client
            .get(format!("{BASE}/users/@me/lists"))
            .bearer_auth(token)
            .query(&[("maxResults", "100")]);
        if let Some(ref pt) = page_token {
            request = request.query(&[("pageToken", pt)]);
        }
        let response = check(request.send().map_err(net_err)?)?;
        let page: Page<GTaskList> = response.json().map_err(|e| ApiError::Other(e.to_string()))?;
        items.extend(page.items.unwrap_or_default());
        match page.next_page_token {
            Some(pt) => page_token = Some(pt),
            None => break,
        }
    }
    Ok(items)
}

/// 未完了タスクを全ページ取得 (showCompleted=false, showHidden=true — GAS 版と同条件)。
pub fn list_open_tasks(token: &str, task_list_id: &str) -> Result<Vec<GTask>, ApiError> {
    let client = client()?;
    let mut items = Vec::new();
    let mut page_token: Option<String> = None;
    loop {
        let mut request = client
            .get(format!("{BASE}/lists/{task_list_id}/tasks"))
            .bearer_auth(token)
            .query(&[
                ("maxResults", "100"),
                ("showCompleted", "false"),
                ("showHidden", "true"),
                ("showDeleted", "false"),
            ]);
        if let Some(ref pt) = page_token {
            request = request.query(&[("pageToken", pt)]);
        }
        let response = check(request.send().map_err(net_err)?)?;
        let page: Page<GTask> = response.json().map_err(|e| ApiError::Other(e.to_string()))?;
        items.extend(page.items.unwrap_or_default());
        match page.next_page_token {
            Some(pt) => page_token = Some(pt),
            None => break,
        }
    }
    Ok(items)
}

/// tasks.patch。body は enqueue 時に確定した JSON (spec §6.3、冪等)。
pub fn patch_task(
    token: &str,
    task_list_id: &str,
    task_id: &str,
    body: &serde_json::Value,
) -> Result<(), ApiError> {
    let client = client()?;
    let response = client
        .patch(format!("{BASE}/lists/{task_list_id}/tasks/{task_id}"))
        .bearer_auth(token)
        .json(body)
        .send()
        .map_err(net_err)?;
    check(response)?;
    Ok(())
}

fn net_err(e: reqwest::Error) -> ApiError {
    ApiError::Other(format!("ネットワークエラー: {e}"))
}
