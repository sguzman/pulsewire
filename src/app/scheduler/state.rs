use std::sync::Arc;

use crate::domain::link_state::{LinkPhase, LinkState, NextAction};
use crate::domain::model::{AppConfig, ErrorKind};
use crate::infra::time::format_epoch_ms;
use crate::ports::random::RandomSource;
use crate::ports::repo::StateRow;

pub fn describe_action(action: &NextAction, cfg: &AppConfig) -> String {
    match action {
        NextAction::SleepUntil { at_ms } => {
            format!("sleep-until {}", format_epoch_ms(*at_ms, &cfg.timezone))
        }
        NextAction::DoHead { .. } => "do-head".to_string(),
        NextAction::DoGet { .. } => "do-get".to_string(),
    }
}

pub fn to_link_state(row: &StateRow, cfg: &AppConfig) -> Option<LinkState> {
    let phase = parse_phase(&row.phase)?;
    Some(LinkState {
        feed_id: row.feed_id.clone(),
        phase,
        last_head_at_ms: row.last_head_at_ms,
        last_head_status: row.last_head_status.map(|x| x as u16),
        last_head_error: row.last_head_error.as_deref().and_then(parse_error),
        last_get_at_ms: row.last_get_at_ms,
        last_get_status: row.last_get_status.map(|x| x as u16),
        last_get_error: row.last_get_error.as_deref().and_then(parse_error),
        etag: row.etag.clone(),
        last_modified_ms: row.last_modified_ms,
        backoff_index: row.backoff_index.max(0) as u32,
        base_poll_seconds: row.base_poll_seconds.max(0) as u64,
        max_poll_seconds: cfg.max_poll_seconds,
        jitter_fraction: cfg.jitter_fraction,
        next_action_at_ms: row.next_action_at_ms,
        jitter_seconds: row.jitter_seconds,
        note: row.note.clone(),
        consecutive_error_count: row.consecutive_error_count.max(0) as u32,
    })
}

pub async fn should_record_history<G: RandomSource>(cfg: &Arc<AppConfig>, rng: &G) -> bool {
    if cfg.state_history_sample_rate >= 1.0 {
        return true;
    }
    if cfg.state_history_sample_rate <= 0.0 {
        return false;
    }
    rng.next_f64().await < cfg.state_history_sample_rate
}

fn parse_error(s: &str) -> Option<ErrorKind> {
    match s {
        "Timeout" => Some(ErrorKind::Timeout),
        "DnsFailure" => Some(ErrorKind::DnsFailure),
        "ConnectionFailure" => Some(ErrorKind::ConnectionFailure),
        "Http4xx" => Some(ErrorKind::Http4xx(0)),
        "Http5xx" => Some(ErrorKind::Http5xx(0)),
        "ParseError" => Some(ErrorKind::ParseError),
        "Unexpected" => Some(ErrorKind::Unexpected),
        _ => parse_http_error(s),
    }
}

fn parse_http_error(s: &str) -> Option<ErrorKind> {
    if let Some(code) = parse_http_code(s, "Http4xx(") {
        return Some(ErrorKind::Http4xx(code));
    }
    if let Some(code) = parse_http_code(s, "Http5xx(") {
        return Some(ErrorKind::Http5xx(code));
    }
    None
}

fn parse_http_code(s: &str, prefix: &str) -> Option<u16> {
    let rest = s.strip_prefix(prefix)?;
    let rest = rest.strip_suffix(')')?;
    rest.parse::<u16>().ok()
}

fn parse_phase(s: &str) -> Option<LinkPhase> {
    match s {
        "NeedsInitialGet" => Some(LinkPhase::NeedsInitialGet),
        "NeedsHead" => Some(LinkPhase::NeedsHead),
        "NeedsGet" => Some(LinkPhase::NeedsGet),
        "Sleeping" => Some(LinkPhase::Sleeping),
        "ErrorBackoff" => Some(LinkPhase::ErrorBackoff),
        _ => None,
    }
}
