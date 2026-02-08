Perfect, that fits cleanly into the model.

Use a two-level watch config in the same site TOML:

## Recommended Structure

- `watch_defaults`: shared extraction/check settings for the whole site file
- `[[watches]]`: one watch per section/page (markets, etfs, funds, etc.)
- per-watch overrides: any field in `watch_defaults` can be overridden inside a `[[watches]]` entry

## Example Shape

- Shared defaults:
  - `item_selector`, `title_selector`, `link_selector`
  - `item_identity`, `detectors`, `check_method`
  - `exclude_selectors`, normalization settings
- Per section override:
  - `id`, `url`, optional `category`/`tags`
  - override selectors if that section DOM differs
  - override detectors or polling if needed

## Extra thing to add now (important)

Add reusable named extraction profiles in the same file:

- `[[watch_profiles]]` with `name = "morningstar-article-list"` and common selectors
- each `[[watches]]` can set `profile = "morningstar-article-list"`
- watch-specific fields still override profile/default values

Precedence should be:

1. `global.toml` watch defaults  
2. file `watch_defaults`  
3. selected `watch_profile`  
4. individual `[[watches]]` override

That gives you DRY configs plus flexibility when one section diverges.

SemVer commit message for this design update:
`docs(ad-hoc): add hierarchical watch defaults and per-section overrides`