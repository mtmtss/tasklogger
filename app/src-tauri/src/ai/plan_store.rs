//! daily_plans の保存・取得 (AI 拡張仕様 §5.1)。追記型。

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::domain::time;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredPlan {
    pub plan_date: String,
    pub generated_at: String,
    pub input_note: String,
    pub model: String,
    pub plan: serde_json::Value,
}

pub fn save_plan(
    conn: &Connection,
    note: &str,
    model: &str,
    plan_json: &serde_json::Value,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO daily_plans (plan_date, generated_at, input_note, model, plan_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            time::today_jst(),
            time::to_iso(&time::now_utc()),
            note,
            model,
            plan_json.to_string()
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 今日の最新プラン。
pub fn latest_plan_today(conn: &Connection) -> Result<Option<StoredPlan>, String> {
    conn.query_row(
        "SELECT plan_date, generated_at, input_note, model, plan_json
         FROM daily_plans WHERE plan_date = ?1
         ORDER BY generated_at DESC LIMIT 1",
        params![time::today_jst()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        },
    )
    .optional()
    .map_err(|e| e.to_string())?
    .map(|(plan_date, generated_at, input_note, model, plan_json)| {
        serde_json::from_str(&plan_json)
            .map(|plan| StoredPlan {
                plan_date,
                generated_at,
                input_note,
                model,
                plan,
            })
            .map_err(|e| format!("保存済みプランの解析に失敗しました: {e}"))
    })
    .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_load_latest() {
        let conn = crate::db::open_in_memory().unwrap();
        assert!(latest_plan_today(&conn).unwrap().is_none());

        let plan1 = serde_json::json!({"advice": "one"});
        let plan2 = serde_json::json!({"advice": "two"});
        save_plan(&conn, "", "claude-opus-4-8", &plan1).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        save_plan(&conn, "外出あり", "claude-opus-4-8", &plan2).unwrap();

        let latest = latest_plan_today(&conn).unwrap().unwrap();
        assert_eq!(latest.plan["advice"], "two");
        assert_eq!(latest.input_note, "外出あり");
    }
}
