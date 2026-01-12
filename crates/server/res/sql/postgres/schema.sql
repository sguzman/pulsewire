CREATE TABLE IF NOT EXISTS users(
  id BIGSERIAL PRIMARY KEY,
  username TEXT NOT NULL UNIQUE,
  password_hash TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS user_tokens(
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  token_hash TEXT NOT NULL,
  expires_at TIMESTAMPTZ NOT NULL,
  created_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS user_password_resets(
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  token_hash TEXT NOT NULL,
  expires_at TIMESTAMPTZ NOT NULL,
  created_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS subscriptions(
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  feed_id TEXT NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
  created_at TIMESTAMPTZ NOT NULL,
  PRIMARY KEY (user_id, feed_id)
);

CREATE TABLE IF NOT EXISTS entry_states(
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  item_id BIGINT NOT NULL REFERENCES feed_items(id) ON DELETE CASCADE,
  read_at TIMESTAMPTZ NULL,
  PRIMARY KEY (user_id, item_id)
);


CREATE TABLE IF NOT EXISTS favorites(
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  feed_id TEXT NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
  created_at TIMESTAMPTZ NOT NULL,
  PRIMARY KEY (user_id, feed_id)
);

CREATE TABLE IF NOT EXISTS folders(
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL,
  UNIQUE (user_id, name)
);

CREATE TABLE IF NOT EXISTS folder_feeds(
  folder_id BIGINT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
  feed_id TEXT NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
  created_at TIMESTAMPTZ NOT NULL,
  PRIMARY KEY (folder_id, feed_id)
);

CREATE INDEX IF NOT EXISTS idx_subscriptions_user ON subscriptions(user_id);
CREATE INDEX IF NOT EXISTS idx_entry_states_user ON entry_states(user_id);
CREATE INDEX IF NOT EXISTS idx_favorites_user ON favorites(user_id);
