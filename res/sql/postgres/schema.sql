CREATE TABLE IF NOT EXISTS categories(
  name TEXT PRIMARY KEY,
  created_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS feeds(
  id TEXT PRIMARY KEY,
  url TEXT NOT NULL,
  domain TEXT NOT NULL,
  category TEXT NOT NULL REFERENCES categories(name),
  base_poll_seconds BIGINT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL
);

ALTER TABLE feeds ADD COLUMN IF NOT EXISTS category TEXT;

CREATE TABLE IF NOT EXISTS feed_state_history(
  id BIGSERIAL PRIMARY KEY,
  feed_id TEXT NOT NULL REFERENCES feeds(id),
  recorded_at TIMESTAMPTZ NOT NULL,
  phase TEXT NOT NULL,
  last_head_at TIMESTAMPTZ NULL,
  last_head_status BIGINT NULL,
  last_head_error TEXT NULL,
  last_get_at TIMESTAMPTZ NULL,
  last_get_status BIGINT NULL,
  last_get_error TEXT NULL,
  etag TEXT NULL,
  last_modified_at TIMESTAMPTZ NULL,
  backoff_index BIGINT NOT NULL,
  base_poll_seconds BIGINT NOT NULL,
  next_action_at TIMESTAMPTZ NOT NULL,
  jitter_seconds BIGINT NOT NULL,
  note TEXT NULL,
  consecutive_error_count BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS fetch_events(
  id BIGSERIAL PRIMARY KEY,
  feed_id TEXT NOT NULL REFERENCES feeds(id),
  event_time TIMESTAMPTZ NOT NULL,
  method TEXT NOT NULL,
  status BIGINT NULL,
  error_kind TEXT NULL,
  latency_ms BIGINT NULL,
  backoff_index BIGINT NOT NULL,
  scheduled_next_action_at TIMESTAMPTZ NOT NULL,
  debug TEXT NULL
);

CREATE TABLE IF NOT EXISTS feed_payloads(
  id BIGSERIAL PRIMARY KEY,
  feed_id TEXT NOT NULL REFERENCES feeds(id),
  fetched_at TIMESTAMPTZ NOT NULL,
  etag TEXT NULL,
  last_modified_at TIMESTAMPTZ NULL,
  content_hash TEXT NULL,
  title TEXT NULL,
  link TEXT NULL,
  description TEXT NULL,
  language TEXT NULL,
  updated_at TIMESTAMPTZ NULL
);

CREATE TABLE IF NOT EXISTS feed_state_current(
  feed_id TEXT PRIMARY KEY REFERENCES feeds(id),
  phase TEXT NOT NULL,
  last_head_at TIMESTAMPTZ NULL,
  last_head_status BIGINT NULL,
  last_head_error TEXT NULL,
  last_get_at TIMESTAMPTZ NULL,
  last_get_status BIGINT NULL,
  last_get_error TEXT NULL,
  etag TEXT NULL,
  last_modified_at TIMESTAMPTZ NULL,
  backoff_index BIGINT NOT NULL,
  base_poll_seconds BIGINT NOT NULL,
  next_action_at TIMESTAMPTZ NOT NULL,
  jitter_seconds BIGINT NOT NULL,
  note TEXT NULL,
  consecutive_error_count BIGINT NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_feed_state_current_next_action
ON feed_state_current(next_action_at);

CREATE TABLE IF NOT EXISTS feed_items(
  id BIGSERIAL PRIMARY KEY,
  payload_id BIGINT NOT NULL REFERENCES feed_payloads(id) ON DELETE CASCADE,
  feed_id TEXT NOT NULL REFERENCES feeds(id),
  title TEXT NULL,
  link TEXT NULL,
  guid TEXT NULL,
  published_at TIMESTAMPTZ NULL,
  category TEXT NULL,
  description TEXT NULL,
  summary TEXT NULL
);

CREATE INDEX IF NOT EXISTS idx_feed_items_payload ON feed_items(payload_id);
CREATE INDEX IF NOT EXISTS idx_feed_items_feed ON feed_items(feed_id);
CREATE INDEX IF NOT EXISTS idx_feeds_domain ON feeds(domain);
CREATE INDEX IF NOT EXISTS idx_feeds_category ON feeds(category);

CREATE TABLE IF NOT EXISTS error_feeds(
  feed_id TEXT PRIMARY KEY REFERENCES feeds(id),
  error_count BIGINT NOT NULL,
  last_error_kind TEXT NULL,
  last_error_status BIGINT NULL,
  last_error_at TIMESTAMPTZ NOT NULL,
  note TEXT NULL
);

ALTER TABLE feed_state_current ADD COLUMN IF NOT EXISTS consecutive_error_count BIGINT NOT NULL DEFAULT 0;
ALTER TABLE feed_state_history ADD COLUMN IF NOT EXISTS consecutive_error_count BIGINT NOT NULL DEFAULT 0;
