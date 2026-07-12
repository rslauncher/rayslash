# Config

`rayslash` reads config from:

```sh
~/.config/rayslash/config.toml
```

If the file does not exist, `rayslash` uses defaults. It does not create common project folders automatically.

## Format

```toml
folder_sources = [
  "~",
  "~/Documents",
]

[[aliases]]
name = "GitHub"
query = "gh"
target = "https://github.com"
kind = "url"

[[web_searches]]
name = "YouTube"
keyword = "yt"
url = "https://www.youtube.com/results?search_query=%s"
enabled = true

[providers]
apps = true
folders = true
calculator = true
aliases = true
web_search = true
unit_conversion = true
currency_conversion = true
time_lookup = true
utility_actions = true

[actions]
alternate_folder_opener_enabled = true
alternate_folder_opener_command = "xdg-terminal-exec"

[appearance]
theme = "dark"
density = "comfortable"
max_results = 36
show_tooltips = true

[ranking]
learn_from_usage = true
```

## Fields

- `folder_sources`: directories whose immediate child folders should be listed as folder results.
- `aliases`: quick links searched by `name` and `query`. `target` supports URLs, files, folders, and explicit commands. `kind` may be `url`, `file`, `folder`, or `command`; when omitted, rayslash infers a kind from the target.
- `web_searches`: additional search engines searched by keyword trigger. `keyword` is the trigger prefix, `url` must contain `%s` where the percent-encoded search terms are inserted, and `enabled` controls whether that engine can be triggered. Older `query` and `url_template` keys still load and are normalized to `keyword` and `url`.
- `providers.apps`: whether installed desktop applications appear in search.
- `providers.folders`: whether discovered folder results appear in search.
- `providers.calculator`: whether calculator result and error rows appear in search.
- `providers.aliases`: whether configured aliases appear in search.
- `providers.web_search`: whether the permanent `Web Search` template and configured additional search engines appear in search.
- `providers.unit_conversion`: whether local unit conversion rows appear in search.
- `providers.currency_conversion`: whether currency conversion rows using Frankfurter rates appear in search.
- `providers.time_lookup`: whether `time in <place>` rows using Open-Meteo place/timezone lookup appear in search.
- `providers.utility_actions`: compatibility mirror for the bundled Timers module, covering timers, reminders, and reboot/shutdown/logout/lock actions.
- `actions.alternate_folder_opener_enabled`: whether Ctrl+Enter and Ctrl+click use the alternate folder opener for folder results.
- `actions.alternate_folder_opener_command`: command line used by the secondary folder action. The value is parsed into direct program and argument values without a shell. Most commands are invoked as `<program> <configured-args...> <folder-path>`. The default `xdg-terminal-exec` is launched with the selected folder as its working directory and no implicit folder argument.
- `appearance.theme`: launcher theme, currently `dark`, `dim`, or `light`.
- `appearance.density`: result-list density, currently `comfortable` or `compact`.
- `appearance.max_results`: maximum number of real results shown in the launcher before a separate non-selectable scroll-end tip states the active cap.
- `appearance.show_tooltips`: whether delayed detail tooltips are shown for settings toggles and result rows.
- `ranking.learn_from_usage`: whether successful app and folder launches update and use local learned ranking state.

`~` and `~/...` are supported in `folder_sources` and file/folder alias targets, and are expanded to the current user's home directory. Relative folder sources are normalized to absolute paths from the current working directory before scanning. Settings saves write normalized folder sources back to `config.toml`.

URL, file, and folder aliases are opened with `xdg-open <target>`. Command aliases are parsed into direct program and argument values and spawned without a shell. For example:

```toml
[[aliases]]
name = "Project docs"
query = "docs"
target = "~/Documents/Project/docs"
kind = "folder"

[[aliases]]
name = "Clock timer"
query = "timer"
target = "gnome-clocks --timer"
kind = "command"
```

When web search is enabled, the permanent first engine uses the explicit `search` command instead of appearing for every non-empty query:

```text
search manhattan
```

Typing `search`, then pressing Space or Tab, activates the permanent `Web Search` engine. It defaults to `https://www.google.com/search?q=%s` for every browser. Change that entry's URL to change the default engine; it can be toggled off but not removed. There is no separate Firefox/Chromium behavior.

Additional search engines use keyword triggers. The trigger must be followed by search terms, either by typing the full prefix in the search text or by pressing Space or Tab after the keyword to turn it into the active search pill:

```toml
[[web_searches]]
name = "YouTube"
keyword = "yt"
url = "https://www.youtube.com/results?search_query=%s"
enabled = true
```

Typing `yt rust slint`, or typing `yt` and pressing Space or Tab before `rust slint`, opens `https://www.youtube.com/results?search_query=rust%20slint`.

Unit conversion is local and deterministic. The first pass supports common length, mass, volume, and temperature units with explicit syntax such as:

```text
10 km to mi
10mi to km
10 miles to km
2 lb to kg
1 cup to ml
32 f to c
10 celsius to fahrenheit
```

Conversion-like text does not fall through to calculator error rows, so compact unit searches such as `10f to c` show the conversion result without an extra calculator diagnostic.

Currency conversion uses the public Frankfurter v2 pair-rate API at `https://api.frankfurter.dev` and caches fetched rates in memory for the resident process. Query text sends only the base and quote currency codes, not the amount. The syntax is explicit ISO-style three-letter codes:

```text
10 usd to eur
25 brl in usd
```

Time lookup uses Open-Meteo geocoding plus the locally installed IANA timezone database. Common country names, including `america` and `brazil`, resolve locally without geocoding; place matching ignores punctuation for forms such as `washington dc`. Country queries return one row per distinct current UTC offset, with regions sharing an offset grouped into the subtitle and tooltip. Time queries hide unrelated results. Remote fallback lookup waits for 450ms of settled input and does not block typing. The syntax is explicit:

```text
time in Argentina
time in Sao Paulo
time in New York
```

The lookup sends the typed place name to Open-Meteo Geocoding and uses the resolved coordinates with Open-Meteo Forecast `timezone=auto` to get the local timezone and UTC offset. Resolved place/timezone metadata is cached in memory for the resident process.

When the bundled Timers module is enabled, system and reminder commands are parsed explicitly:

```text
reboot
reboot in 10
shutdown in 10min
logout now
timer in 10
timer 10 feed the cat
timer feed the cat 10min
timer "feed 2 cats" 10min
remind me to feed the cat in 10 minutes
remind in 10 to feed the cat
```

System actions run immediately when no time is given; timers still default to 30 seconds. Bare numbers are seconds; `s`, `min`, and `h` units are supported. `restart` and `reset` map to reboot; `shutdown`, `shut down`, `power off`, and `turn off` map to shutdown. Partial system-action queries such as `shutdow` produce fuzzy search items alongside matching apps and can participate in learned ranking. Timers use `notify-send -i stopwatch` for the reminder notification. If a timer query contains more than one time-like value, quote the message text.

Folder discovery is intentionally shallow for now:

- Visible immediate child directories are treated as folder results.
- Hidden directories, such as `.git` or `.cache`, are ignored.
- Nested directories are not scanned yet.

## Bundled Module Config

The first module-system slice stores official-module enablement separately at:

```sh
~/.config/rayslash/modules.toml
```

The file is versioned and currently contains seven installed, official, built-in modules:

```toml
version = 1

[modules."rayslash.calculator"]
enabled = true
version = "0.1.0"
channel = "stable"

[modules."rayslash.units"]
enabled = true
version = "0.1.0"
channel = "stable"

[modules."rayslash.currency"]
enabled = true
version = "0.1.0"
channel = "stable"

[modules."rayslash.time"]
enabled = true
version = "0.1.0"
channel = "stable"

[modules."rayslash.web-search"]
enabled = true
version = "0.1.0"
channel = "stable"

[modules."rayslash.timers"]
enabled = true
version = "0.1.0"
channel = "stable"

[modules."rayslash.aliases"]
enabled = true
version = "0.1.0"
channel = "stable"
```

The stored version for each bundled descriptor follows the rayslash package version. Apps and Folders are core providers, remain controlled by `providers.apps` and `providers.folders`, and do not receive module entries.

Migration and precedence are intentional:

- On the first startup with a valid `config.toml` and no `modules.toml`, rayslash creates `modules.toml` once and seeds the seven `enabled` values from `providers.calculator`, `providers.aliases`, `providers.web_search`, `providers.unit_conversion`, `providers.currency_conversion`, `providers.time_lookup`, and `providers.utility_actions`.
- Once `modules.toml` exists, its official-module entries take precedence over those compatibility booleans at startup. A missing official entry is seeded in memory from the corresponding legacy value.
- Toggling a module writes `modules.toml` first, applies the state immediately, and mirrors all seven values back into `config.toml` for compatibility with older rayslash versions and manual tooling. The Timers mirror is `providers.utility_actions`.
- General settings saves continue carrying the hidden compatibility values, so changing folder, appearance, alias, or search-engine settings does not reset module state.

`modules.toml` uses same-directory temporary-file-and-rename atomic writes. Unlike `config.toml`, it does not need a pre-save backup to preserve unrecognized data: unknown top-level fields, unknown fields inside module entries, and unknown module entries are retained during load/save round trips. Unknown modules are not treated as installed official modules and cannot be toggled through the first-slice UI.

If `modules.toml` is unreadable, malformed, or has an unsupported top-level `version`, rayslash uses a legacy-derived in-memory module view and blocks module writes for that process. It does not replace the bad file with defaults. Fix the file and restart rayslash before changing module state. If the main `config.toml` could not be read or parsed, both normal settings writes and module writes are blocked so fallback defaults cannot overwrite user-authored configuration.

## Defaults

When no config file exists, the default folder source is:

- `~`

Current baseline provider toggles for apps, folders, calculator, aliases, web search, unit conversion, currency conversion, time lookup, and utility actions default to `true`. The seven bundled module entries therefore default to enabled when no legacy preference says otherwise. Config normalization always keeps `Web Search` as the first template with keyword `search`; its initial URL is Google. The alternate folder opener is enabled by default and defaults to `xdg-terminal-exec`. `theme` defaults to `dark`, `density` defaults to `comfortable`, `max_results` defaults to `36`, `show_tooltips` defaults to `true`, learned ranking defaults to enabled, and shortcut hints are always shown when no status message is active.

The previous `project_roots`, `providers.projects`, and `actions.project_editor_command` keys are still accepted for compatibility. Autosaving from the settings UI writes the current public keys.

## Settings UI

The first public settings surface lives inside the launcher panel behind the header Settings button. Its top-level navigation separates `General` from `Modules`.

General can edit:

- Folder sources as a semicolon-separated list, with a native folder picker for choosing a source.
- The optional alternate folder opener command line used by Ctrl+Enter or Ctrl+click.
- An installed-app picker that can fill the alternate folder opener command, limited to apps that can reasonably open folders: apps declaring `inode/directory`, file managers, terminal emulators, and IDEs.
- Core Apps and Folders provider toggles plus the alternate folder opener toggle.
- Individual alias field cards for name, keyword, kind, and target. Alias provider enablement lives on the Modules page.
- Search-engine cards with bordered fields, a title-aligned enable switch, favicon or magnifying-glass fallback, an amber warning for incomplete drafts, and small save/remove actions. The permanent first engine has no remove action. Adding another engine immediately persists a draft; each field saves when it loses focus, and the row remains inactive until name, keyword, and a URL containing `%s` are valid. This makes it safe to copy fields from another window across several launcher openings. Non-default engine favicons are cached under the XDG cache directory and used by the settings card, matching result rows, and active-pill accent. Web Search provider enablement lives on the Modules page.
- Theme and density.
- Max shown results.
- Delayed detail tooltips.
- Learned ranking on/off.
- Clear learned ranking history.

Modules has `Installed` and `Official` tabs. Both currently show the same seven bundled rows with module ID-backed metadata: name, description, `rayslash` author, package version, official/installed status, enabled state, and an enable/disable switch. A non-interactive notice makes clear that community registry search and third-party installation are deferred; there is no Community tab or remote installer in this slice.

The General settings UI autosaves changes to `~/.config/rayslash/config.toml` with same-directory temporary-file-and-rename writes, rescans folder sources when settings are persisted, and refreshes the current result list. Before replacing an existing `config.toml`, settings saves create a timestamped sibling backup such as `config.toml.backup-<pid>-<timestamp>`. If rayslash started with a config read or parse error and fell back to defaults, settings saves are blocked until the config file is fixed and rayslash is restarted. Toggles and picker choices save immediately. Single-line text fields save when Enter is pressed in the field or when focus leaves the field; multiline alias and search-engine fields save on focus loss. This keeps partial path or number edits from repeatedly rescanning or overwriting config with invalid values. The settings UI also shows diagnostics for the config location, state location, socket path, discovered folder count, discovered app count, and resolved app icon count.

Module switches save `modules.toml` atomically and refresh the current search immediately. The compatibility mirror then writes `config.toml` with its normal backup behavior. If the mirror fails, the module change remains saved and the status line reports the compatibility failure. If module writes were blocked at startup, the switch snaps back to the loaded state and the status line directs the user to fix `config.toml` or `modules.toml` and restart.

Manual TOML editing remains supported. Unknown `config.toml` fields are ignored when loading; the General settings UI writes the known public fields, including aliases and web searches, but it does not preserve unknown main-config fields when autosaving. The pre-save backup keeps the previous file contents recoverable. In contrast, the versioned module config preserves unknown fields and unknown module entries. Clearing learned ranking history removes generated state only and does not rewrite folder sources, provider/module toggles, actions, aliases, web searches, or appearance settings.

## Learned Ranking State

Learned ranking state lives under the XDG state directory:

```sh
~/.local/state/rayslash/ranking.toml
```

The file is generated by rayslash and should not be treated as user-authored config. The current format is versioned TOML:

```toml
version = 1

[entries."app:org.example.Editor.desktop"]
launch_count = 3
last_launched_unix = 1782777600
query_prefixes = { ed = 2, edi = 1 }

[entries."folder:/home/example/Documents/Project"]
launch_count = 1
last_launched_unix = 1782777700
query_prefixes = { pr = 1, pro = 1 }
```

Tracked signals are intentionally small:

- Stable result ID for current app and folder rows.
- Successful launch count.
- Last launched Unix timestamp in seconds.
- Query prefixes from successful non-empty launches, starting at two characters.

Ranking state is pruned after learned app/folder launches. Entries for apps or folders no longer present in the current index are removed, entries older than 180 days are removed, each entry keeps at most 64 query prefixes, and the state keeps at most 1000 entries by recency.

Calculator, no-results, and placeholder rows have stable IDs where useful for internal consistency, but they are not learned from in this phase. Corrupted or unsupported ranking state falls back to empty state instead of blocking launcher startup.

The ranking formula is conservative:

- Empty queries keep the base title/type/subtitle ordering.
- Valid calculator rows and calculator error rows still appear before app and folder rows for math-like queries.
- App and folder rows keep the existing fuzzy score as the base score.
- Learned boost is only considered when the row title starts with the current query after trimming/case normalization.
- Boost is bounded to at most 20 points, with at most 8 points from total launch count and at most 16 points from the matching query prefix count.
- Ties fall back to the original fuzzy score and then the existing deterministic title/type/subtitle ordering.

This means learned ranking can break close prefix-result ties, but should not promote weaker in-string matches above strong textual prefix matches.

## App Install State

New-app flair state lives under the XDG state directory:

```sh
~/.local/state/rayslash/apps.toml
```

The first run records the current desktop app IDs as the baseline. Later desktop app IDs discovered during startup or settings-open refresh are marked as new until the app is successfully launched from rayslash. This generated state is separate from config and learned ranking.

## Planned Expansion

The current public config remains intentionally small. Future customization should expand it deliberately rather than adding one-off fields as features land.

Possible future shape:

```toml
folder_sources = [
  "~",
  "~/Documents",
]

[providers]
apps = true
folders = true
calculator = true
aliases = true
web_search = true
unit_conversion = true
currency_conversion = true
time_lookup = true
utility_actions = true

[actions]
folder_default = "file_manager"
alternate_folder_opener_enabled = true
alternate_folder_opener_command = "xdg-terminal-exec"

[appearance]
theme = "dark"
density = "comfortable"
max_results = 36
show_tooltips = true

[ranking]
learn_from_usage = true

[[aliases]]
name = "GitHub"
query = "gh"
target = "https://github.com"

[[web_searches]]
name = "YouTube"
keyword = "yt"
url = "https://www.youtube.com/results?search_query=%s"
enabled = true
```

The `folder_sources`, `[[aliases]]`, `[[web_searches]]`, `[providers]` apps/folders/calculator/aliases/web_search/unit_conversion/currency_conversion/time_lookup/utility_actions fields, `[actions] alternate_folder_opener_enabled/alternate_folder_opener_command`, `[appearance] theme/density/max_results/show_tooltips`, `[ranking] learn_from_usage`, and separate versioned `modules.toml` schema are implemented.

Before adding more schema, define:

- Which fields are stable public config.
- Which fields are internal state and should not live in config.
- How unknown fields are handled.
- How config migration works if schema versions are added.
- Which settings can be changed from the UI.

## State Versus Config

Config should store user intent: `config.toml` owns launcher-wide settings and the core app/folder provider switches, while `modules.toml` owns bundled-module enablement. The legacy module-backed booleans remain in `config.toml` only as a compatibility mirror during this migration.

Learned ranking should not be stored in `config.toml`. It should live under the XDG state directory so users can reset it without losing preferences:

```sh
~/.local/state/rayslash/
```

Cache-like data, such as expensive discovered indexes if they are added later, should live under the XDG cache directory:

```sh
~/.cache/rayslash/
```

Runtime IPC stays under `$XDG_RUNTIME_DIR` as documented in [ARCHITECTURE.md](ARCHITECTURE.md).
