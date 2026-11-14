//! Link state machine for a single
//! feed: decides next actions, applies
//! HEAD/GET results, and computes
//! exponential backoff with jitter.

use crate::domain::model::{
  ErrorKind,
  GetResult,
  HeadResult
};

#[derive(
  Debug, Clone, Copy, PartialEq, Eq,
)]

pub enum LinkPhase {
  NeedsInitialGet,
  NeedsHead,
  NeedsGet,
  Sleeping,
  ErrorBackoff
}

#[derive(Debug, Clone)]

pub struct LinkState {
  pub feed_id: String,
  pub phase:   LinkPhase,

  pub last_head_at_ms:  Option<i64>,
  pub last_head_status: Option<u16>,
  pub last_head_error:
    Option<ErrorKind>,

  pub last_get_at_ms:  Option<i64>,
  pub last_get_status: Option<u16>,
  pub last_get_error: Option<ErrorKind>,

  pub etag:             Option<String>,
  pub last_modified_ms: Option<i64>,

  pub backoff_index:     u32,
  pub base_poll_seconds: u64,
  pub max_poll_seconds:  u64,
  pub jitter_fraction:   f64,

  pub next_action_at_ms: i64,
  pub jitter_seconds:    i64,

  pub note: Option<String>,
  pub consecutive_error_count: u32
}

#[derive(Debug, Clone)]

pub enum NextAction {
  DoHead { state: LinkState },
  DoGet { state: LinkState },
  SleepUntil { at_ms: i64 }
}

impl LinkState {
  pub fn initial(
    feed_id: String,
    base_poll_seconds: u64,
    max_poll_seconds: u64,
    jitter_fraction: f64,
    now_ms: i64
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
      consecutive_error_count: 0
    }
  }

  pub fn decide_next_action(
    state: &LinkState,
    now_ms: i64
  ) -> NextAction {
    if now_ms < state.next_action_at_ms
    {
      return NextAction::SleepUntil {
        at_ms: state.next_action_at_ms
      };
    }

    match state.phase {
      | LinkPhase::NeedsInitialGet
      | LinkPhase::NeedsGet => {
        NextAction::DoGet {
          state: state.clone()
        }
      }
      | LinkPhase::NeedsHead => {
        NextAction::DoHead {
          state: state.clone()
        }
      }
      // Once the scheduled
      // sleep/backoff has elapsed, wake
      // up with a HEAD to re-check.
      | LinkPhase::Sleeping
      | LinkPhase::ErrorBackoff => {
        NextAction::DoHead {
          state: state.clone()
        }
      }
    }
  }

  pub fn apply_head_result(
    mut state: LinkState,
    result: HeadResult,
    now_ms: i64,
    rand01: f64
  ) -> LinkState {
    let modified =
      has_changed(&state, &result);

    let is_error =
      result.error.is_some()
        || result
          .status
          .map(is_error_status)
          .unwrap_or(false);

    let (
      backoff_idx,
      phase,
      note,
      consecutive_error_count
    ) = if is_error {
      (
        state
          .backoff_index
          .saturating_add(1),
        LinkPhase::ErrorBackoff,
        Some(format!(
          "head-error-{:?}",
          result.error
        )),
        state
          .consecutive_error_count
          .saturating_add(1)
      )
    } else if modified {
      (
        state.backoff_index,
        LinkPhase::NeedsGet,
        Some(
          "head-modified".to_string()
        ),
        0
      )
    } else {
      (
        state
          .backoff_index
          .saturating_add(1),
        LinkPhase::Sleeping,
        Some(
          "head-not-modified"
            .to_string()
        ),
        0
      )
    };

    let delay = compute_delay_seconds(
      state.base_poll_seconds,
      backoff_idx,
      state.max_poll_seconds,
      state.jitter_fraction,
      rand01
    );

    state.phase = phase;

    state.last_head_at_ms =
      Some(now_ms);

    state.last_head_status =
      result.status;

    state.last_head_error =
      result.error;

    state.backoff_index = backoff_idx;

    if result.etag.is_some()
      || state.etag.is_none()
    {
      state.etag = result.etag.clone();
    }

    if result.last_modified.is_some()
      || state
        .last_modified_ms
        .is_none()
    {
      state.last_modified_ms =
        result.last_modified;
    }

    state.next_action_at_ms = now_ms
      + (delay.total_seconds as i64)
        * 1000;

    state.jitter_seconds =
      delay.jitter_seconds;

    state.note = note;

    state.consecutive_error_count =
      consecutive_error_count;

    state
  }

  pub fn apply_get_result(
    mut state: LinkState,
    result: GetResult,
    now_ms: i64,
    body_changed: bool,
    rand01: f64
  ) -> LinkState {
    let is_error =
      result.error.is_some()
        || result
          .status
          .map(is_error_status)
          .unwrap_or(false);

    let (
      backoff_idx,
      phase,
      note,
      consecutive_error_count
    ) = if is_error {
      (
        state
          .backoff_index
          .saturating_add(1),
        LinkPhase::ErrorBackoff,
        Some(format!(
          "get-error-{:?}",
          result.error
        )),
        state
          .consecutive_error_count
          .saturating_add(1)
      )
    } else if body_changed {
      (
        0,
        LinkPhase::Sleeping,
        Some(
          "get-body-changed"
            .to_string()
        ),
        0
      )
    } else {
      (
        state
          .backoff_index
          .saturating_add(1),
        LinkPhase::Sleeping,
        Some(
          "get-unchanged".to_string()
        ),
        0
      )
    };

    let delay = compute_delay_seconds(
      state.base_poll_seconds,
      backoff_idx,
      state.max_poll_seconds,
      state.jitter_fraction,
      rand01
    );

    state.phase =
      if phase == LinkPhase::Sleeping {
        LinkPhase::NeedsHead
      } else {
        phase
      };

    state.last_get_at_ms = Some(now_ms);

    state.last_get_status =
      result.status;

    state.last_get_error = result.error;

    if result.etag.is_some() {
      state.etag = result.etag;
    }

    if result.last_modified.is_some() {
      state.last_modified_ms =
        result.last_modified;
    }

    state.backoff_index = backoff_idx;

    state.next_action_at_ms = now_ms
      + (delay.total_seconds as i64)
        * 1000;

    state.jitter_seconds =
      delay.jitter_seconds;

    state.note = note;

    state.consecutive_error_count =
      consecutive_error_count;

    state
  }
}

fn has_changed(
  state: &LinkState,
  result: &HeadResult
) -> bool {
  let by_status =
    matches!(result.status, Some(200))
      && matches!(
        state.last_head_status,
        Some(304)
      );

  let by_etag =
    match (&state.etag, &result.etag) {
      | (Some(a), Some(b)) => a != b,
      | _ => false
    };

  let by_mod = match (
    state.last_modified_ms,
    result.last_modified
  ) {
    | (Some(a), Some(b)) => a != b,
    | _ => false
  };

  by_status || by_etag || by_mod
}

#[derive(Debug, Clone, Copy)]

pub struct Delay {
  pub total_seconds:  u64,
  pub jitter_seconds: i64
}

pub fn compute_delay_seconds(
  base: u64,
  backoff: u32,
  max_seconds: u64,
  jitter_fraction: f64,
  rand01: f64
) -> Delay {
  let base_seconds = base
    .saturating_mul(
      2u64.saturating_pow(backoff)
    );

  let clamped =
    base_seconds.min(max_seconds);

  let jitter_raw =
    (clamped as f64) * jitter_fraction;

  let centered =
    (rand01 * 2.0 - 1.0) * jitter_raw;

  let jitter_seconds =
    centered.round() as i64;

  let total = (clamped as i64
    + jitter_seconds)
    .max(0) as u64;

  Delay {
    total_seconds: total,
    jitter_seconds
  }
}

fn is_error_status(code: u16) -> bool {
  (400..=599).contains(&code)
}

#[cfg(test)]
mod scheduler_retry_tests {
  use super::*;

  fn timeout_get() -> GetResult {
    GetResult {
      status:             None,
      body:               None,
      etag:               None,
      last_modified:      None,
      error:              Some(
        ErrorKind::Timeout
      ),
      latency_ms:         25,
      set_cookie_headers: Vec::new()
    }
  }

  fn timeout_head() -> HeadResult {
    HeadResult {
      status:             None,
      etag:               None,
      last_modified:      None,
      error:              Some(
        ErrorKind::Timeout
      ),
      latency_ms:         25,
      set_cookie_headers: Vec::new()
    }
  }

  #[test]
  fn error_backoff_defers_retries_and_caps_delay()
   {
    let state = LinkState::initial(
      "failing-feed".into(),
      2,
      16,
      0.0,
      1_000
    );
    let first =
      LinkState::apply_get_result(
        state,
        timeout_get(),
        1_000,
        false,
        0.5
      );
    assert_eq!(
      first.phase,
      LinkPhase::ErrorBackoff
    );
    assert_eq!(
      first.consecutive_error_count,
      1
    );
    assert_eq!(first.backoff_index, 1);
    assert_eq!(
      first.next_action_at_ms,
      5_000
    );
    assert!(matches!(
      LinkState::decide_next_action(
        &first, 4_999
      ),
      NextAction::SleepUntil {
        at_ms: 5_000
      }
    ));
    assert!(matches!(
      LinkState::decide_next_action(
        &first, 5_000
      ),
      NextAction::DoHead { .. }
    ));

    let second =
      LinkState::apply_head_result(
        first,
        timeout_head(),
        5_000,
        0.5
      );
    assert_eq!(
      second.consecutive_error_count,
      2
    );
    assert_eq!(second.backoff_index, 2);
    assert_eq!(
      second.next_action_at_ms,
      13_000
    );

    let third =
      LinkState::apply_head_result(
        second,
        timeout_head(),
        13_000,
        0.5
      );
    assert_eq!(
      third.consecutive_error_count,
      3
    );
    assert_eq!(
      third.next_action_at_ms,
      29_000
    );

    let fourth =
      LinkState::apply_head_result(
        third,
        timeout_head(),
        29_000,
        0.5
      );
    assert_eq!(
      fourth.consecutive_error_count,
      4
    );
    assert_eq!(
      fourth.next_action_at_ms,
      45_000
    );
    assert_eq!(fourth.backoff_index, 4);
  }

  #[test]
  fn recovered_head_clears_error_streak_and_returns_to_get()
   {
    let mut state = LinkState::initial(
      "recovered-feed".into(),
      2,
      16,
      0.0,
      1_000
    );
    state = LinkState::apply_get_result(
      state,
      timeout_get(),
      1_000,
      false,
      0.5
    );
    state.etag =
      Some("old-version".into());

    let due_at =
      state.next_action_at_ms;
    let recovered =
      LinkState::apply_head_result(
        state,
        HeadResult {
          status:             Some(200),
          etag:               Some(
            "new-version".into()
          ),
          last_modified:      None,
          error:              None,
          latency_ms:         4,
          set_cookie_headers: Vec::new(
          )
        },
        due_at,
        0.5
      );

    assert_eq!(
      recovered.phase,
      LinkPhase::NeedsGet
    );
    assert_eq!(
      recovered.consecutive_error_count,
      0
    );
    assert!(
      recovered
        .last_head_error
        .is_none()
    );
    assert_eq!(
      recovered.etag.as_deref(),
      Some("new-version")
    );
    assert!(matches!(
      LinkState::decide_next_action(
        &recovered, due_at
      ),
      NextAction::SleepUntil { .. }
    ));
    assert!(matches!(
      LinkState::decide_next_action(
        &recovered,
        recovered.next_action_at_ms
      ),
      NextAction::DoGet { .. }
    ));
  }
}
