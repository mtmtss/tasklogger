//! OAuth 2.0 installed-app flow (PKCE + loopback リダイレクト, spec §6.1)。
//! refresh token は Windows Credential Manager (keyring)、access token はメモリのみ。

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;

use base64::Engine;
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};

const AUTH_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const SCOPE: &str = "https://www.googleapis.com/auth/tasks";
const KEYRING_SERVICE: &str = "TaskLogger";
const KEYRING_USER: &str = "google_refresh_token";

pub struct TokenSet {
    pub access_token: String,
    /// UNIX epoch 秒
    pub expires_at: i64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
    refresh_token: Option<String>,
}

fn b64url(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub fn save_refresh_token(token: &str) -> Result<(), String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .and_then(|e| e.set_password(token))
        .map_err(|e| format!("トークンの保存に失敗しました: {e}"))
}

pub fn load_refresh_token() -> Option<String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .ok()?
        .get_password()
        .ok()
}

pub fn delete_refresh_token() {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        let _ = entry.delete_credential();
    }
}

/// 認可フロー全体を実行し、(TokenSet, refresh_token) を返す。ブロッキング。
pub fn run_authorization_flow(
    client_id: &str,
    client_secret: &str,
) -> Result<(TokenSet, String), String> {
    // PKCE verifier / challenge
    let mut verifier_bytes = [0u8; 48];
    rand::thread_rng().fill_bytes(&mut verifier_bytes);
    let verifier = b64url(&verifier_bytes);
    let challenge = b64url(&Sha256::digest(verifier.as_bytes()));

    let mut state_bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut state_bytes);
    let state = b64url(&state_bytes);

    // loopback リダイレクト受け口 (ランダムポート)
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("ローカルポートの確保に失敗しました: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| e.to_string())?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}");

    let auth_url = format!(
        "{AUTH_ENDPOINT}?{}",
        url::form_urlencoded::Serializer::new(String::new())
            .append_pair("client_id", client_id)
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("response_type", "code")
            .append_pair("scope", SCOPE)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("access_type", "offline")
            .append_pair("prompt", "consent")
            .append_pair("state", &state)
            .finish()
    );

    // 既定ブラウザで認可画面を開く
    opener_open(&auth_url)?;

    // ブラウザからのリダイレクトを 1 回だけ受ける (タイムアウト 180 秒)
    listener
        .set_nonblocking(false)
        .map_err(|e| e.to_string())?;
    let code = accept_authorization_code(&listener, &state)?;

    // トークン交換
    let params = [
        ("code", code.as_str()),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("redirect_uri", redirect_uri.as_str()),
        ("grant_type", "authorization_code"),
        ("code_verifier", verifier.as_str()),
    ];
    let response = http_post_form(TOKEN_ENDPOINT, &params)?;
    let token: TokenResponse = serde_json::from_str(&response)
        .map_err(|e| format!("トークン応答の解析に失敗しました: {e} / {response}"))?;

    let refresh_token = token
        .refresh_token
        .ok_or("refresh_token が返されませんでした。Google 側の承認をやり直してください。")?;

    Ok((
        TokenSet {
            access_token: token.access_token,
            expires_at: chrono::Utc::now().timestamp() + token.expires_in,
        },
        refresh_token,
    ))
}

/// refresh token で access token を更新する。ブロッキング。
pub fn refresh_access_token(
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<TokenSet, String> {
    let params = [
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
    ];
    let response = http_post_form(TOKEN_ENDPOINT, &params)?;
    let token: TokenResponse = serde_json::from_str(&response)
        .map_err(|e| format!("トークン更新応答の解析に失敗しました: {e} / {response}"))?;
    Ok(TokenSet {
        access_token: token.access_token,
        expires_at: chrono::Utc::now().timestamp() + token.expires_in,
    })
}

fn accept_authorization_code(listener: &TcpListener, expected_state: &str) -> Result<String, String> {
    listener
        .set_nonblocking(false)
        .map_err(|e| e.to_string())?;
    // accept は認可完了までブロックする。3 分であきらめる
    listener
        .local_addr()
        .map_err(|e| e.to_string())?;
    let (mut stream, _) = {
        listener
            .set_nonblocking(true)
            .map_err(|e| e.to_string())?;
        let deadline = std::time::Instant::now() + Duration::from_secs(180);
        loop {
            match listener.accept() {
                Ok(conn) => break Ok(conn),
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if std::time::Instant::now() > deadline {
                        break Err("認証がタイムアウトしました。もう一度お試しください。".to_string());
                    }
                    std::thread::sleep(Duration::from_millis(200));
                }
                Err(e) => break Err(format!("コールバックの受信に失敗しました: {e}")),
            }
        }
    }?;

    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
    let request = String::from_utf8_lossy(&buf[..n]);

    // "GET /?code=...&state=... HTTP/1.1"
    let query = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|path| path.split_once('?').map(|(_, q)| q.to_string()))
        .unwrap_or_default();

    let mut code = None;
    let mut state = None;
    let mut error = None;
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            "error" => error = Some(value.into_owned()),
            _ => {}
        }
    }

    let body = if code.is_some() && state.as_deref() == Some(expected_state) {
        "<html><body style='font-family:sans-serif'><h3>TaskLogger の接続が完了しました</h3><p>このタブは閉じてください。</p></body></html>"
    } else {
        "<html><body style='font-family:sans-serif'><h3>接続に失敗しました</h3><p>アプリに戻ってやり直してください。</p></body></html>"
    };
    let _ = stream.write_all(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .as_bytes(),
    );

    if let Some(err) = error {
        return Err(format!("Google が認可を拒否しました: {err}"));
    }
    if state.as_deref() != Some(expected_state) {
        return Err("state が一致しません。認証をやり直してください。".into());
    }
    code.ok_or("認可コードが取得できませんでした。".into())
}

fn opener_open(url: &str) -> Result<(), String> {
    // Windows: 既定ブラウザで開く
    std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("ブラウザを開けませんでした: {e}"))
}

fn http_post_form(url: &str, params: &[(&str, &str)]) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .post(url)
        .form(params)
        .send()
        .map_err(|e| format!("Google への接続に失敗しました: {e}"))?;
    let status = response.status();
    let text = response.text().map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Google がエラーを返しました ({status}): {text}"));
    }
    Ok(text)
}
