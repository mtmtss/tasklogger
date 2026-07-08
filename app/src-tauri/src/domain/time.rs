use chrono::{DateTime, Utc};
use chrono_tz::Asia::Tokyo;

/// アプリ全体のタイムゾーン (spec §10)。
pub const TIMEZONE: chrono_tz::Tz = Tokyo;

pub fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

pub fn to_iso(dt: &DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// JST での yyyy-MM-dd (logDate 規則: startTime の JST 日付)。
pub fn jst_date_text(dt: &DateTime<Utc>) -> String {
    dt.with_timezone(&TIMEZONE).format("%Y-%m-%d").to_string()
}

pub fn today_jst() -> String {
    jst_date_text(&now_utc())
}

pub fn parse_iso(text: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// GAS 版と同じ丸め: 分 = ceil(秒 / 60)、0 秒は 0 分。
pub fn ceil_minutes(seconds: i64) -> i64 {
    if seconds <= 0 {
        0
    } else {
        (seconds + 59) / 60
    }
}

/// Google Tasks の due (RFC3339, date-only) を today 用の文字列にする。
pub fn today_due_value() -> String {
    format!("{}T00:00:00.000Z", today_jst())
}

/// due 文字列の先頭 10 文字が今日 (JST) かどうか。
pub fn is_due_today(due: &Option<String>) -> bool {
    match due {
        Some(d) => d.len() >= 10 && d[0..10] == today_jst(),
        None => false,
    }
}

/// due 文字列の先頭 10 文字が今日より前 (期限切れ) かどうか。
pub fn is_overdue(due: &Option<String>) -> bool {
    match due {
        Some(d) => d.len() >= 10 && &d[0..10] < today_jst().as_str(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_overdue_cases() {
        assert!(!is_overdue(&None));
        assert!(!is_overdue(&Some(format!("{}T00:00:00.000Z", today_jst()))));
        assert!(is_overdue(&Some("2000-01-01T00:00:00.000Z".to_string())));
        assert!(!is_overdue(&Some("2999-01-01T00:00:00.000Z".to_string())));
    }
}
