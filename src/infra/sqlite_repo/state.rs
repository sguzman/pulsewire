//! Persist/read current and historical link state snapshots.
use chrono_tz::Tz;
use sqlx::SqlitePool;

use crate::domain::link_state::LinkState;
use crate::ports::repo::StateRow;

use super::models::StateRowRecord;

pub async fn latest_state(pool: &SqlitePool, feed_id: &str) -> Result<Option<StateRow>, String> {
    let row = sqlx::query_as::<_, StateRowRecord>(
        r#"
      SELECT
        feed_id,
        phase,
        last_head_at_ms,
        last_head_status,
        last_head_error,
        last_get_at_ms,
        last_get_status,
        last_get_error,
        etag,
        last_modified_ms,
        backoff_index,
        base_poll_seconds,
        next_action_at_ms,
        jitter_seconds,
        note,
        consecutive_error_count
      FROM feed_state_current
      WHERE feed_id = ?1
      "#,
    )
    .bind(feed_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("latest_state error: {e}"))?;
    Ok(row.map(StateRow::from))
}

pub async fn insert_state(
    pool: &SqlitePool,
    state: &LinkState,
    recorded_at_ms: i64,
    _zone: &Tz,
    record_history: bool,
) -> Result<(), String> {
    if record_history {
        sqlx::query(
            r#"
        INSERT INTO feed_state_history(
          feed_id, recorded_at_ms, phase,
          last_head_at_ms, last_head_status, last_head_error,
          last_get_at_ms, last_get_status, last_get_error,
          etag, last_modified_ms,
          backoff_index, base_poll_seconds, next_action_at_ms,
          jitter_seconds, note, consecutive_error_count
        ) VALUES (
          ?1, ?2, ?3,
          ?4, ?5, ?6,
          ?7, ?8, ?9,
          ?10, ?11,
          ?12, ?13, ?14,
          ?15, ?16, ?17
        )
        "#,
        )
        .bind(&state.feed_id)
        .bind(recorded_at_ms)
        .bind(format!("{:?}", state.phase))
        .bind(state.last_head_at_ms)
        .bind(state.last_head_status.map(|x| x as i64))
        .bind(state.last_head_error.map(|e| format!("{:?}", e)))
        .bind(state.last_get_at_ms)
        .bind(state.last_get_status.map(|x| x as i64))
        .bind(state.last_get_error.map(|e| format!("{:?}", e)))
        .bind(&state.etag)
        .bind(state.last_modified_ms)
        .bind(state.backoff_index as i64)
        .bind(state.base_poll_seconds as i64)
        .bind(state.next_action_at_ms)
        .bind(state.jitter_seconds)
        .bind(&state.note)
        .bind(state.consecutive_error_count as i64)
        .execute(pool)
        .await
        .map_err(|e| format!("insert_state history error: {e}"))?;
    }

    sqlx::query(
        r#"
      INSERT INTO feed_state_current(
        feed_id, phase,
        last_head_at_ms, last_head_status, last_head_error,
        last_get_at_ms, last_get_status, last_get_error,
        etag, last_modified_ms,
        backoff_index, base_poll_seconds, next_action_at_ms,
        jitter_seconds, note, consecutive_error_count
      ) VALUES (
        ?1, ?2,
        ?3, ?4, ?5,
        ?6, ?7, ?8,
        ?9, ?10,
        ?11, ?12, ?13,
        ?14, ?15, ?16
      )
      ON CONFLICT(feed_id) DO UPDATE SET
        phase = excluded.phase,
        last_head_at_ms = excluded.last_head_at_ms,
        last_head_status = excluded.last_head_status,
        last_head_error = excluded.last_head_error,
        last_get_at_ms = excluded.last_get_at_ms,
        last_get_status = excluded.last_get_status,
        last_get_error = excluded.last_get_error,
        etag = excluded.etag,
        last_modified_ms = excluded.last_modified_ms,
        backoff_index = excluded.backoff_index,
        base_poll_seconds = excluded.base_poll_seconds,
        next_action_at_ms = excluded.next_action_at_ms,
        jitter_seconds = excluded.jitter_seconds,
        note = excluded.note,
        consecutive_error_count = excluded.consecutive_error_count
      "#,
    )
    .bind(&state.feed_id)
    .bind(format!("{:?}", state.phase))
    .bind(state.last_head_at_ms)
    .bind(state.last_head_status.map(|x| x as i64))
    .bind(state.last_head_error.map(|e| format!("{:?}", e)))
    .bind(state.last_get_at_ms)
    .bind(state.last_get_status.map(|x| x as i64))
    .bind(state.last_get_error.map(|e| format!("{:?}", e)))
    .bind(&state.etag)
    .bind(state.last_modified_ms)
    .bind(state.backoff_index as i64)
    .bind(state.base_poll_seconds as i64)
    .bind(state.next_action_at_ms)
    .bind(state.jitter_seconds)
    .bind(&state.note)
    .bind(state.consecutive_error_count as i64)
    .execute(pool)
    .await
    .map_err(|e| format!("insert_state current error: {e}"))?;

    Ok(())
}
