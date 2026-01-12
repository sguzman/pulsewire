# feedrv3-tui

Interactive terminal UI for the feedrv3 server API.

## Features
- Login with username/password.
- Browse feeds, entries, favorites, folders, and subscriptions.
- Toggle entry read/unread.
- Subscribe/unsubscribe feeds.
- Refresh data on demand.
- Configurable keybindings and server settings.

## Usage
```
FEEDRV3_TUI_CONFIG=crates/tui/res/config.toml cargo run -p feedrv3-tui
```

Controls (defaults, configurable in `config.toml`):
- Login: type username/password, Tab to switch field, Enter to login.
- Tabs: `1` Feeds, `2` Entries, `3` Favorites, `4` Folders, `5` Subscriptions.
- Navigation: Up/Down or `j`/`k`.
- Open entries for selected feed: `e`.
- Toggle read/unread on selected entry: `m`.
- Toggle subscribe on selected feed: `s`.
- Filters: `c`/`C` cycle category, `t`/`T` cycle tag, `x` clears filters.
- Entries paging: `n`/`p`.
- List paging (feeds, favorites, folders, subscriptions): `n`/`p`.
- Refresh: `r`.
- Quit: `q`.

## Config
Default config: `crates/tui/res/config.toml`.
Schema: `crates/tui/res/schemas/tui.schema.json`.

Sections:
- `[server]` – API base URL and timeout.
- `[auth]` – auto-login flag + default credentials.
- `[ui]` – page size and refresh interval.
- `[keybindings]` – action bindings (supports `left`, `right`, `up`, `down`, `tab`, `enter`, `backspace`, `esc`, single chars, and `ctrl+<key>`).
