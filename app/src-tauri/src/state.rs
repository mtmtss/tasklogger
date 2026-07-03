use rusqlite::Connection;
use std::sync::Mutex;

/// アプリ全体の状態。真実は SQLite にあり、Connection を Mutex で直列化する。
pub struct AppState {
    pub db: Mutex<Connection>,
}

impl AppState {
    pub fn new(conn: Connection) -> Self {
        Self {
            db: Mutex::new(conn),
        }
    }
}
