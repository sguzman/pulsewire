//! Small helpers shared across Postgres repo modules.
use chrono::{offset::Offset, DateTime, FixedOffset, TimeZone, Utc};
use chrono_tz::Tz;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn ts_from_ms(ms: i64, tz: &Tz) -> DateTime<FixedOffset> {
    let local = tz
        .timestamp_millis_opt(ms)
        .single()
        .unwrap_or_else(|| tz.timestamp_millis_opt(0).unwrap());
    let offset = local.offset().fix();
    DateTime::from_naive_utc_and_offset(local.naive_utc(), offset)
}

pub fn ts_from_ms_opt(ms: Option<i64>, tz: &Tz) -> Option<DateTime<FixedOffset>> {
    ms.map(|v| ts_from_ms(v, tz))
}

pub fn ms_from_ts(ts: Option<DateTime<Utc>>) -> Option<i64> {
    ts.map(|dt| dt.timestamp_millis())
}

pub fn chunk_statements(schema: &str) -> impl Iterator<Item = &str> {
    schema.split(';').map(str::trim).filter(|s| !s.is_empty())
}
