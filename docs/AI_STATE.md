# AI State

Last updated: 2026-07-11

## Current status

Phase 0, Phase 1, Phase 2, the first Phase 3 desktop app launcher pass, the Phase 3 launch-stdio cleanup, Phase 4, Phase 5, Phase 6, Phase 7 settings/customization, Phase 8 learned-ranking/aliases, Phase 9 refactoring/internal-boundaries, and Phase 10 packaging/Linux-integration work are complete. The first Phase 12 utility-provider pass now has default-enabled explicit default-browser `search`, configurable additional web search engines, local unit conversion, currency conversion through Frankfurter pair rates, time lookup through Open-Meteo place/timezone metadata, reboot/shutdown/logout actions, timer/reminder notifications, and generated new-app flair state for newly discovered desktop apps. Phase 10 added a packaging inventory, AppStream/metainfo metadata, metadata validation script, GitHub Actions CI, Fedora spec, Arch/AUR `PKGBUILD`, Flatpak prototype manifest, and AppImage deferral note. Phase 9 split core app/search/icon/calculator modules, UI callback/helper modules, reusable Slint components, and core integration fixtures while preserving current behavior. Phase 7 now has an in-launcher settings panel with an unframed gear header button, folder sources with a native folder picker, alias and additional search-engine row editors, current provider toggles including aliases and Phase 12 providers, an optional alternate folder opener with an installed-app picker filtered to apps that declare `inode/directory` or use folder-relevant categories such as file managers, terminals, and IDEs, max result count, dark/dim/light theme controls, density controls, learned-ranking controls, autosave for settings changes, Esc-to-return from settings to the main launcher view, and diagnostics including resolved app icon count. Phase 8 has local learned ranking plus aliases/quick links for URLs, files, folders, and explicit no-shell commands. Desktop app discovery now filters unavailable desktop entries with missing `TryExec` targets, missing `Exec` programs when no `TryExec` is present, incompatible `OnlyShowIn`/`NotShowIn` metadata, and supports localized labels, keywords, desktop actions, `StartupWMClass`, and `DBusActivatable`. Phase 6 has local install documentation, a Linux desktop entry template, launcher/window icon polish, theme-aware app icon lookup, real launcher UI/UX fixes for shortcut placement, scrollable results, keyboard/mouse selection, opening selection state, click-outside hiding, header icon alignment, app icon display quality, Ctrl-held folder opener row icon switching, and opt-in performance profiling. The previous Phase 1 show-animation polish item is considered good enough for v1 after manual testing.

A post-v1 planning pass now exists. [ROADMAP.md](ROADMAP.md) treats Phases 0 through 6 as the completed v1 baseline and adds public-readiness phases for settings/customization, learned ranking/aliases, refactoring/internal boundaries, packaging/Linux integration, project maturity, and optional providers. [TASKS.md](TASKS.md) has matching unchecked tasks, and the cleanup audit backlog is complete as of 2026-07-02. [COMPARISON.md](COMPARISON.md) records launcher comparisons and product lessons. [REFACTORING.md](REFACTORING.md) records the Phase 9 module/test split plan. [TESTING.md](TESTING.md) records the testing maturity plan. [CLEANUP_AUDIT.md](CLEANUP_AUDIT.md) records bugs, inconsistencies, optimization opportunities, redesign candidates, and odd implementation details found during a cleanup-focused pass.

The repository is a Cargo workspace with a pure Rust core crate and a Slint UI binary crate. `cargo run -p rayslash` starts a single resident GUI instance, opens a frameless launcher with a search input, mixed calculator, desktop app, and folder results when available, and provider-aware placeholder results only when no enabled desktop apps or folder results are available and the query is not a valid calculation.

The module migration now uses on-demand verified GitHub Release packages, a signed static GitHub Pages registry with raw/cache fallbacks, and a separately installed no-WASI host. Fresh configurations contain no optional modules. Apps and Folders are the only built-in providers; the extracted Calculator, Units, Currency, Time, Web Search, Timers, and Aliases live in separate repositories. Existing virtual-module entries appear as explicit `Restore` actions and never trigger silent downloads. See [manual_migration.md](manual_migration.md) for repository and release state.

The latest utility/settings correction moves remote time and currency resolution out of the per-keystroke UI path, waits for the provider-declared debounce interval, and discards stale query generations. Time lookup canonicalizes punctuation-free names such as `washington dc`, prevents `america` from resolving to the same-named Dutch village, suppresses unrelated results, and expands countries through the local IANA timezone database into rows grouped by current UTC offset. Time tooltips allow three wrapped lines and clamp their screen position inside the result viewport. The permanent first `Web Search` template uses keyword `search` and a user-editable Google `%s` URL for every browser; it can be disabled but not removed. Search-engine and alias settings use compact bordered field cards with a larger identity header, small save action, and red SVG trash action. Additional search engines persist incomplete rows as inactive drafts on field focus loss, show an amber warning until valid, and use cached favicons in both settings and result rows with a magnifying-glass/keyword fallback. System actions support fuzzy partial queries and additional aliases, run immediately unless a delay is explicit, and participate in learned ranking without tests executing the destructive commands.

The current UI supports dark, dim, and light themes, keyboard and mouse selection, a header icon loaded from `icons/rayslash-icon.svg`, an unframed gear Settings button in the header, result row icons, a bounded scrollable result area, a compact single-line shortcut/status hint below the search field, no selected result on empty launch, first-result selection after typing a non-empty query with matches, a short no-results row with delayed detail tooltip, and a separate non-selectable max-results tip after the final real result when more results exist than the configured cap. Enter or click focuses/launches apps, opens folders through `xdg-open`, opens additional web search URLs through `xdg-open`, opens default-browser search rows only for explicit `search` queries, schedules reboot/shutdown/logout and timer/reminder actions, copies calculator/unit/currency/time lookup results, previews calculator/currency/time lookup/utility errors in the status line, and Ctrl+Enter or Ctrl+click opens folders through the configured alternate folder opener command line when enabled. Typing `search` or an enabled additional search-engine keyword and pressing Space or Tab turns it into an active search pill; Backspace on an empty active search clears the pill. Ctrl-held folder row icon switching previews the alternate opener action. Settings content scrolls inside the launcher panel, setting toggles have delayed 800ms hover detail text over the full toggle row, app/folder/no-results rows have delayed 800ms detail tooltips, and theme/density use segmented controls. Tooltips can be disabled through `appearance.show_tooltips` or the Settings Tooltips toggle. Hover selects visible result rows, including clipped partial rows, without moving the result scroll position, and result scrolling updates selection for the row under a stationary cursor. Learned ranking state is pruned after learned launches so removed app/folder IDs, old entries, and oversized prefix maps do not grow indefinitely. New desktop app IDs discovered after the initial app-state baseline show a `New` flair immediately after the app name until successfully launched. The resident process discovers desktop apps at startup and refreshes desktop app discovery when the settings panel opens, throttling repeated refreshes so rapid toggles do not repeatedly rescan desktop entries; refreshes sync new-app state, clear the UI icon image cache, and update opener picker choices, opener visuals, and settings diagnostics from the refreshed app list. Result-list refresh, settings-dependent UI refresh, and settings field validation now go through shared helpers instead of being duplicated across callbacks. Settings saves create a timestamped backup of an existing `config.toml` before replacement, unchanged focus-loss saves return without writing or rescanning, and saves are blocked if startup fell back to defaults because config loading failed. `rayslash toggle` contacts the resident process over a Unix domain socket and shows or hides the launcher.

## Finished

- Created workspace structure.
- Added `rayslash-core` crate.
- Added `rayslash` UI crate.
- Added initial config path helpers.
- Added placeholder search result data.
- Added required documentation files.
- Fixed Slint list model setup by wrapping placeholder results in `Rc<VecModel<_>>`.
- Installed required Fedora build dependencies:
  - `gcc`
  - `pkgconfig(fontconfig)`
- Verified formatting with `cargo fmt --check`.
- Verified tests with `cargo test`.
- Verified build with `cargo build`.
- Verified runtime launch with `cargo run -p rayslash`.
- Started Phase 1 UI polish:
  - Replaced the stock `LineEdit` with a custom `TextInput` search box for controlled contrast.
  - Added startup focus for the search field.
  - Improved panel spacing, borders, colors, and result row contrast.
  - Removed native window decorations with Slint's built-in `Window.no-frame` property.
- Verified the Phase 1 UI polish pass with `cargo fmt --check`, `cargo test`, and `cargo build`.
- Added Phase 1 placeholder navigation:
  - Tracks a selected result index in Slint.
  - Up/Down arrows move through placeholder results.
  - Selection is clamped at the first and last result rather than wrapping.
  - Enter calls a placeholder callback only.
  - Rust logs the selected placeholder title and updates the visible status text.
- Previously replaced the original beige palette as part of Phase 1 polish.
- Verified this navigation pass with `cargo fmt --check`, `cargo test`, and `cargo build`.
- Reworked the theme again to remove strong blue/slate accents:
  - Uses near-black grey window/panel backgrounds.
  - Uses neutral grey input, row, border, text, and placeholder icon colors.
  - Selected row is indicated with grey contrast and border, not blue.
  - Removed the unused light-theme branch for now.
  - Increased border radii and bottom padding.
  - Centered placeholder icons in result rows.
  - Added Tab and Shift+Tab as keyboard navigation aliases.
- Verified this visual polish pass with `cargo fmt --check`, `cargo test`, and `cargo build`.
- Fixed remaining Phase 1 visual issues:
  - Increased the launcher preferred height and minimum height so the layout is not compressed.
  - Increased bottom padding and added a small spacer before the status line.
  - Changed the outer Slint window/root backgrounds to `transparent`.
  - Enabled `clip: true` on the rounded main panel so children stay inside the rounded shape.
- Verified this visual bugfix pass with `cargo fmt --check`, `cargo test`, and `cargo build`.
- Started Phase 2 project launcher foundation:
  - `rayslash-core` loads config from `~/.config/rayslash/config.toml`.
  - Missing config falls back to sensible defaults.
  - Default project roots include only existing common directories under home:
    - `~/Projects`
    - `~/Code`
    - `~/Documents/Projects`
  - Project scanning lists immediate visible child directories of configured roots.
  - Project search uses `nucleo-matcher` fuzzy matching on folder names.
  - Project result subtitles display compact paths, using `~/...` for paths under the current user's home directory.
  - Matching project results are sorted by descending fuzzy score, with project name/path order as the deterministic tie-breaker.
  - Empty project queries show all projects in stable project name/path order.
  - Queries with no matching projects return an empty result list.
  - The UI updates the result list as the user types.
  - Enter on a project asks `rayslash-core` to spawn `xdg-open <project-path>` without shelling through `sh -c` or `zsh -c`.
  - Ctrl+Enter on a project asks `rayslash-core` to spawn `code <project-path>` without shelling through `sh -c` or `zsh -c`.
  - Successful folder or VS Code launch updates the status line and hides the launcher window immediately after the spawn succeeds.
  - Missing or unspawnable `xdg-open` or `code` keeps the launcher visible, updates the status line with a clear PATH/action-specific message, and logs the error.
  - Placeholder activation remains preview-only and harmless.
  - Added a core action-construction test that verifies the `code` program and project path argument without requiring VS Code to be installed.
  - Added a core action-construction test that verifies the `xdg-open` program and project path argument without requiring `xdg-open` to be installed.
  - Added core tests for compact home-relative display paths.
- Added the first Phase 3 desktop app launcher pass:
  - `rayslash-core` recursively scans `.desktop` files under:
    - `~/.local/share/applications`
    - `/usr/local/share/applications`
    - `/usr/share/applications`
  - Desktop entries are parsed from the `[Desktop Entry]` group.
  - Supported fields are `Name`, `GenericName`, `Comment`, `Exec`, `Icon`, `NoDisplay`, `Hidden`, and `Type`.
  - App discovery includes only `Type=Application` entries with both `Name` and `Exec`.
  - App discovery excludes `NoDisplay=true` and `Hidden=true`.
  - App result subtitles use `Application`, optionally followed by `Comment` or `GenericName`.
  - Desktop `Exec` lines are parsed into direct program/argument command specs without `sh -c` or `zsh -c`.
  - Desktop field codes are removed from parsed command args, with `%%` preserved as a literal percent.
  - App and project results are searched together with `nucleo-matcher`.
  - Empty queries show a mixed alphabetical app/project list.
  - Non-empty queries rank matching apps and projects together by descending fuzzy score.
  - Enter on an app launches the parsed app command and hides the launcher after successful spawn.
  - Ctrl+Enter on an app currently does the same as Enter.
  - Failed app launch keeps the launcher visible and shows a clear PATH-oriented status message.
  - External app/project launches detach child stdin, stdout, and stderr from the rayslash process by setting each stream to `Stdio::null()` before spawn.
  - Spawn failures are still returned to the UI for status text and logging, but output from successfully spawned GUI children is discarded.
  - Added core tests for desktop parsing/filtering, Exec parsing, and mixed app/project search.
- Completed Phase 3 launch-stdio cleanup:
  - Desktop app launches from parsed `.desktop` commands no longer inherit rayslash stdin, stdout, or stderr.
  - Project folder launches through `xdg-open <path>` no longer inherit rayslash stdin, stdout, or stderr.
  - Project VS Code launches through `code <path>` no longer inherit rayslash stdin, stdout, or stderr.
  - Added a core spawn-path test using the test binary itself, avoiding real GUI apps, VS Code, `xdg-open`, and a desktop session.
- Added the first Phase 5 resident/toggle foundation:
  - `rayslash` with no args starts the resident GUI when none is running.
  - A second `rayslash` invocation sends `show` to the resident instance and exits quickly.
  - `rayslash toggle` sends `toggle` to the resident instance and exits quickly.
  - `rayslash toggle` starts the resident GUI and shows the launcher when no instance is running.
  - Unknown CLI arguments print a short usage message and exit non-zero.
  - Local IPC uses a Unix domain socket at `$XDG_RUNTIME_DIR/rayslash.sock`, falling back to a user-specific temp subdirectory only when `XDG_RUNTIME_DIR` is unset.
  - Startup removes stale socket paths when no live process responds.
  - The IPC listener runs on a small blocking thread and forwards UI work onto the Slint event loop.
  - Toggle hides the launcher when visible and shows it when hidden.
  - Showing through IPC resets the query, result list, status line, and selection, then focuses the search field.
  - App/project launch success paths still hide the launcher, failed launches keep it visible, and child stdio remains detached.
  - Added UI-crate unit tests for CLI parsing, socket path construction, stale socket replacement, active socket detection, and IPC request parsing.
- Completed Phase 5 desktop shortcut documentation:
  - Added `docs/SHORTCUTS.md` with GNOME and KDE Plasma custom shortcut setup.
  - Documented `rayslash toggle` as the installed command and `cargo run -p rayslash -- toggle` as the development command.
  - Documented that Wayland global shortcuts should be bound by the desktop environment, not captured by the app.
  - Documented the resident IPC socket path at `$XDG_RUNTIME_DIR/rayslash.sock`.
- Completed Phase 4 calculator support:
  - Added a small safe recursive-descent parser/evaluator in `rayslash-core`.
  - Supported decimal numbers, addition, subtraction, multiplication, division, exponentiation with `^` and `**`, parentheses, unary signs, implicit multiplication, constants `pi` and `e`, and common one-argument functions.
  - Calculator detection requires a valid expression or math-like expression with a calculation signal such as an operator, superscript exponent, function call, or implicit multiplication, so ordinary app/project queries like `code`, `calculator`, and `pi` are not treated as math.
  - Superscript exponents such as `10²` and `10⁻²` are supported.
  - Linear equations in one variable named `x` are supported, such as `x + 10 / 2 = 8`, `2x + 4 = 10`, and `2(x + 3) = 10`.
  - Division by zero, incomplete expressions, unsupported characters, unknown identifiers, domain errors, and non-finite results become calculator error rows instead of disappearing.
  - Formula syntax errors such as `10++2` and `10+/2` become calculator error rows.
  - Nonlinear equations such as `x^2 = 4` and division by `x` are rejected with a clear calculator error.
  - Valid calculator results and calculator error rows are inserted as the first mixed search result above app/project results for math-like queries.
  - Calculator result titles show the computed result and subtitles use `Calculate: <expression>`.
  - Calculator error row titles show the error message and subtitles use `Calculate: <expression>`.
  - Enter on a calculator result copies the result to the system clipboard using the UI crate's direct `arboard` dependency and hides the launcher after a successful copy.
  - Enter on a calculator error keeps the launcher visible and updates the status line with the error message.
  - Calculator activation does not launch external commands.
  - Added core unit tests for expression detection, precedence, parentheses, superscript exponents, linear equations, invalid expressions, calculator error messages, normal query rejection, and mixed search ranking.
- Started Phase 6 basic install/packaging notes:
  - Added `docs/INSTALL.md` with development run commands and local user install instructions.
  - Documented `cargo install --path crates/rayslash-ui` as the local install command for the `rayslash` binary.
  - Documented `command -v rayslash` and `rayslash toggle` as PATH/install verification steps.
  - Updated `docs/SHORTCUTS.md` to point to install instructions before binding desktop shortcuts.
  - Made the UI crate's `rayslash` binary target explicit in `crates/rayslash-ui/Cargo.toml`.
- Continued Phase 6 Linux desktop integration and packaging notes:
  - Added `packaging/linux/dev.rayan6ms.rayslash.desktop`.
  - The desktop entry uses app ID `dev.rayan6ms.rayslash`, `Name=rayslash`, `Exec=rayslash toggle`, `Icon=dev.rayan6ms.rayslash`, `NoDisplay=true`, `StartupNotify=false`, and `StartupWMClass=dev.rayan6ms.rayslash`.
  - Added local and package icon install notes for `icons/rayslash-icon.svg` as `dev.rayan6ms.rayslash.svg`.
  - The core app ID constant now matches `dev.rayan6ms.rayslash`.
  - The UI selects the Slint backend before component construction, sets Slint's XDG app ID, and sets the window icon from `icons/rayslash-icon.svg` so desktop panels can match the installed desktop file and icon.
  - Documented local desktop entry install steps in `docs/INSTALL.md`.
  - Added `docs/PACKAGING.md` with Fedora notes, Arch/AUR notes, and AppImage deferral notes.
  - Recorded the desktop integration structure in `docs/ARCHITECTURE.md`.
- Continued Phase 6 UI icon polish:
  - Added the rayslash SVG icon before the launcher title using `icons/rayslash-icon.svg`.
  - Extended search results with display-only icon metadata while leaving typed activation data in `SearchResultKind`.
  - Added calculator, project folder, generic app, and placeholder row icon rendering in Slint.
  - Added best-effort desktop app icon resolution from parsed `.desktop` `Icon` fields.
  - The icon resolver supports absolute icon paths, extensionless absolute AppImage-style icon files, named icons in configured/common icon themes such as Papirus, theme inheritance from `index.theme`, hicolor fallbacks, and pixmaps.
  - The icon resolver prefers launcher-sized app icon assets around the rendered row size before scalable or very large assets when multiple named icon sizes exist.
  - Supported displayed app icon file types are SVG, PNG, JPG, and JPEG, subject to Slint loading support.
  - Added resolver/search model tests that do not require a real desktop icon theme.
- Continued Phase 6 real launcher UI/UX fixes:
  - Moved shortcut help out of the result list into a single subtle line below the search field.
  - Shortcut key labels are lighter than their descriptions in the hint line, and hint text segments use zero horizontal stretch so descriptions sit next to their keys.
  - Removed the default bottom "Type to filter..." status area and trimmed the idle bottom strip; action, calculator, and error feedback now reuse the top hint/status line when needed.
  - Added a bounded Slint `Flickable` result viewport so long app/project lists can scroll without pushing the hint line out of view.
  - Up/Down and Tab/Shift+Tab keep the selected result row visible by adjusting the result viewport when selection crosses the visible top or bottom edge.
  - IPC show/reset with an empty query leaves no result row selected, while a non-empty query with matches selects the first result so type-and-Enter remains fast.
  - Non-empty searches with real indexes but no matches show a single no-results row instead of a blank list.
  - Enter on a no-results row hides the launcher instead of showing preview text.
  - Query changes and IPC show/reset reset the result viewport to the top along with query, status, selection, results, and focus.
  - Hovering visible result rows selects/highlights them, including clipped partial rows, without mouse-driven viewport scrolling.
  - Clicking a result row activates it through the same path as Enter.
  - Ctrl+click activates through the same secondary path as Ctrl+Enter, so projects open in VS Code and apps continue launching normally.
  - Losing the winit window focus hides the launcher, covering normal click-outside behavior.
  - Enlarged and vertically aligned the rayslash header icon with the title while keeping the header design intact.
  - Real app icons and fallback row icons now render at the same 42px target size.
  - Added resolver tests for preferring launcher-sized hicolor app icons and checking theme-specific app icon directories.
  - Added Ctrl-held project row icon switching: fallback folder icons crossfade to a VS Code-styled fallback icon while Ctrl is held, without changing activation data.
  - Added opt-in `RAYSLASH_PROFILE=1` timing logs for startup stages, query refresh phases, model replacement, UI property updates, and settings-open app refresh.
- Added post-v1 planning documentation:
  - Reframed the roadmap around completed v1 phases and new public-readiness phases.
  - Added new unchecked tasks for settings/customization, learned ranking/aliases, refactoring/internal boundaries, packaging/Linux integration, project maturity, and optional providers.
  - Added launcher comparison notes in `docs/COMPARISON.md`.
  - Added testing strategy notes in `docs/TESTING.md`.
  - Expanded config docs with planned settings/config/state boundaries.
  - Expanded packaging docs with source-of-truth metadata, standards, and Flatpak evaluation notes.
  - Recorded decisions for the post-v1 phase shift, settings entry point, learned ranking state, and packaging metadata source of truth.
- Added Phase 9 refactoring planning documentation:
  - Inserted a dedicated refactoring/internal-boundaries phase before packaging.
  - Renumbered packaging to Phase 10, project maturity to Phase 11, and optional provider expansion to Phase 12.
  - Updated `docs/REFACTORING.md` with current large-file context and priorities for splitting `apps.rs`, `search.rs`, `main.rs`, and possibly `rayslash.slint` and `calc.rs`.
  - Moved crate-level integration tests and reusable fixtures into the Phase 9 task scope.
  - Recorded the decision to refactor before packaging in `docs/DECISIONS.md`.
- Started Phase 9 refactoring implementation:
  - Split core desktop app handling into `apps/app_discovery.rs`, `apps/desktop_entry.rs`, and `apps/icon_lookup.rs` while keeping `rayslash_core::apps` as the public API.
  - Split icon lookup internals into theme-discovery and path-resolution modules.
  - Split calculator internals into public API, parser, equation, and error modules.
  - Split core search into `search/result.rs`, `search/providers.rs`, and `search/matcher.rs` while keeping existing mixed search behavior and public API.
  - Split UI result item conversion, alternate opener visuals, settings diagnostics/parsing, settings callback wiring, activation handling, runtime search/profiling helpers, and show/hide helpers out of `crates/rayslash-ui/src/main.rs`.
  - Split reusable Slint pieces into `crates/rayslash-ui/ui/components.slint`, `ui/models.slint`, `ui/settings_panel.slint`, and `ui/result_list.slint` while preserving the generated `AppWindow`, `ResultItem`, and `AppChoiceItem` surface used by Rust.
  - Added `crates/rayslash-core/tests/` integration coverage and reusable fixtures for desktop entries, hicolor app icons, config shapes, project/app rows, and learned ranking.
  - Moved broad action, calculator, config, desktop-entry, icon lookup, ranking, project scanning/search, stable-result-ID, and mixed-search tests out of implementation files; small private helper tests remain inline.
  - Replaced the separate `icons/settings-gear.svg` asset with an inline Slint vector component.
- Started Phase 12 utility provider implementation:
  - Added default-enabled default-browser web search through the built-in `search` command and configurable `[[web_searches]]` with explicit keyword triggers and URL templates using `%s`.
  - Added default-enabled local unit conversion for common length, mass, volume, and temperature queries.
  - Added default-enabled currency conversion using Frankfurter pair rates with in-memory resident-process caching.
  - Added default-enabled `time in <place>` lookup using Open-Meteo place/timezone metadata with in-memory resident-process caching.
  - Changed default-browser web search to require `search` or the built-in Search pill instead of appearing for every query.
  - Added reboot, shutdown, logout, timer, and reminder command rows with dedicated icons and delayed action scheduling.
  - Migrated `ureq` from 2.12.1 to 3.3.0 for network-backed utility providers.
  - Added Settings toggles for Web, Units, Currency, and Time, plus editable alias and additional search-engine rows.
  - Added generated `apps.toml` state for ArcMenu-style `New` flair on newly discovered desktop apps until successful launch.
  - Added tests for web search templates, explicit default browser search rows, local unit conversion, conversion/calculator suppression, no-network same-currency conversion, time lookup parsing/formatting, timer/system action parsing, and app install state.

## Partially done

- Installable module migration:
  - Internal provider boundary, typed actions, virtual descriptors, version-1 `modules.toml`, and local Modules settings toggles exist.
  - Remote registry/package lifecycle, integrity, community authoring API/SDK, optional WASM host, extraction of seven official modules, and version-1 user migration remain.
  - Permanent GitHub repositories, public signing key, maintainers, support contacts, and packaging targets are owner prerequisites in [manual_migration.md](../manual_migration.md).

- Minimal launcher UI exists with:
  - Search input.
  - Mixed desktop app and project result list when either source has entries.
  - Provider-aware placeholder result list when no enabled apps or folders are found.
  - Single-line shortcut/status hint below the search input.
  - Scrollable results for longer app/project lists.
  - No selected row when the launcher opens empty, then first-result selection after typing a non-empty matching query.
  - Clamped keyboard selection with Up/Down and Tab/Shift+Tab.
  - Keyboard navigation keeps the selected result visible in the scroll viewport.
  - Hovering visible rows selects/highlights them, including clipped partial rows, without mouse-driven viewport scrolling.
  - Clicking rows activates them.
  - Ctrl+clicking rows uses the same secondary activation path as Ctrl+Enter.
  - Enter launches app results and hides the launcher after a successful spawn.
  - Enter opens project result folders with the system file manager and hides the launcher after a successful spawn.
  - Ctrl+Enter launches app results and hides the launcher after a successful spawn.
  - Ctrl+Enter opens project results in VS Code and hides the launcher after a successful spawn.
  - Project row fallback icons switch to a VS Code-styled icon while Ctrl is held.
  - Failed app and project actions keep the launcher visible and show a clear status message.
  - Calculator results and calculator errors appear above app/project results for math-like expressions; Enter on a result copies it to the clipboard and hides the launcher, while Enter on an error keeps the launcher visible and updates the status line.
  - Placeholder activation previews only and does not hide the launcher.
  - Escape-to-hide callback.
  - Click-outside hiding through the winit `Focused(false)` window event.
- Phase 5 resident/toggle foundation exists with local Unix socket IPC and no in-app global shortcut capture.
- Phase 1 UI foundation exists with visual polish, keyboard navigation, and placeholder activation.
- Placeholder result items remain as provider-aware fallback rows when no enabled apps, folders, or aliases are available:
  - Open applications.
  - Find folders.
  - Calculate.
  - Use aliases.
## Not started

- Phase 11 project maturity implementation.
- Remaining Phase 12 optional providers such as snippets, clipboard history, scripts, and window switching.
- AppImage build.
- Full freedesktop icon-theme lookup.

## Current known issues

- `docs/CLEANUP_AUDIT.md` is the current source for cleanup findings that do not fit neatly into feature phases.
- `docs/UI_VERIFICATION.md` is the current manual Slint/UI verification checklist for result scrolling, hover selection, settings autosave, focus loss, and icon rendering.
- Cleanup passes on 2026-07-01 and 2026-07-02 verified `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.
- Settings autosave rewrites known config fields atomically and does not preserve unknown fields, comments, ordering, or formatting in-place. It creates a timestamped backup before replacing an existing `config.toml`, and save attempts are blocked after startup config read/parse failures.
- The app now uses a frameless Slint window via `no-frame: true`; this is toolkit-supported and does not use compositor-specific APIs.
- The corner leak was addressed using Slint-level transparent backgrounds and rounded-panel clipping, not compositor-specific APIs. If a desktop/backend does not honor transparent windows, square corners may remain a platform limitation to document rather than hack around.
- The window does not yet implement custom screen positioning.
- Frameless windows may have desktop-environment-specific move/resize tradeoffs; keep the implementation simple unless a real issue appears.
- The resident IPC socket is Linux/Unix-specific. Cross-platform IPC is not planned until a real target needs it.
- Visibility state is tracked in the UI crate because Slint's portable window API exposes `show` and `hide`, but not a simple cross-backend visible query.
- Project scanning is synchronous and shallow by design; it only scans immediate children of configured roots.
- Desktop app scanning is synchronous at startup and recursively scans standard application directories.
- Local install is documented with Cargo and a user desktop entry copy step. Fedora RPM packaging, Arch/AUR packaging, and a Flatpak prototype manifest exist; AppImage remains deferred.
- Desktop panel icon matching uses Slint's XDG app ID, `Window.icon`, and `StartupWMClass`, and still depends on installing/refreshing the matching desktop entry and hicolor icon on the user's desktop for panels that ignore per-window icons.
- Desktop `Exec` parsing is conservative: it handles whitespace, double quotes, backslash escapes, and field-code removal, but it does not implement full field-code expansion with filenames, URLs, icons, translated names, or desktop-file paths. Localized labels, keywords, desktop actions, and `DBusActivatable` are parsed.
- Desktop app icon lookup is best-effort only. It reads common GNOME/GTK/KDE icon theme settings when available, checks common installed themes such as Papirus, follows simple `Inherits=` values from `index.theme`, prefers launcher-sized app assets when multiple named sizes exist, checks common symbolic/app scalable layouts, and has a conservative reverse-DNS suffix fallback for WPS 2019-style icon names. It still does not implement the full freedesktop icon-theme algorithm, all icon-theme metadata, symbolic recoloring, scaled directories, or guaranteed current-theme detection on every desktop.
- The UI uses provider-aware placeholder results only when no enabled desktop apps, folders, or aliases are available and there is no calculator row.
- When real entries exist but the query matches none of them, the UI shows a single provider-aware no-results placeholder row.
- Calculator result clipboard writes use `arboard`; if clipboard access fails, the launcher stays visible and shows a status message.
- Performance profiling is opt-in through `RAYSLASH_PROFILE=1`, the ignored `crates/rayslash-core/tests/performance.rs` probe prints synthetic search timings, and [PERFORMANCE.md](PERFORMANCE.md) records comparable history; no background indexer or on-disk cache exists yet.

## Next recommended steps

- Continue Phase 11 maturity work by adding a manual distro/session verification matrix and release notes/changelog process.
- Manually verify Phase 5 resident/toggle behavior on a real desktop session:
  - Start with `cargo run -p rayslash`.
  - Run `cargo run -p rayslash -- toggle` from another terminal to hide/show the resident window.
  - Run `cargo run -p rayslash` from another terminal while the resident process is hidden and confirm it shows the launcher.
  - Confirm showing through IPC resets the query and focuses the search field.
- Run `RAYSLASH_PROFILE=1 cargo run -p rayslash` on a real desktop session and compare startup/query/settings-open timings, plus `cargo test -p rayslash-core --test performance -- --ignored --nocapture` for synthetic core search timing, before deciding whether app discovery, icon loading, result conversion, model replacement, or indexing needs optimization.
- Manually run `cargo install --path crates/rayslash-ui`, confirm `command -v rayslash`, then bind a desktop shortcut to `rayslash toggle` using `docs/SHORTCUTS.md` and verify `Super+\` toggles the resident launcher.
- Manually verify Phase 3 app launching with `cargo run -p rayslash` on a real desktop session, including that launched app warnings no longer appear in the rayslash terminal.
- Keep AppImage builds, global shortcut capture/registration, full plugin marketplace work, clipboard history, snippets, script providers, and full freedesktop icon-theme lookup deferred until the relevant post-v1 phase is active.

## Important commands

- Build: `cargo build`
- Run UI: `cargo run -p rayslash`
- Run toggle during development: `cargo run -p rayslash -- toggle`
- Local install: `cargo install --path crates/rayslash-ui`
- Verify installed binary: `command -v rayslash`
- Toggle installed app: `rayslash toggle`
- Install local desktop entry: `cp packaging/linux/dev.rayan6ms.rayslash.desktop ~/.local/share/applications/`
- Install local icon: `cp icons/rayslash-icon.svg ~/.local/share/icons/hicolor/scalable/apps/dev.rayan6ms.rayslash.svg`
- Refresh local icon cache: `gtk-update-icon-cache ~/.local/share/icons/hicolor`
- Refresh local desktop database: `update-desktop-database ~/.local/share/applications`
- Test: `cargo test`
- Format check: `cargo fmt --check`
- Format: `cargo fmt`
- Clippy: `cargo clippy --workspace --all-targets -- -D warnings`
- Metadata validation: `packaging/validate-metadata.sh`
- Desktop entry validation: `desktop-file-validate packaging/linux/dev.rayan6ms.rayslash.desktop`
- AppStream validation: `appstreamcli validate --no-net packaging/linux/dev.rayan6ms.rayslash.metainfo.xml`

Fedora setup used during Phase 0:

```sh
sudo dnf install gcc
sudo dnf install 'pkgconfig(fontconfig)'
