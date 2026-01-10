//! Link state machine for a single feed: decides next actions, applies HEAD/GET results,
//! and computes exponential backoff with jitter.
use crate::domain::model::{ErrorKind, GetResult, HeadResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkPhase {
    NeedsInitialGet,
    NeedsHead,
    NeedsGet,
    Sleeping,
    ErrorBackoff,
}

#[derive(Debug, Clone)]
pub struct LinkState {
    pub feed_id: String,
    pub phase: LinkPhase,

    pub last_head_at_ms: Option<i64>,
    pub last_head_status: Option<u16>,
    pub last_head_error: Option<ErrorKind>,

    pub last_get_at_ms: Option<i64>,
    pub last_get_status: Option<u16>,
    pub last_get_error: Option<ErrorKind>,

    pub etag: Option<String>,
    pub last_modified_ms: Option<i64>,

    pub backoff_index: u32,
    pub base_poll_seconds: u64,
    pub max_poll_seconds: u64,
    pub jitter_fraction: f64,

    pub next_action_at_ms: i64,
    pub jitter_seconds: i64,

    pub note: Option<String>,
    pub consecutive_error_count: u32,
}

#[derive(Debug, Clone)]
pub enum NextAction {
    DoHead { state: LinkState },
    DoGet { state: LinkState },
    SleepUntil { at_ms: i64 },
}

impl LinkState {
    pub fn initial(
        feed_id: String,
        base_poll_seconds: u64,
        max_poll_seconds: u64,
        jitter_fraction: f64,
        now_ms: i64,
    ) -> Self {
        Self {
            feed_id,
            phase: LinkPhase::NeedsInitialGet,
            last_head_at_ms: None,
            last_head_status: None,
            last_head_error: None,
            last_get_at_ms: None,
            last_get_status: None,
            last_get_error: None,
            etag: None,
            last_modified_ms: None,
            backoff_index: 0,
            base_poll_seconds,
            max_poll_seconds,
            jitter_fraction,
            next_action_at_ms: now_ms,
            jitter_seconds: 0,
            note: Some("initial".to_string()),
            consecutive_error_count: 0,
        }
    }

    pub fn decide_next_action(state: &LinkState, now_ms: i64) -> NextAction {
        if now_ms < state.next_action_at_ms {
            return NextAction::SleepUntil {
                at_ms: state.next_action_at_ms,
            };
        }
        match state.phase {
            LinkPhase::NeedsInitialGet | LinkPhase::NeedsGet => NextAction::DoGet {
                state: state.clone(),
            },
            LinkPhase::NeedsHead => NextAction::DoHead {
                state: state.clone(),
            },
            // Once the scheduled sleep/backoff has elapsed, wake up with a HEAD to re-check.
            LinkPhase::Sleeping | LinkPhase::ErrorBackoff => NextAction::DoHead {
                state: state.clone(),
            },
        }
    }

    pub fn apply_head_result(
        mut state: LinkState,
        result: HeadResult,
        now_ms: i64,
        rand01: f64,
    ) -> LinkState {
        let modified = has_changed(&state, &result);
        let is_error =
            result.error.is_some() || result.status.map(is_error_status).unwrap_or(false);

        let (backoff_idx, phase, note, consecutive_error_count) = if is_error {
            (
                state.backoff_index.saturating_add(1),
                LinkPhase::ErrorBackoff,
                Some(format!("head-error-{:?}", result.error)),
                state.consecutive_error_count.saturating_add(1),
            )
        } else if modified {
            (
                state.backoff_index.max(0),
                LinkPhase::NeedsGet,
                Some("head-modified".to_string()),
                0,
            )
        } else {
            (
                state.backoff_index.saturating_add(1),
                LinkPhase::Sleeping,
                Some("head-not-modified".to_string()),
                0,
            )
        };

        let delay = compute_delay_seconds(
            state.base_poll_seconds,
            backoff_idx,
            state.max_poll_seconds,
            state.jitter_fraction,
            rand01,
        );

        state.phase = phase;
        state.last_head_at_ms = Some(now_ms);
        state.last_head_status = result.status;
        state.last_head_error = result.error;
        state.backoff_index = backoff_idx;
        if state.etag.is_none() {
            state.etag = result.etag.clone();
        } else if result.etag.is_some() {
            state.etag = result.etag.clone();
        }
        if state.last_modified_ms.is_none() {
            state.last_modified_ms = result.last_modified;
        } else if result.last_modified.is_some() {
            state.last_modified_ms = result.last_modified;
        }
        state.next_action_at_ms = now_ms + (delay.total_seconds as i64) * 1000;
        state.jitter_seconds = delay.jitter_seconds;
        state.note = note;
        state.consecutive_error_count = consecutive_error_count;
        state
    }

    pub fn apply_get_result(
        mut state: LinkState,
        result: GetResult,
        now_ms: i64,
        body_changed: bool,
        rand01: f64,
    ) -> LinkState {
        let is_error =
            result.error.is_some() || result.status.map(is_error_status).unwrap_or(false);

        let (backoff_idx, phase, note, consecutive_error_count) = if is_error {
            (
                state.backoff_index.saturating_add(1),
                LinkPhase::ErrorBackoff,
                Some(format!("get-error-{:?}", result.error)),
                state.consecutive_error_count.saturating_add(1),
            )
        } else if body_changed {
            (
                0,
                LinkPhase::Sleeping,
                Some("get-body-changed".to_string()),
                0,
            )
        } else {
            (
                state.backoff_index.saturating_add(1),
                LinkPhase::Sleeping,
                Some("get-unchanged".to_string()),
                0,
            )
        };

        let delay = compute_delay_seconds(
            state.base_poll_seconds,
            backoff_idx,
            state.max_poll_seconds,
            state.jitter_fraction,
            rand01,
        );

        state.phase = if phase == LinkPhase::Sleeping {
            LinkPhase::NeedsHead
        } else {
            phase
        };
        state.last_get_at_ms = Some(now_ms);
        state.last_get_status = result.status;
        state.last_get_error = result.error;
        if result.etag.is_some() {
            state.etag = result.etag;
        }
        if result.last_modified.is_some() {
            state.last_modified_ms = result.last_modified;
        }
        state.backoff_index = backoff_idx;
        state.next_action_at_ms = now_ms + (delay.total_seconds as i64) * 1000;
        state.jitter_seconds = delay.jitter_seconds;
        state.note = note;
        state.consecutive_error_count = consecutive_error_count;
        state
    }
}

fn has_changed(state: &LinkState, result: &HeadResult) -> bool {
    let by_status =
        matches!(result.status, Some(200)) && matches!(state.last_head_status, Some(304));
    let by_etag = match (&state.etag, &result.etag) {
        (Some(a), Some(b)) => a != b,
        _ => false,
    };
    let by_mod = match (state.last_modified_ms, result.last_modified) {
        (Some(a), Some(b)) => a != b,
        _ => false,
    };
    by_status || by_etag || by_mod
}

#[derive(Debug, Clone, Copy)]
pub struct Delay {
    pub total_seconds: u64,
    pub jitter_seconds: i64,
}

pub fn compute_delay_seconds(
    base: u64,
    backoff: u32,
    max_seconds: u64,
    jitter_fraction: f64,
    rand01: f64,
) -> Delay {
    let base_seconds = base.saturating_mul(2u64.saturating_pow(backoff));
    let clamped = base_seconds.min(max_seconds);

    let jitter_raw = (clamped as f64) * jitter_fraction;
    let centered = (rand01 * 2.0 - 1.0) * jitter_raw;
    let jitter_seconds = centered.round() as i64;

    let total = (clamped as i64 + jitter_seconds).max(0) as u64;

    Delay {
        total_seconds: total,
        jitter_seconds,
    }
}

fn is_error_status(code: u16) -> bool {
    (400..=599).contains(&code)
}
