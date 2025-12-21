CREATE TABLE IF NOT EXISTS categories(
  name TEXT PRIMARY KEY,
  created_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS feeds(
  id TEXT PRIMARY KEY,
  url TEXT NOT NULL,
  domain TEXT NOT NULL,
  category TEXT NOT NULL REFERENCES categories(name),
  base_poll_seconds INTEGER NOT NULL,
  created_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS feed_state_history(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  feed_id TEXT NOT NULL REFERENCES feeds(id),
  recorded_at_ms INTEGER NOT NULL,
  phase TEXT NOT NULL,
  last_head_at_ms INTEGER NULL,
  last_head_status INTEGER NULL,
  last_head_error TEXT NULL,
  last_get_at_ms INTEGER NULL,
  last_get_status INTEGER NULL,
  last_get_error TEXT NULL,
  etag TEXT NULL,
  last_modified_ms INTEGER NULL,
  backoff_index INTEGER NOT NULL,
  base_poll_seconds INTEGER NOT NULL,
  next_action_at_ms INTEGER NOT NULL,
  jitter_seconds INTEGER NOT NULL,
  note TEXT NULL,
  consecutive_error_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS fetch_events(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  feed_id TEXT NOT NULL REFERENCES feeds(id),
  event_time_ms INTEGER NOT NULL,
  method TEXT NOT NULL,
  status INTEGER NULL,
  error_kind TEXT NULL,
  latency_ms INTEGER NULL,
  backoff_index INTEGER NOT NULL,
  scheduled_next_action_at_ms INTEGER NOT NULL,
  debug TEXT NULL
);

CREATE TABLE IF NOT EXISTS feed_payloads(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  feed_id TEXT NOT NULL REFERENCES feeds(id),
  fetched_at_ms INTEGER NOT NULL,
  etag TEXT NULL,
  last_modified_ms INTEGER NULL,
  content_hash TEXT NULL,
  title TEXT NULL,
  link TEXT NULL,
  description TEXT NULL,
  language TEXT NULL,
  updated_at_ms INTEGER NULL
);

CREATE TABLE IF NOT EXISTS feed_state_current(
  feed_id TEXT PRIMARY KEY REFERENCES feeds(id),
  phase TEXT NOT NULL,
  last_head_at_ms INTEGER NULL,
  last_head_status INTEGER NULL,
  last_head_error TEXT NULL,
  last_get_at_ms INTEGER NULL,
  last_get_status INTEGER NULL,
  last_get_error TEXT NULL,
  etag TEXT NULL,
  last_modified_ms INTEGER NULL,
  backoff_index INTEGER NOT NULL,
  base_poll_seconds INTEGER NOT NULL,
  next_action_at_ms INTEGER NOT NULL,
  jitter_seconds INTEGER NOT NULL,
  note TEXT NULL,
  consecutive_error_count INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_feed_state_current_next_action
ON feed_state_current(next_action_at_ms);

CREATE TABLE IF NOT EXISTS feed_items(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  payload_id INTEGER NOT NULL REFERENCES feed_payloads(id) ON DELETE CASCADE,
  feed_id TEXT NOT NULL REFERENCES feeds(id),
  title TEXT NULL,
  link TEXT NULL,
  guid TEXT NULL,
  published_at_ms INTEGER NULL,
  category TEXT NULL,
  description TEXT NULL,
  summary TEXT NULL
);

CREATE TABLE IF NOT EXISTS error_feeds(
  feed_id TEXT PRIMARY KEY REFERENCES feeds(id),
  error_count INTEGER NOT NULL,
  last_error_kind TEXT NULL,
  last_error_status INTEGER NULL,
  last_error_at_ms INTEGER NOT NULL,
  note TEXT NULL
);

CREATE INDEX IF NOT EXISTS idx_feed_items_payload ON feed_items(payload_id);
CREATE INDEX IF NOT EXISTS idx_feed_items_feed ON feed_items(feed_id);
CREATE INDEX IF NOT EXISTS idx_feeds_domain ON feeds(domain);
CREATE INDEX IF NOT EXISTS idx_feeds_category ON feeds(category);
