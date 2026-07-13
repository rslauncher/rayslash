# Architecture

## Selected stack

- Rust workspace.
- Slint for the desktop UI.
- `rayslash-core` for pure Rust launcher logic.
- `rayslash` UI binary in `crates/rayslash-ui`.
- `serde` and `toml` for config loading.
- `dirs` for platform config paths.
- `nucleo-matcher` for app/project fuzzy matching and ranking.
- `arboard` for copying successful calculator results to the system clipboard.

## Why Rust and Slint

Rust gives the launcher a small native binary, predictable performance, and a strong base for testable parsing, indexing, and launching logic. Slint provides a lightweight native UI toolkit without pulling in a browser runtime.

## Wayland approach

For v1, `rayslash` uses a normal desktop window. Raw Wayland protocols, layer shell, GTK layer shell, and GNOME/KDE-specific overlay APIs are intentionally avoided so the app can work under Wayland and X11 through the toolkit.

Global shortcuts should be configured in the desktop environment. The intended command is:

```sh
rayslash toggle
```

Desktop setup steps live in [SHORTCUTS.md](SHORTCUTS.md).

## Crate and module structure

- `crates/rayslash-core/src/config.rs`: config types, config/state path helpers, and TOML loading/saving.
- `crates/rayslash-core/src/providers/mod.rs`: internal `Provider` trait, built-in catalog exports, diagnostics defaults, and aggregate execution-hint selection.
- `crates/rayslash-core/src/providers/types.rs`: stable provider IDs, metadata, permissions, runtime config, query context, typed actions, provider results/outcomes, diagnostics, and local/debounced-network execution hints.
- `crates/rayslash-core/src/providers/builtins.rs`: the two core provider implementations and metadata catalog for Apps and Folders.
- `crates/rayslash-core/src/modules/descriptors.rs`: stable metadata for the seven official module identities and migration compatibility; it contains no provider implementation.
- `crates/rayslash-core/src/modules/state.rs`: versioned `modules.toml` loading, first-run legacy seeding, official-module enable/disable state, compatibility mapping, unknown-field preservation, and atomic saving.
- `crates/rayslash-core/src/search.rs`: central mixed-provider orchestration, suppression/exclusivity handling, ranking, and the public search API.
- `crates/rayslash-core/src/search/result.rs`: search result types, icons, kinds, stable IDs, provider ownership, and typed provider-action projection.
- `crates/rayslash-core/src/search/providers.rs`: shared result constructors plus placeholder and no-results construction used by the built-in providers.
- `crates/rayslash-core/src/search/matcher.rs`: fuzzy matcher setup, deterministic ordering, and learned-ranking score boost integration.
- `crates/rayslash-core/src/ranking.rs`: learned ranking state format, load/save/clear helpers, and bounded boost calculation.
- `crates/rayslash-core/src/actions.rs`: action construction, launch, app activation, and delayed action helpers.
- `crates/rayslash-core/src/aliases.rs`: alias target normalization, kind inference, and display helpers.
- `crates/rayslash-core/src/app_state.rs`: generated app install state for new-app flair tracking.
- `crates/rayslash-core/src/web_search.rs`: explicit default-browser `search` trigger support, additional search-engine trigger matching, keyword detection, and URL template rendering.
- `crates/rayslash-core/src/units.rs`: local unit conversion parsing and conversion tables.
- `crates/rayslash-core/src/currency.rs`: explicit currency conversion parsing and Frankfurter pair-rate lookup/cache.
- `crates/rayslash-core/src/time_lookup.rs`: explicit `time in <place>` parsing, Open-Meteo place/timezone lookup, and in-memory place/timezone cache.
- `crates/rayslash-core/src/utility_actions.rs`: reboot, shutdown, logout, timer, and reminder parsing plus command construction.
- `crates/rayslash-core/src/projects.rs`: shallow project folder discovery.
- `crates/rayslash-core/src/apps.rs`: desktop app public API and shared `DesktopApp` type.
- `crates/rayslash-core/src/apps/app_discovery.rs`: application directory traversal, desktop-file collection, deduplication, icon resolution wiring, and app ordering.
- `crates/rayslash-core/src/apps/desktop_entry.rs`: `.desktop` parsing, filtering, and `Exec` tokenization.
- `crates/rayslash-core/src/apps/icon_lookup.rs`: desktop icon resolver module root and cache.
- `crates/rayslash-core/src/apps/icon_lookup/themes.rs`: best-effort icon-theme directory discovery, configured/common theme ordering, and theme inheritance.
- `crates/rayslash-core/src/apps/icon_lookup/paths.rs`: absolute and named desktop icon path resolution.
- `crates/rayslash-core/src/calc.rs`: calculator public API, expression detection, and result formatting.
- `crates/rayslash-core/src/calc/parser.rs`: numeric expression parser, functions, constants, implicit multiplication, and superscript exponents.
- `crates/rayslash-core/src/calc/equation.rs`: linear equation parsing and solving for `x`.
- `crates/rayslash-core/src/calc/error.rs`: calculator error types and user-facing messages.
- `crates/rayslash-ui/src/main.rs`: binary entry point, process startup, Slint component construction, IPC hookup, and high-level callback wiring.
- `crates/rayslash-ui/src/cli.rs`: command-line parsing for `rayslash` and `rayslash toggle`.
- `crates/rayslash-ui/src/ipc.rs`: Unix domain socket IPC for single-instance show/toggle requests.
- `crates/rayslash-ui/src/window_state.rs`: launcher show/hide helpers and IPC visibility dispatch.
- `crates/rayslash-ui/src/runtime_state.rs`: profiling helpers, runtime ranking load fallback, search truncation, selected-index policy, and shared result/settings UI refresh helpers.
- `crates/rayslash-ui/src/activation.rs`: selected-result activation, status messages, clipboard copying, launch dispatch, and learned-ranking recording.
- `crates/rayslash-ui/src/settings_callbacks.rs`: settings callback registration, autosave, folder picker integration, and ranking clear flow.
- `crates/rayslash-ui/src/module_settings.rs`: runtime module config loading, module-row model construction, guarded module toggles, compatibility mirroring, and result refresh.
- `crates/rayslash-ui/src/settings.rs`: settings property population, settings diagnostics readouts, and settings field parsing helpers.
- `crates/rayslash-ui/src/result_items.rs`: core search result to Slint item conversion and UI-side icon image cache.
- `crates/rayslash-ui/src/opener_visual.rs`: alternate opener app matching, picker item construction, icon/background selection, and fallback labels.
- `crates/rayslash-ui/ui/rayslash.slint`: main Slint `AppWindow` composition, public properties/callbacks, header, and shortcut/status hint.
- `crates/rayslash-ui/ui/models.slint`: shared Slint result, app choice, alias, web-search, and module item structs used by Rust and UI components.
- `crates/rayslash-ui/ui/components.slint`: reusable Slint components for the search box, settings toggle, and inline gear icon.
- `crates/rayslash-ui/ui/settings_panel.slint`: in-launcher General/Modules navigation, General settings, alternate opener picker, diagnostics, and settings save wiring.
- `crates/rayslash-ui/ui/modules_panel.slint`: Installed/Official/Community catalog tabs with verified metadata, permissions, install/update/remove, data deletion, and enable controls.
- `crates/rayslash-ui/ui/result_list.slint`: result viewport, row rendering, fallback icons, hover/click activation, and selection scrolling.
- `crates/rayslash-core/tests/`: crate-level integration tests for actions, calculator behavior, config compatibility, desktop-entry parsing, desktop app discovery, icon fixtures, project search, mixed provider orchestration, module descriptors/state migration, provider toggles, stable result IDs, and learned ranking.
- `crates/rayslash-core/tests/fixtures/`: reusable synthetic temp-dir, desktop-entry, icon-theme, config, project, app, and learned-ranking fixtures.

## Install model

The installable binary is `rayslash` from the UI crate at `crates/rayslash-ui`. A local user install from the workspace uses:

```sh
cargo install --path crates/rayslash-ui
```

Desktop shortcuts should invoke the installed binary with `rayslash toggle` instead of using `cargo run`.

Linux desktop integration files live under `packaging/linux`. The desktop entry template is:

```sh
packaging/linux/dev.rayan6ms.rayslash.desktop
```

It uses app ID `dev.rayan6ms.rayslash`, `Name=rayslash`, `Exec=rayslash toggle`, `Icon=dev.rayan6ms.rayslash`, and `StartupWMClass=dev.rayan6ms.rayslash`. It is marked `NoDisplay=true` because the app is primarily shortcut-driven and should not add a redundant menu item by default. The desktop shortcut command remains `rayslash toggle` even when the desktop entry is installed. At runtime, the UI selects the Slint backend before component construction, sets Slint's XDG app ID to the same value, and sets the window icon from `icons/rayslash-icon.svg` so Wayland/X11 panels can match the installed desktop entry and icon.

## Interaction model

The UI crate should remain thin. It asks `rayslash-core` for config, searchable items, and executable actions, then renders and dispatches user intent back to core services.

The launcher panel is a compact fixed stack: header, search field, single-line shortcut/status hint, bounded result viewport, and optional result tip text. The right side of the header contains an unframed inline-vector gear Settings button that opens Settings inside the same launcher panel. Settings has top-level `General` and `Modules` navigation; Modules always exposes `Installed`, `Official`, and `Community` catalog tabs, including install/update/remove and permission-review actions. Esc closes Settings back to the launcher view first; pressing Esc from the launcher view hides the window. Results live inside a Slint `Flickable`, so long app/folder lists scroll within the middle of the panel instead of pushing the hint line out of view. Keyboard and mouse selection remain clamped and visible, query changes reset selection/scroll appropriately, and the UI hides on ordinary focus loss while suppressing focus loss caused by the folder picker.

Current calculator/app/project and resident flow:

1. `rayslash` parses CLI arguments in the UI crate. Supported forms are `rayslash` and `rayslash toggle`.
2. Unknown CLI arguments print `Usage: rayslash [toggle]` and exit non-zero.
3. `rayslash` with no args sends `show` to an existing resident instance when one is already running; otherwise it starts the resident GUI.
4. `rayslash toggle` sends `toggle` to an existing resident instance and exits quickly.
5. If `rayslash toggle` cannot contact a resident instance, it starts the resident GUI and shows the launcher.
6. The resident process binds a Unix domain socket at `$XDG_RUNTIME_DIR/rayslash.sock`. If `XDG_RUNTIME_DIR` is unset, it falls back to a user-specific subdirectory under the system temp directory.
7. If binding finds an existing path, startup attempts to connect to it. A successful connection means another instance is running. A failed connection marks the path stale, removes it, and binds a fresh listener.
8. The IPC listener runs on a small blocking thread and accepts newline-delimited `show` and `toggle` requests.
9. IPC requests are forwarded onto the Slint event loop before touching UI state.
10. `show` resets the query, status line, default result list, result scroll position, and selection to no row, then shows the window and focuses the search field.
11. `toggle` hides the launcher if it is currently visible and otherwise performs the same show/reset/focus behavior as `show`.
12. UI loads `~/.config/rayslash/config.toml`, then loads or creates versioned `~/.config/rayslash/modules.toml`. Fresh users receive an empty version-2 module config. Existing version-1 entries are backed up and become explicit `Restore` choices without downloading code. A main-config or module-config read/parse/version failure blocks writes rather than overwriting the broken file.
13. UI loads learned ranking state from `~/.local/state/rayslash/ranking.toml`, falling back to empty state if the file is missing, corrupted, or unsupported.
14. Core scans immediate visible child directories of configured folder sources.
15. Core recursively scans desktop application files under `$XDG_DATA_HOME/applications`, falling back to `~/.local/share/applications`, and under each `$XDG_DATA_DIRS` `applications` directory, falling back to `/usr/local/share/applications` and `/usr/share/applications`.
16. Core parses the `[Desktop Entry]` group from `.desktop` files.
17. App entries support `Name`, localized names, `GenericName`, localized generic names, `Comment`, localized comments, `Exec`, `Icon`, `StartupWMClass`, `MimeType`, `Categories`, `Keywords`, `Actions`, `DBusActivatable`, `TryExec`, `NoDisplay`, `Hidden`, `Type`, `OnlyShowIn`, and `NotShowIn`.
18. Core keeps only `Type=Application` entries with `Name` and an available launch target, excluding `NoDisplay=true`, `Hidden=true`, entries whose `TryExec` target is missing, entries whose `Exec` program is unavailable when `TryExec` is absent, and entries excluded by the current desktop environment. `DBusActivatable=true` entries launch through `gio launch <desktop-id>` when no direct `Exec` command is used.
19. Core parses each desktop `Exec` line into a command spec with a program and argument vector.
20. Core resolves app icon paths from parsed `.desktop` `Icon` fields on a best-effort basis.
21. UI sends the query string to central core search. When a custom search-engine pill is active, the UI sends the effective query as `<keyword> <terms>`.
22. Core runs Apps and Folders directly. Enabled installed modules are queried in parallel through persistent, deadline-bounded host processes and return typed results/actions over versioned JSON IPC.
23. Calculator expressions support decimal numbers, addition, subtraction, multiplication, division, exponentiation with `^`, `**`, or superscript digits such as `10²`, parentheses, unary signs, implicit multiplication such as `2(3 + 4)` and `2pi`, constants `pi` and `e`, and common one-argument functions such as `sqrt`, `abs`, `round`, `sin`, `cos`, `tan`, `ln`, `log`, and `exp`.
24. Calculator equations support one variable named `x`, one equals sign, and linear operations over the same expression grammar, such as `x + 10 / 2 = 8`, `2x + 4 = 10`, and `2(x + 3) = 10`. Nonlinear equations such as `x^2 = 4` or division by `x` return calculator error rows.
25. Calculator detection requires a calculation signal such as an operator, equals sign, function call, superscript exponent, or implicit multiplication. Plain app/project queries such as `calculator`, `code`, and `pi` do not become calculator rows.
26. Core combines module results with Apps and Folders, honoring module exclusivity and the configured result cap. Apps are matched against app names, localized names, generic names, comments, and keywords; folders use case-insensitive fuzzy matching.
27. Configured aliases are searched by alias name and query when the alias provider is enabled. URL, file, and folder aliases activate through `xdg-open`; command aliases are parsed into direct program/argument values and spawned without a shell.
28. The permanent first `Web Search` template requires the explicit `search` command, such as `search manhattan`, or its Search pill activated with Space or Tab. Its default Google `%s` URL is rendered exactly like every other configured engine and opened through `xdg-open`; changing that URL changes the default for every browser family.
29. Configured additional search engines use explicit trigger prefixes such as `yt rust slint`; the configured `%s` placeholder is replaced with percent-encoded search terms before activation. Typing a configured keyword and pressing Space or Tab makes the UI show that keyword as an active search pill and sends the effective query to core as `<keyword> <terms>`.
30. Unit conversion is local and supports explicit length, mass, volume, and temperature queries such as `10 km to mi`, `10mi to km`, and `32 f to c`. Conversion-like queries suppress calculator error rows so unit text does not show calculator-only diagnostics.
31. Currency conversion uses explicit three-letter currency codes, fetches pair rates from the public Frankfurter v2 API, and caches rates in memory for the resident process. Remote currency and time work is debounced for 450ms and performed away from the Slint event loop; generation checks discard results from queries that have already changed.
32. Time lookup uses explicit `time in <place>` syntax. Common country names such as `america` and `brazil` resolve directly without network access; other places use one Open-Meteo geocoding request. Punctuation-insensitive matching accepts forms such as `washington dc`. Country results expand locally through the installed IANA timezone database and are grouped by distinct current UTC offset, so multi-zone countries return several meaningful rows without one HTTP request per timezone. Time queries suppress unrelated provider results, and time subtitles expose their full regional description through the normal result tooltip.
33. Core result values include display-only icon metadata and flair text separately from typed activation data in `SearchResultKind`. The provider boundary also assigns every row a stable `ProviderId` and projects the row into a typed `ProviderAction` such as copy text, show message, open URL/folder, launch app/alias, run utility, dismiss, or no action.
34. Valid calculator results are inserted above app and project matches. The title is the computed result or solved equation, and the subtitle is `Calculate: <expression>`.
35. Math-like invalid calculator queries, such as formula syntax errors, division by zero, incomplete expressions, unsupported characters, unknown functions, domain errors, non-finite results, and unsupported nonlinear equations, return a calculator row with the error message as the title and `Calculate: <expression>` as the subtitle. Valid unit and currency conversions are inserted before calculator errors so conversion syntax such as `10c to k` shows the conversion first.
36. UI displays app titles as app names and app subtitles as the app `Comment` or `GenericName`, falling back to `Application` when neither field exists. New desktop app IDs discovered after the first app-state baseline display a `New` flair after the app name until they are successfully launched from rayslash.
37. UI displays project paths in compact form, using `~/...` for paths under the current user's home directory while keeping the full real path in the result kind.
38. UI maps icon metadata to Slint row rendering. App icon paths are loaded as Slint images with a small UI-side cache and rendered unframed at the same 42px target size as the fallback icons; calculator, unit conversion, currency conversion, time lookup, power/reboot/logout, timer, web search badges, folder, generic app, and placeholder icons are drawn as lightweight Slint fallback shapes. Folder path, app-description, and no-results subtitles expose delayed hover detail text when the result row is hovered.
39. UI keeps shortcut help outside the scrollable result viewport in a single line below the search input. Shortcut key labels use a lighter color than their descriptions. Action, calculator, and error feedback reuse that line when needed.
40. Empty queries leave selection on the search input/no row. Non-empty queries with matches select the first result.
41. Hovering a visible result row selects/highlights it, including partially clipped rows at the viewport edges. Mouse selection does not adjust the result scroll position; keyboard navigation remains responsible for scrolling selected rows fully into view.
42. Clicking a result row activates it through the same callback path as Enter.
43. Enter activates a selected calculator, unit conversion, currency conversion, or time lookup result by copying the result text to the clipboard and hiding the launcher after a successful copy.
44. Enter activates a selected calculator, currency, time lookup, timer, or system-action error row internally by echoing the error message in the status line and keeping the launcher visible.
45. Web search is represented by configured URL templates only. The first permanent template is `Web Search`, uses keyword `search`, and defaults to `https://www.google.com/search?q=%s`. It can be disabled and its URL can be edited, but it cannot be removed. Incomplete additional rows are persisted as inactive drafts and filtered by the provider until valid. Non-default engine favicons are fetched into the XDG cache in the background, normalized to PNG, shown in settings and matching result rows, and sampled with the same muted-accent technique used by the alternate folder opener; that accent colors the active search pill. The permanent default retains its neutral magnifying-glass treatment. All web-search activation opens the rendered URL through `xdg-open`, normally handing it to an existing browser as a new tab without browser-family-specific command handling.
46. Enter activates a selected no-results row by hiding the launcher.
47. Enter activates a selected app result by asking core to focus an existing app window when possible, then to launch through desktop activation (`gio launch <desktop-id>`) with a parsed desktop command fallback.
48. Enter activates a selected folder result by asking core to spawn `xdg-open <project-path>` on Linux.
49. Enter activates a selected alias result by asking core to open the alias URL/file/folder target or spawn the alias command target.
50. Ctrl+Enter and Ctrl+click activate a selected app result the same way as Enter for now.
51. Ctrl+Enter and Ctrl+click activate a selected folder result by asking core to spawn the configured alternate folder opener. The configured value is parsed into direct program/argument values without a shell. Most commands receive the folder path as their final argument; the default `xdg-terminal-exec` is launched with the folder as its working directory and no implicit folder argument.
52. Core constructs launch commands as separate program and argument values and spawns them directly through Rust process APIs without a shell.
53. Core sets child stdin, stdout, and stderr to `Stdio::null()` before spawning external app, folder, editor, alias, and web-search open actions, so successfully launched GUI children do not inherit rayslash's terminal streams.
54. UI hides the launcher window after a successful app, folder, editor, alias, web search, timer/system action scheduling, calculator-result copy, utility-result copy, or no-results activation.
55. UI hides the launcher window on Escape or when the winit window reports `Focused(false)`.
56. UI keeps the launcher visible and reports a clear status message after failed app, folder, editor, alias, web search, or clipboard actions.
57. UI leaves placeholder rows as preview-only items.
58. After a successful app or folder activation, UI records the selected result ID, launch count, last launched Unix timestamp, and query prefixes into local ranking state when `ranking.learn_from_usage` is enabled. Successful app launches also clear that app ID from generated new-app state.
59. The General settings page autosaves folder sources, legacy alias/search rows, core Apps/Folders toggles, alternate folder opener, learned ranking, theme, density, result limits, and tooltips to `config.toml`. The Modules page loads Installed/Official/Community metadata from the verified registry and performs asynchronous install, restore, repair, update, enable, disable, removal, and separate data-removal operations. Module config changes are atomic, permission expansion requires explicit approval, and successful state changes refresh the active query. General settings saves back up an existing `config.toml`; writes remain blocked after unsafe config fallback so user-authored configuration is never overwritten.
60. The settings panel can clear learned ranking history by removing `ranking.toml` and resetting the in-memory ranking state without changing user-authored config.
61. The resident process discovers desktop apps at startup and refreshes desktop app discovery when the settings panel opens, throttling repeated refreshes so rapid toggling does not repeatedly block the UI thread. This lets newly installed desktop entries appear without restarting rayslash when settings are opened while per-keystroke search and show/reset use the in-memory app list. Each refresh syncs generated new-app state, clears the UI icon image cache, rebuilds alternate opener picker choices, and updates derived opener visuals and settings diagnostics.

Empty mixed queries return all discovered apps and projects in stable title order, with apps before projects only when titles tie. Non-empty queries return utility rows first for explicit calculator, conversion, configured web-search, default `search`, time-lookup, timer/reminder, or system-action syntax, followed by fuzzy app/project matches sorted by descending score plus an optional learned boost. Valid conversions suppress calculator error rows so conversion syntax does not get hidden behind calculator diagnostics. Learned boost is local-only, deterministic for a given state file and query, and bounded to at most 20 points. It is only applied to app/folder rows whose title starts with the current query, with at most 8 points from launch count and at most 16 points from matching query-prefix count. Ties fall back to the original fuzzy score and then title/type/subtitle order. If real entries exist but the query matches none of them and no utility provider returns a row, core returns a single `No results` row with a short subtitle and a delayed detail tooltip.

Placeholder rows are only used when no enabled desktop apps, folder results, or aliases are available, and they only describe currently enabled providers.

The default folder action is intentionally neutral: open the selected folder in the system file manager. The secondary Ctrl+Enter shortcut opens the folder with the configured alternate opener command line, which defaults to `xdg-terminal-exec`.

Project rows switch their fallback folder icon while Ctrl is held, with a short background/opacity transition. If the configured alternate opener matches a discovered desktop app, the row shows that app icon on a small sampled/tinted background. Otherwise it uses a compact two-character command label. This is display-only state in the Slint UI; Ctrl+Enter and Ctrl+click behavior still comes from the activation path and core action helpers.

Desktop `Exec` parsing is intentionally conservative. The parser handles whitespace splitting, double-quoted arguments, simple backslash escapes, `%%` as a literal percent, and removal of field codes such as `%f`, `%F`, `%u`, `%U`, `%i`, `%c`, and `%k`. Real discovery also filters entries with missing `TryExec` targets, missing `Exec` programs when no `TryExec` is provided, or incompatible `OnlyShowIn`/`NotShowIn` metadata. Localized labels, keywords, desktop actions, and `DBusActivatable` are parsed; desktop actions are stored for compatibility and future action UI work, while normal result activation still launches the primary app command. It does not expand field codes or run through a shell, so desktop entries that depend on shell syntax or field-code expansion may need later compatibility work.

Desktop app icon lookup is still intentionally small, but it is theme-aware enough for common desktops. Absolute icon paths are accepted when they exist and either use a supported extension or are extensionless files, which covers common AppImage desktop entries that point at PNG files without a suffix. The UI image loader materializes extensionless PNG, JPEG, and SVG icon files into `~/.cache/rayslash/icons` with a detected extension before loading them through Slint. Named icons are checked first against configured/common icon themes, including GTK/GNOME `gsettings`, GTK settings files, KDE `kdeglobals`, common installed themes such as Papirus, and simple `Inherits=` values from `index.theme`. The resolver then falls back to hicolor and pixmaps-style paths, includes common symbolic and `apps/scalable` app-icon directories, and has a conservative reverse-DNS suffix fallback for entries such as WPS 2019 whose desktop `Icon` name differs from the installed theme icon. For named app icons, launcher-sized assets such as 42x42, 48x48, and nearby sizes are preferred before scalable or very large assets. Supported named-icon extensions are SVG, PNG, JPG, and JPEG. The implementation does not fully implement the freedesktop icon-theme algorithm, all index metadata, symbolic recoloring, or guaranteed current-theme detection on every desktop.

If more results exist than `appearance.max_results`, the UI shows only the capped real results and displays a separate non-selectable tip such as `Max results: 36` after the last real row inside the result scroll content.

External launch actions are fire-and-forget after a successful spawn. Spawn errors still return through the core action helper so the UI can keep the launcher visible and report a status message, but child process output is intentionally discarded.

## Implemented provider boundary

The first internal provider boundary lives under `crates/rayslash-core/src/providers/`:

- `types.rs` defines `ProviderId`, display metadata, declared permissions, enabled state, diagnostics, `ProviderContext`, `ProviderResult`, typed `ProviderAction`, `ProviderOutcome`, and `ProviderExecutionHint`.
- `builtins.rs` contains only `rayslash.core.apps` and `rayslash.core.folders`. Optional provider IDs are supplied by installed packages and never compiled into the core catalog.
- `Provider::run` combines provider-specific query work with its execution hint. Currency and time can request a 450ms debounced-network path; other queries remain local.
- `search.rs` is the central orchestrator. It runs the catalog, stops calculator evaluation when an earlier provider suppresses it, honors exclusive outcomes, keeps utility/exact rows ahead of fuzzy candidates, applies learned boosts only to eligible actions, and owns fallback/placeholder/no-results behavior.
- Provider metadata includes ranking eligibility and current network/filesystem/process/clipboard requirements so future module work does not need to infer those capabilities from result variants.

The public module boundary is the separately versioned SDK WIT contract. Registry signatures, pinned package digests, safe atomic extraction, no-WASI execution, capability checks, and typed activation keep third-party code outside the launcher process.

Phase 13 keeps Apps and Folders in core, distributes API v1 WASM modules as immutable `.tar.zst` GitHub Release assets, and verifies their digests through a signed static registry served by GitHub Pages with raw-GitHub and last-verified-cache fallbacks. Modules use the versioned WIT contract through a separately maintained `rayslash-module-host` process with no ambient WASI capabilities. Supported app packages require or include that host so module installation works out of the box, while no official or community module code is bundled. The declarative manifest value is reserved and rejected in API v1. See [manual_migration.md](manual_migration.md) for the authoritative prerequisites and acceptance criteria.

## Config and state model

Config and generated state should stay separate:

- Config: `~/.config/rayslash/config.toml`
- Module config: `~/.config/rayslash/modules.toml`
- State: `~/.local/state/rayslash/`
- Cache: `~/.cache/rayslash/`
- Runtime IPC: `$XDG_RUNTIME_DIR/rayslash.sock`, falling back to a user-specific temp subdirectory when `XDG_RUNTIME_DIR` is unavailable.

`config.toml` stores launcher-wide intent such as folder sources, the core app/folder toggles, theme, aliases, and command preferences. `modules.toml` stores versioned official-module enablement separately and is also user-authored config. Learned ranking should live in state so it can be cleared independently. Future expensive indexes, if added, should live in cache.

The implemented public config fields are currently:

- `folder_sources`
- `[[aliases]]`
- `[[web_searches]]`
- `[providers] apps`, `folders`, `calculator`, `aliases`, `web_search`, `unit_conversion`, `currency_conversion`, `time_lookup`, and `utility_actions`
- `[actions] alternate_folder_opener_enabled` and `alternate_folder_opener_command`
- `[appearance] theme`, `density`, `max_results`, and `show_tooltips`
- `[ranking] learn_from_usage`

The General settings UI autosaves those known `config.toml` fields, including alias and web-search entries. Unknown main-config fields are ignored when loading and are not preserved when General autosaves, but an existing `config.toml` is backed up before replacement. Older `project_roots`, `providers.projects`, `actions.project_editor_command`, `web_searches.query`, and `web_searches.url_template` keys are accepted on read for compatibility.

`modules.toml` uses `version = 2`, contains only installed or migrated module entries keyed by module ID, and is written with the same atomic temp-file/rename helper as other generated TOML writes. Its schema flattens unknown top-level fields and unknown fields inside each module entry so they survive a load/save round trip. Parse-corrupt or unsupported versions are never replaced with fallback defaults; module writes remain blocked until the file is fixed and rayslash is restarted.

On a fresh startup without earlier configuration, rayslash creates an empty version-2 file. An existing version-1 file is backed up and converted into explicit `Restore` choices without downloading code. Install, toggle, update, and removal operations update `modules.toml`; Apps and Folders remain core settings and are never inserted as module entries.

The implemented learned ranking state file is:

- `~/.local/state/rayslash/ranking.toml`

The implemented app install state file is:

- `~/.local/state/rayslash/apps.toml`

It uses `version = 1` and stores entries by stable result ID. Current learned IDs are `app:<desktop-id>` for desktop apps and `folder:<absolute-path>` for folders. Calculator and no-results rows have stable IDs for consistency but are not learned from in this phase. State entries store `launch_count`, `last_launched_unix`, and `query_prefixes`. Ranking state is pruned after learned launches: IDs missing from the current app/folder index are removed, entries older than 180 days are removed, each entry keeps at most 64 query prefixes, and the state keeps at most 1000 entries by recency. Unsupported versions and corrupted files fall back to empty state so startup remains reliable.

## Performance profiling

`rayslash` intentionally discovers configured project folders and desktop apps at resident startup, then keeps those lists in memory so every query can be matched immediately on the UI thread. Project scanning is shallow: it reads only immediate visible child directories under configured roots. Desktop app discovery recursively scans XDG application directories, parses `.desktop` files, and resolves icon paths up front; Slint image objects are cached UI-side after they are first loaded. Desktop apps are also refreshed when settings opens, with repeated refreshes throttled, so normal resident use can pick up newly installed apps without adding discovery work to each typed query or every launcher show/reset.

Set `RAYSLASH_PROFILE=1` to print lightweight timing lines for startup stages, settings-open app refresh, and query refresh phases such as core search, result-item conversion, model replacement, and UI property updates:

```sh
RAYSLASH_PROFILE=1 rayslash
```

The profiling output is intentionally opt-in so normal shortcut launches stay quiet. It is meant to identify whether local cost is coming from config load, project scan, app discovery/icon path resolution, result item construction, Slint model replacement, app refresh, or per-query matching before changing the indexing strategy. A larger synthetic search probe is also available with `cargo test -p rayslash-core --test performance -- --ignored --nocapture`, and comparable results are recorded in [PERFORMANCE.md](PERFORMANCE.md).

Current provider flow:

1. UI supplies the effective query and current config/data snapshots.
2. Core runs the built-in provider catalog and returns centrally composed, ranked `SearchResult` values.
3. Each result exposes its owning provider ID and typed provider action while preserving the established result-kind compatibility surface.
4. UI activates the selected result through the existing safe core action helpers.

## Config

The config file lives at:

```sh
~/.config/rayslash/config.toml
```

Supported fields:

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
alternate_folder_opener_enabled = true
alternate_folder_opener_command = "xdg-terminal-exec"

[appearance]
max_results = 36
show_tooltips = true

[ranking]
learn_from_usage = true
```

Configured roots may use `~` or `~/...`, which are expanded to the current user's home directory before scanning.

If the file is missing, the default folder source is:

- `~`

The app does not create folders automatically.

Provider toggles default to enabled. The seven module-backed compatibility values are seeded into `~/.config/rayslash/modules.toml` on its first creation; afterward that module config is authoritative for Calculator, Aliases, Web Search, Units, Currency, Time, and Timers, while Apps and Folders remain core `config.toml` settings. `alternate_folder_opener_enabled` defaults to true, and `alternate_folder_opener_command` defaults to `xdg-terminal-exec`. Alternate opener command lines are parsed without a shell; most commands receive configured arguments followed by the folder path, while `xdg-terminal-exec` is launched from the selected folder working directory. `max_results` defaults to `36`, `show_tooltips` defaults to true, `ranking.learn_from_usage` defaults to true, and shortcut hints are visible by default. Full module config and migration behavior is documented in [CONFIG.md](CONFIG.md).

## Startup and toggle model

`rayslash` runs as a single resident GUI process. The first invocation starts the Slint window and binds the local IPC socket. Later invocations contact that socket and exit quickly.

The IPC socket path is:

```sh
$XDG_RUNTIME_DIR/rayslash.sock
```

If `XDG_RUNTIME_DIR` is unavailable, rayslash falls back to a user-specific subdirectory under the system temp directory, for example `/tmp/rayslash-1000/rayslash.sock`. This fallback is mainly for tests and unusual environments; normal Linux desktop sessions should provide `XDG_RUNTIME_DIR`.

Desktop environments should bind their global shortcut to:

```sh
rayslash toggle
```

The app does not capture or register global shortcuts itself.
