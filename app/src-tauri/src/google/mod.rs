//! Google Tasks 連携 (M2 で実装)。
//! M1 時点ではスタブ: 同期キューには積まれるが送信はしない (オフライン動作と同じ)。

/// 同期ワーカーを起動する (M2 で実装)。
pub fn start_sync_worker(_app: tauri::AppHandle) {}

/// 変更操作の直後に同期を促す (M2 で実装)。
pub fn kick_sync(_app: &tauri::AppHandle) {}
