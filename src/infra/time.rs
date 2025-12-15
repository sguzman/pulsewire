//! Helpers for formatting epoch milliseconds into human-readable strings in a given timezone.
use chrono::{DateTime, TimeZone, Utc};
use chrono_tz::Tz;

pub fn format_epoch_ms(ms: i64, zone: &Tz) -> String {
    let dt_utc: DateTime<Utc> = Utc
        .timestamp_millis_opt(ms)
        .single()
        .unwrap_or_else(|| Utc.timestamp_millis_opt(0).single().unwrap());
    let dt_local = dt_utc.with_timezone(zone);
    dt_local.format("%Y-%m-%d %H:%M:%S%.3f %Z").to_string()
}

pub fn epoch_ms_to_iso(ms: i64, zone: &Tz) -> String {
    let dt_utc: DateTime<Utc> = Utc
        .timestamp_millis_opt(ms)
        .single()
        .unwrap_or_else(|| Utc.timestamp_millis_opt(0).single().unwrap());
    dt_utc.with_timezone(zone).to_rfc3339()
}
