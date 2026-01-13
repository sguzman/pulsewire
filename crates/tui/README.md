# Pulsewire TUI (feedrv3-tui)

Interactive terminal UI for the Pulsewire server API.

## Features
- Login with username/password.
- Browse feeds, entries, favorites, folders, and subscriptions.
- Toggle entry read/unread and feed subscriptions.
- Favorite/unfavorite feeds.
- Folder create/rename/delete, feed assignment, and folder feed list.
- Folder entries, global entries, and search.
- Configurable keybindings and server settings.

## Usage
```
FEEDRV3_TUI_CONFIG=crates/tui/res/config.toml cargo run -p feedrv3-tui
```

Controls (defaults, configurable in `config.toml`):
- Login: type username/password, Tab to switch field, Enter to login.
- Tabs: `1` Feeds, `2` Entries, `3` Favorites, `4` Folders, `5` Subscriptions.
- Navigation: Up/Down or `j`/`k`.
- Top/middle/bottom: `g`/`M`/`G`.
- Open entries for selected item: `e` (feeds/favorites/subscriptions or folder entries in Folders).
- Open all entries: `E`.
- Search entries: `/` (opens search prompt).
- Toggle read filter (all/read/unread): `w`.
- Toggle read/unread on selected entry: `m`.
- Toggle subscribe on selected feed: `s`.
- Toggle favorite on selected feed: `f`.
- Folder actions: `C` create, `R` rename, `X` delete (Folders tab).
- Folder assign/unassign: `A` / `U` (feeds/favorites/subscriptions).
- Filters: `c`/`t` open category/tag menus, `x` clears filters.
- Sort: `o` opens the sort menu.
- Entries paging: `n`/`p`.
- Feeds paging: `[`/`]`.
- List paging (favorites, folders, subscriptions): `n`/`p`.
- Refresh: `r`.
- Quit: `q`.

## Config
Default config: `crates/tui/res/config.toml`.
Schema: `crates/tui/res/schemas/tui.schema.json`.

Sections:
- `[server]` – API base URL and timeout.
- `[auth]` – auto-login flag + default credentials.
- `[ui]` – per-tab page sizes and refresh interval.
- `[keybindings]` – action bindings (supports `left`, `right`, `up`, `down`, `tab`, `enter`, `backspace`, `esc`, single chars, and `ctrl+<key>`).
