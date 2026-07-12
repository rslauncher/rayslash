# Decisions

## 2026-07-11 - Preserve incomplete search engines and use cached favicons consistently

Decision: Treat incomplete additional search-engine rows as inactive drafts, persist every field on focus loss, show an inline amber warning until the row is valid, and use a successfully cached favicon in both the settings card and matching result row.
Context: Closing the launcher on focus loss made it impractical to copy several fields from another window, invalid partial rows were discarded, and fetched favicons only affected the active-search pill accent while settings and result rows kept fallback badges.
Reasoning: A draft is user-authored configuration but not an active provider entry. Retaining it in `config.toml` avoids a second hidden draft store, while provider validation prevents incomplete rows from matching. Reusing the cached PNG path keeps favicon decoding and display local after the explicit engine configuration is saved.
Consequences: Blank or incomplete rows survive restart and ordinary Settings autosave, display a warning icon, and never trigger search until name, keyword, and a URL containing `%s` are valid. Search-engine switches now use the same crisp geometry as general feature switches. Official module cards use provider-specific glyphs matching result rows; bundled module packaging is unchanged.

## 2026-07-10 - Unify default web search, expand country timezones, and fuzzy-match system actions

Decision: Represent the default web path as a permanent first `Web Search` URL template, identify countries with one debounced geocoding request and expand their distinct current offsets locally, and make system actions fuzzy searchable alongside apps.
Context: Browser-family command handling was inconsistent, partial time input could start several remote requests, country-center time was insufficient for multi-zone countries, and system actions appeared only after exact parsing.
Reasoning: A normal `%s` template gives every browser the same predictable behavior and lets the user change the engine in one place. The installed IANA timezone database can enumerate and calculate country offsets without N network calls. Fuzzy system action rows match the rest of the launcher and can use the existing local ranking state, while command-construction tests avoid executing destructive operations.
Consequences: `Web Search` defaults to Google, remains first, can be disabled, and cannot be removed. Country time queries suppress unrelated results and group regions by current offset. Bare system actions run immediately, explicit delays remain supported, and aliases include `turn off` and `reset`. The older decisions below document the superseded implementation history.

## 2026-07-08 - Default web search is command-gated and utilities add timed actions

Decision: Change default-browser web search to require the built-in `search` command or active `Search` pill, add reboot/shutdown/logout plus timer/reminder rows as typed utility actions, and migrate network utility providers to `ureq` 3.3.0.
Context: Always showing default web search prevented `No results` from appearing and passing bare text to browser command lines could be interpreted as a URL such as `http://manhattan/`. The launcher also needs ArcMenu-style power actions and lightweight reminders without adding shell aliases.
Reasoning: A command-gated default search keeps ordinary unmatched queries local while preserving a fast web path. Firefox-like browsers expose `--search`, which uses the browser's configured search engine; other browser families do not expose a common default-engine search CLI, so desktop/default-browser behavior remains best effort. Typed utility actions allow strict parsing, short error rows, dedicated icons, and delayed scheduling without shell command strings.
Consequences: `search manhattan` or `search` followed by Space/Tab creates the default browser search row. Additional configured search engines still use their own keywords. Timer messages with multiple time-like values show a short quote-the-message hint. Reboot, shutdown, and logout use system tools after the parsed delay, defaulting to 30 seconds.

## 2026-07-08 - Web search uses default browser search plus configurable additional engines

Decision: Add a default-browser web search row, initially for non-empty queries, and change additional search engines to use `keyword`, `url` with `%s`, and per-engine `enabled` config. Add settings fields for aliases and additional search engines, and add a search-box keyword pill when a configured engine keyword is followed by Space or Tab. Refined later on 2026-07-08 so the default-browser row requires the built-in `search` command and uses browser-specific search mode where available.
Context: Web search should not hardcode a search provider for normal searches. Users also need Zen Browser-style additional engines such as YouTube search with a keyword trigger and visible active keyword state.
Reasoning: The default-browser path should let the browser choose its configured search engine when the browser exposes a command-line search mode. Appending the default web search row only after an explicit command keeps app/folder launching predictable. Additional engines still need explicit URL templates because sites such as YouTube expose specific search URLs. Keeping the config TOML-backed preserves manual editing while settings rows make add/edit/delete/toggle available in the UI.
Consequences: `[[web_searches]]` now serializes as `name`, `keyword`, `url`, and `enabled`; legacy `query` and `url_template` still load. Additional engines can be triggered by typing `keyword terms` or by pressing Space/Tab after the keyword to activate a pill. The default browser search row is triggered by `search terms` or the built-in `Search` pill. The row icon is a local keyword badge rather than a fetched remote favicon.

## 2026-07-08 - Explicit utility providers default on and time lookup uses Open-Meteo

Decision: Enable web search, local unit conversion, currency conversion, and time lookup providers by default. Add explicit `time in <place>` lookups using Open-Meteo Geocoding plus Open-Meteo Forecast `timezone=auto` metadata, caching resolved place/timezone metadata in the resident process.
Context: The default launcher should expose the small utility actions without requiring a settings visit. The time lookup needs country/city/place resolution, including country queries such as `time in Argentina`, without requiring an API key.
Reasoning: Unit and currency conversion require conversion syntax, time lookup requires `time in`, and web search activation is explicit because the user must select the search row or use a configured engine trigger. Open-Meteo is open source, offers free non-commercial no-key access, returns location coordinates and IANA timezone metadata, and can resolve coordinates to a local timezone/UTC offset. Currency remains on Frankfurter pair rates.
Consequences: `providers.web_search`, `providers.unit_conversion`, `providers.currency_conversion`, and `providers.time_lookup` default to true. Time lookup sends the typed place name and resolved coordinates to Open-Meteo, reports an in-launcher error row when lookup fails, and can be disabled in Settings.

## 2026-07-07 - Phase 12 utilities stay opt-in and currency uses Frankfurter

Decision: Add Phase 12 web search templates, local unit conversion, and currency conversion as opt-in providers. Use Frankfurter's public v2 pair-rate API for currency conversion, cache fetched rates in the resident process, and keep web search templates as manual TOML entries for now. Superseded on 2026-07-08 for default-enabled utility providers.
Context: Optional provider expansion should add useful commands without making default search noisier or sending user-entered queries to network services unexpectedly. Currency conversion requires a live rate source, while unit conversion can remain fully local and deterministic.
Reasoning: Web search sends the user's search terms to the configured target, so it should be explicitly enabled and configured. Unit conversion is local, but keeping all new Phase 12 providers default-off preserves existing search behavior. Frankfurter currently provides a no-key, open source public API with compact pair-rate responses and self-hosting as an escape hatch; rayslash sends only base and quote currency codes and applies the amount locally.
Consequences: Initially, `providers.web_search`, `providers.unit_conversion`, and `providers.currency_conversion` defaulted to false. As of 2026-07-08, these utility providers default to true. Later on 2026-07-08, settings gained editable alias and additional search-engine rows. Currency conversion depends on network availability and reports an in-launcher error row when a rate cannot be fetched.

## 2026-07-07 - New app flair uses generated app install state

Decision: Track desktop app IDs in generated XDG state and display a `New` flair after app names for app IDs discovered after the initial baseline until they are successfully launched from rayslash.
Context: The user requested ArcMenu-style new-app flair for newly installed applications that have not been selected yet. Desktop app discovery already runs at startup and on settings-open refresh, so it can detect new desktop IDs without adding a background watcher.
Reasoning: First run should not mark every existing app as new. A small `apps.toml` state file lets rayslash establish a baseline, mark later app IDs as new, and clear the marker independently of learned ranking. Clearing on successful launch avoids removing the marker just because keyboard or pointer hover happened to select the row.
Consequences: Generated app install state lives at `~/.local/state/rayslash/apps.toml`. Newly discovered apps are marked during startup or settings-open app refresh. Successful app launches clear the new marker and save state.

## 2026-07-03 - Complete Phase 10 packaging metadata before deeper package release work

Decision: Add a source-of-truth packaging inventory, AppStream/metainfo metadata, metadata validation script, GitHub Actions CI, Fedora spec, Arch/AUR `PKGBUILD`, and a Flatpak prototype manifest. Keep AppImage deferred with explicit revisit criteria.
Context: Phase 10 required public Linux identity values and install outputs to stop drifting across desktop files, icons, package formats, docs, and code.
Reasoning: The inventory and validation script catch identity regressions cheaply. AppStream and desktop-entry validation are the common base for Flatpak and distro-native packages. Flatpak remains the first public target to evaluate because broad distribution is attractive, but rayslash's host app discovery and app launching model still needs sandbox testing before committing to it as the release path. Fedora and Arch packaging can already use the normal install layout directly.
Consequences: CI now runs Rust checks and metadata validation. Fedora and Arch package files are present. AppImage is intentionally not implemented until shortcut invocation, desktop integration, and update expectations are clearer.

## 2026-07-03 - Light mode and result caps become public appearance behavior

Decision: Add `light` as a supported appearance theme, change the default max result count from 50 to 36, and show a separate non-selectable tip when more results exist than the configured cap.
Context: The settings panel had grown enough that scrolling, readable toggle text, and clearer exclusive appearance controls became user-facing problems. The default result list was also longer than needed for a compact launcher.
Reasoning: Light mode should use the existing settings-backed theme field instead of a separate UI path. A 36-result default keeps broad empty-query discovery useful while the separate tip makes the cap discoverable without adding a selectable pseudo-result. Segmented controls communicate exclusive theme/density choices better than separate button blocks.
Consequences: Config remains backward compatible for existing `max_results` values and `dark`/`dim` themes. New default configs use `max_results = 36`, and settings can choose `dark`, `dim`, or `light`.

## 2026-07-03 - Desktop compatibility expands without shell execution

Decision: Parse localized desktop labels, keywords, desktop actions, and `DBusActivatable`, search the extra app metadata, and launch DBus-activatable entries through `gio launch <desktop-id>`. Improve local icon lookup with additional app icon directory shapes and a conservative reverse-DNS suffix fallback for WPS-style icon names.
Context: Phase 10 packaging raised the cost of partial desktop standards behavior, and WPS 2019 exposed an icon naming mismatch between the desktop entry and the installed Papirus icon.
Reasoning: These additions improve common freedesktop compatibility while preserving direct process spawning and the current primary-result UI. Desktop actions are parsed for compatibility and future action UI work, not exposed as separate rows yet. The icon suffix fallback runs only after exact lookup fails and is constrained to long reverse-DNS suffixes.
Consequences: Apps can match localized names and keywords, DBus-activatable apps can launch without a direct `Exec`, and WPS 2019-style icon names resolve when a theme provides the corresponding suffix icon. Full freedesktop icon-theme lookup remains a later possible improvement.

## 2026-07-02 - Cleanup backlog closes with shared UI refresh paths

Decision: Keep settings callback registration as event wiring, but move settings-field validation into a shared config builder and move result/settings UI refresh into shared runtime helpers. Add a manual Slint UI verification checklist instead of introducing a screenshot automation dependency in this pass.
Context: The cleanup audit still had duplicated settings save/apply/refresh behavior and no explicit UI verification workflow for layout and pointer/focus behavior.
Reasoning: Shared helpers reduce callback-specific drift for query refresh, show/reset, settings save, and ranking clear without changing the resident UI architecture. The remaining Slint risks require real desktop interaction today, so a concrete checklist is more useful than brittle headless coverage.
Consequences: The cleanup audit backlog is complete. Future UI automation can build from [UI_VERIFICATION.md](UI_VERIFICATION.md) if Slint testing or screenshot tooling becomes worth the dependency.

## 2026-07-02 - Settings saves protect existing config files

Decision: Keep settings saves as normalized full-file TOML serialization, but create a timestamped backup of an existing `config.toml` before replacement and block settings saves when startup config loading failed and the UI is running on fallback defaults.
Context: Manual TOML editing is supported, while the settings UI only knows the current public schema. A full settings save drops comments, ordering, formatting, and unknown fields, and a startup parse error could otherwise be followed by saving defaults over a recoverable file.
Reasoning: Backups give users a recoverable copy of hand-authored config without introducing a partial TOML editing layer yet. Blocking saves after config read/parse failures prevents fallback defaults from silently replacing the user's broken file.
Consequences: Settings saves still normalize and rewrite known fields, but an existing file is preserved beside the replacement. Comment-preserving structured config edits remain a possible later improvement.

## 2026-07-02 - Learned ranking state is bounded

Decision: Prune learned ranking after successful learned app/folder launches by removing entries missing from the current app/folder index, removing entries older than 180 days, capping each entry to 64 query prefixes, and capping the state to 1000 entries by recency.
Context: Ranking state was append-only and could retain removed apps, deleted folders, and unbounded query-prefix maps.
Reasoning: Ranking is generated local state, not archival data. Pruning during learned launches keeps the file bounded without adding startup I/O or background maintenance. The caps are conservative enough for normal launcher use while preventing pathological growth.
Consequences: Removed apps/folders age out as soon as ranking is next updated. Very old rarely used entries no longer influence search. If a removed app or folder returns later, it can be learned again.

## 2026-07-02 - Alternate opener command lines are parsed without a shell

Decision: Keep the public `actions.alternate_folder_opener_command` key but parse it as a small command line into direct program/argument values. Append the selected folder path as the final argument for normal commands, and keep `xdg-terminal-exec` as a special terminal opener that runs with the selected folder as the working directory and no implicit folder argument.
Context: The settings UI and docs called this field a command, but the implementation previously treated the whole value as one executable name. Values such as `code --reuse-window` were therefore broken.
Reasoning: Supporting simple arguments matches user expectations while preserving direct process spawning and avoiding shell execution. Keeping the existing key avoids a config migration for current users.
Consequences: Existing values such as `code`, `codium`, and `xdg-terminal-exec` keep working. Values with arguments such as `code --reuse-window` now work. A richer structured action schema can still replace this later if aliases or script-like actions need stricter policy.

## 2026-07-02 - Mouse hover selection should not auto-scroll rows

Decision: Result-row hover and pointer-move selection can select visible rows, including clipped partial rows at the viewport edge, but mouse selection should not adjust the result viewport.
Context: Manual testing showed that rejecting clipped rows made pointer selection feel broken at the top and bottom of the list. The important constraint is preventing mouse hover from causing jumpy scroll movement.
Reasoning: Keyboard navigation should keep the selected row fully visible because the keyboard has no pointer position. Mouse selection already has an explicit pointer target, so selecting a partially visible row is reasonable as long as the viewport does not move unexpectedly.
Consequences: Keyboard navigation still scrolls selected rows into view. Mouse hover can select clipped visible rows, while mouse-driven selection does not scroll the result viewport.

## 2026-07-02 - Cleanup saves and fallback paths stay conservative

Decision: Use same-directory temporary-file-and-rename writes for `config.toml` and `ranking.toml`, normalize relative folder sources to absolute paths during config normalization, and use a user-specific temp subdirectory for the IPC socket fallback when `XDG_RUNTIME_DIR` is unavailable.
Context: The cleanup audit identified direct config/state writes, relative folder paths, and a shared `/tmp/rayslash.sock` fallback as public-readiness risks.
Reasoning: Atomic replacement reduces the chance of truncated config or ranking files without changing the public schema. Absolute folder sources keep scan paths, stable IDs, and launch actions independent of the resident process working directory. A user-specific temp socket fallback avoids cross-user collisions in unusual environments while preserving tests and basic non-desktop operation.
Consequences: Settings saves may rewrite folder sources in normalized absolute form. Config and ranking saves are safer against partial writes.

## 2026-07-01 - Finish the second Phase 9 cleanup pass

Decision: Extend the initial Phase 9 refactor by splitting calculator internals, splitting icon lookup internals, splitting the large Slint window file into settings/result components, moving broad public behavior tests into crate-level integration tests, removing the unused `Action::OpenPlaceholder` enum, trimming activation debug prints, and replacing stale phase-era placeholder copy.
Context: After the first Phase 9 pass, several files were still large because public behavior tests and separable helper code remained inline. The calculator, icon lookup, and main Slint file were still the largest areas, and the placeholder fallback rows still contained old "will land in Phase" text.
Reasoning: The calculator public API can stay small while parser, equation solving, and error messages live in focused modules. Icon lookup has a natural split between theme directory discovery and path resolution. The Slint window can keep a stable Rust-facing `AppWindow` while moving settings and result rendering to focused components. Moving public behavior tests to integration fixtures makes source files smaller and tests the same API the UI depends on. Keeping placeholder rows but updating their copy preserves fallback behavior without presenting obsolete implementation phases to users.
Alternatives considered: Leave calculator and icon lookup intact until packaging, remove placeholder rows entirely, or introduce a broader provider framework as part of the cleanup.
Consequences: `calc.rs` is now a small public module root, `apps/icon_lookup.rs` is a small resolver root, `rayslash.slint` is a smaller composition file, and broad core behavior tests now live under `crates/rayslash-core/tests/`. The current UI behavior, config/state formats, ranking behavior, and launch semantics are preserved.

## 2026-07-01 - Finish Phase 9 with conservative splits

Decision: Complete the initial Phase 9 refactoring pass by splitting core app discovery, core search orchestration, UI callback/helper responsibilities, and low-risk Slint components while keeping the calculator in one file for that pass.
Context: The app is approaching packaging work, and the largest files were mixing behavior that future Linux integration work will need to test independently. The calculator is also large, but its tokenizer/parser/evaluator/equation behavior currently forms one cohesive feature with inline tests that document the grammar clearly.
Reasoning: The app and search splits create useful boundaries without changing public APIs or launch behavior. UI helper modules reduce `main.rs` to startup and high-level wiring. Moving only the reusable Slint search box, settings toggle, and gear icon keeps the build straightforward and preserves the generated Rust surface around `AppWindow`. Keeping `calc.rs` together avoids a mechanical split that would scatter tightly coupled parser tests without improving packaging readiness.
Alternatives considered: Split the calculator immediately into tokenizer/parser/evaluator/equation files, split the full Slint file into many component files, or leave broad mixed-search tests inline.
Consequences: `crates/rayslash-core/tests/` now owns broad desktop app, icon fixture, mixed search, provider toggle, and learned-ranking regression coverage. Small parser/helper tests remain inline. Future packaging work can build on smaller app/search/UI modules, while a calculator split remains a later option if calculator changes become blocked by file size.

## 2026-07-01 - Move refactoring before packaging

Decision: Insert Phase 9 as a dedicated refactoring and internal-boundaries phase before packaging, then renumber packaging to Phase 10, project maturity to Phase 11, and optional provider expansion to Phase 12.
Context: The first settings and learned-ranking foundations are implemented, but the current code still has several large mixed-responsibility files: UI `main.rs`, the Slint UI file, core `apps.rs`, core `search.rs`, and core `calc.rs`. Packaging will need more reliable desktop-entry handling, icon behavior, metadata validation, and tests, which are harder to change safely while those concerns are still concentrated in a few files.
Reasoning: Packaging should be built on stable internal boundaries and reusable fixtures. Splitting desktop-entry parsing, icon lookup, search result construction, settings wiring, activation, diagnostics, and result item conversion before packaging reduces the chance that packaging work becomes tangled with broad cleanup. This is still a conservative refactor phase: preserve config/state compatibility, launch semantics, ranking behavior, and the current UI flow.
Alternatives considered: Keep refactoring under the old project-maturity phase after packaging, start package implementation first, or do a broader rewrite/provider framework before public packaging.
Consequences: [ROADMAP.md](ROADMAP.md) and [TASKS.md](TASKS.md) now place refactoring before packaging. [REFACTORING.md](REFACTORING.md) has Phase 9 priorities and current large-file context. [TESTING.md](TESTING.md) treats integration tests and reusable fixtures as Phase 9 prerequisites instead of post-packaging polish.

## 2026-06-30 - Shift from completed v1 phases to public-readiness phases

Decision: Treat Phases 0 through 6 as the completed v1 baseline and plan future work as public-readiness phases: settings/customization, learned ranking/aliases, packaging/Linux integration, project maturity, and optional providers. This phase list was later amended on 2026-07-01 to insert refactoring/internal boundaries before packaging.
Context: The original task list is now effectively complete, and the app is already useful when installed from source for the current user. The next problem is not one missing launcher feature; it is making the app reliable, configurable, and understandable for users on other machines and distributions.
Reasoning: A public app needs less hardcoded behavior, stronger standards alignment, clearer config/state boundaries, packaging metadata, and contributor-friendly tests. Planning these as phases prevents individual features such as a settings button or learned ranking from landing without the underlying config, privacy, testing, and packaging story.
Alternatives considered: Continue appending polish tasks to Phase 6, jump directly into package implementation, or add unrelated feature ideas without a comparison pass.
Consequences: [ROADMAP.md](ROADMAP.md) now has post-v1 phases, [TASKS.md](TASKS.md) has new unchecked tasks, [COMPARISON.md](COMPARISON.md) records launcher/product references, and [TESTING.md](TESTING.md) records the maturity plan.

## 2026-06-30 - Settings button should replace the header preview label

Decision: The next settings entry point should likely replace the right-aligned `preview` text in the launcher header with a settings button on the same line as the icon and title.
Context: The current UI header has a left icon/title group and a right `preview` label. The app no longer needs a preview marker as a primary signal, and public customization needs an obvious but unobtrusive entry point.
Reasoning: A header settings button uses already available space without adding another row or making the launcher feel like a landing page. It also gives users a discoverable path for folder sources, provider toggles, alternate folder opener command, theme/density, learned ranking controls, and diagnostics.
Alternatives considered: Keep settings config-file-only, add a command row in search results, place settings in a bottom status bar, or add a visible app-menu desktop entry just for settings.
Consequences: The roadmap treats settings as Phase 7 work. The implementation should still preserve manual TOML editing and avoid hiding important config behind UI-only state.

## 2026-06-30 - First settings surface stays inside the launcher panel

Decision: Implement the first public settings surface as an in-launcher panel opened by the header Settings button.
Context: Phase 7 needs practical customization without adding another window lifecycle, new IPC commands, or a broader preferences framework.
Reasoning: The current Slint app is a compact single-window resident process. An in-launcher panel keeps the UI crate thin, reuses existing state, and lets settings save directly to the same TOML config that manual users edit. The first surface is intentionally limited to folder sources, the optional alternate folder opener, current provider toggles, result count, and diagnostics for config/state/socket paths and discovered app/folder counts.
Alternatives considered: A second normal preferences window, a separate dialog, settings exposed only as a search result, or config-file-only customization.
Consequences: Saving settings writes the known public TOML fields, rescans folder sources, and refreshes search results without restarting the resident process. The previous `project_roots`, `providers.projects`, and `actions.project_editor_command` keys still load for compatibility, but saving writes `folder_sources`, `providers.folders`, and `actions.alternate_folder_opener_command`. Theme/density, learned ranking, aliases, optional providers, packaging, and icon lookup diagnostics remain deferred to later work.

## 2026-06-30 - Learned ranking belongs in local state, not config

Decision: Usage-based ranking should persist under the XDG state directory and remain separate from `config.toml`.
Context: Mature launchers commonly learn from selected results. The user specifically expects ArcMenu-like behavior where a frequently used app can outrank an alphabetically earlier app for the same query, and Ulauncher documents previous-choice learning.
Reasoning: Learned ranking is generated local state, not user-authored preference. Keeping it separate lets users reset history without losing folder sources, aliases, theme choices, or provider settings. A bounded boost can improve frequent choices while preserving deterministic fuzzy matching, calculator-first behavior, and stable tie-breaks.
Alternatives considered: Store use counts in config, reorder results purely by recency, add manual favorites only, or keep deterministic fuzzy/alphabetical ranking forever.
Consequences: Phase 8 should define stable result IDs, state file format, decay/boost formula, clear-history behavior, disabled-learning behavior, and tests before wiring ranking into the UI.

## 2026-06-30 - First learned ranking formula stays prefix-gated and bounded

Decision: Implement the Phase 8 learned ranking foundation with a small `ranking.toml` state file, app/folder-only learning, and a prefix-gated boost capped at 20 fuzzy-score points.
Context: The goal is to make repeated launches feel more personal without making search surprising. Calculator rows must remain first for valid math-like queries, disabled providers must stay disabled, and strong textual matches should not disappear behind history.
Reasoning: Launch counts and query-prefix counts are enough for the first useful signal. A result only spends learned boost when its title starts with the current query, which keeps the first pass conservative and avoids promoting weaker in-string matches over clear prefix matches. The boost is deterministic for a given state file and query: no wall-clock decay or randomization is used during ranking. `last_launched_unix` is stored for future maintenance or diagnostics but does not currently change ordering.
Alternatives considered: Store learning in config, use recency-heavy ordering, boost all fuzzy matches for any query, add manual favorites first, or implement aliases and ranking in one larger pass.
Consequences: Successful app and folder launches record `launch_count`, `last_launched_unix`, and query prefixes in `~/.local/state/rayslash/ranking.toml`. The settings panel can disable learning and clear history. Aliases, quick links, and broader provider/plugin changes remain deferred.

## 2026-06-30 - Result viewport height comes from layout stretch

Decision: Let the Slint result `Flickable` take the remaining launcher height from the layout instead of setting its height to `self.preferred-height`.
Context: Results could still be selected and activated with Enter while no rows were visible, which pointed to a UI layout problem rather than missing search results.
Reasoning: `self.preferred-height` on the result viewport can collapse the visible area even when the model contains rows. Keeping the viewport mounted but using layout stretch preserves the reset/scroll functions and lets the row list render in the available panel space.
Alternatives considered: Change search result generation, increase the window height, or move result scrolling into Rust.
Consequences: The result list remains visually bounded and scrollable, while activation behavior continues to use the existing selected result model. The header settings button uses an inline Slint vector gear, avoiding a separate small SVG asset.

## 2026-06-30 - Alternate folder opener defaults to a terminal-style command

Decision: Default the alternate folder opener to `xdg-terminal-exec`, add an installed-app picker for the command field, and use the selected app icon for the Ctrl-held folder-row preview when available.
Context: VS Code is useful for the original developer workflow, but it is not installed on most Linux desktops and a hardcoded `VS` preview is misleading when the command is configurable.
Reasoning: `xdg-terminal-exec` is a better neutral default than `code` for public use. The app picker reuses already-discovered desktop entries and stays inside the current settings architecture instead of trying to call a file manager's private "Open With" sheet. The row preview can match a configured command to a discovered desktop app icon and choose a small sampled/tinted background from simple SVG color extraction, falling back to a deterministic command color and short label when no icon is available.
Alternatives considered: Keep `code` as the default, add a full action-template schema immediately, call a desktop-specific Open With dialog, or add a plugin/provider system for openers.
Consequences: Existing user configs that already set `alternate_folder_opener_command = "code"` continue to load. New default configs use `xdg-terminal-exec`; that command is launched with the selected folder as the working directory and no implicit folder argument. Most other configured opener command lines receive configured arguments followed by the folder path.

## 2026-06-30 - Treat packaging metadata as a source-of-truth problem

Decision: Before adding multiple package formats, define and validate a packaging inventory for binary name, app ID, desktop entry, icon name, AppStream/metainfo ID, config/state/cache paths, and runtime socket path.
Context: Current packaging notes are enough for local use and future RPM/AUR/AppImage work, but public distribution across distros will fail if identity values and install paths drift between code, desktop files, icons, metadata, and docs.
Reasoning: Linux desktop integration depends on several files agreeing with each other. Standards-based metadata also makes it easier to choose between Flatpak, Fedora RPM, Arch/AUR, and AppImage without copying assumptions. A validation step catches packaging regressions early.
Alternatives considered: Implement Fedora, AUR, and AppImage files independently right away, rely on manual docs, or keep packaging source-only.
Consequences: Packaging now starts with metadata inventory and validation. Flatpak is called out as the strongest broad-distribution candidate to evaluate first, but the docs leave room to choose distro-native packages if a prototype shows Flatpak is not a good fit for host app discovery and launching.

## 2026-06-30 - Keep eager startup discovery with opt-in profiling

Decision: Keep eager startup discovery for configured projects and installed desktop apps, and add opt-in timing output behind `RAYSLASH_PROFILE=1`.
Context: Search currently feels instant because apps and projects are discovered once in the resident process and searched from memory on every keystroke. There is still uncertainty about whether startup cost comes from project scanning, recursive `.desktop` discovery, icon path resolution, initial result item construction, or per-query matching.
Reasoning: Project scanning is shallow and desktop app lists are usually small enough for in-memory fuzzy search. Changing to lazy loading or background indexing before measuring could make the first query or first visible result feel worse, which would work against the launcher's main interaction goal. Lightweight stage timings give enough signal to optimize the actual bottleneck without making normal shortcut launches noisy.
Alternatives considered: Lazy-loading all apps/projects on first query, indexing in a background worker immediately, caching discovered entries on disk, or always printing timing logs.
Consequences: Normal launches stay quiet and keep the current fast search behavior. Developers can run `RAYSLASH_PROFILE=1 rayslash` to see startup, query phase, model replacement, and UI update timings before deciding whether app discovery, icon handling, result conversion, Slint model updates, or a future async/cache strategy needs work. Larger synthetic search timings are available through the ignored `crates/rayslash-core/tests/performance.rs` probe.

## 2026-06-30 - Ctrl-held project icon previews VS Code action

Decision: While Ctrl is held, project rows switch their fallback folder icon to a VS Code-styled fallback icon with a short background/opacity transition.
Context: Ctrl+Enter and Ctrl+click already open projects with `code <folder>`, but project rows previously kept the same folder icon while the modifier changed the action.
Reasoning: The icon state should communicate the active secondary action without changing result ranking, selection, or activation data. Keeping this in Slint as display-only modifier state avoids coupling core search results to transient keyboard state.
Alternatives considered: Changing subtitles while Ctrl is held, adding a broader animation pass, or pushing modifier-aware icons into `rayslash-core`.
Consequences: The row icon now previews the VS Code action while Ctrl is held. The activation path remains unchanged: Enter opens the folder, Ctrl+Enter opens VS Code, Ctrl+click uses the same secondary path, and app rows still launch normally.

## 2026-06-30 - Bounded launcher result viewport

Decision: Keep shortcut help and transient status outside the result list in a single line below the search field, and render results inside a bounded Slint `Flickable` viewport.
Context: Manual Phase 6 testing showed that long app/project result lists pushed help text out of view, while keyboard navigation could select rows that were not visible.
Reasoning: The shortcut hint is launcher guidance and should not compete with search results for scroll space. A single line below the search field keeps the shortcuts discoverable without a bottom status block, while still leaving a place for calculator/error feedback after activation. Splitting hint text into zero-stretch key/description segments keeps descriptions close to their keys without losing the lighter key styling. A bounded Slint viewport keeps the panel compact, preserves mouse-wheel/touchpad scrolling through toolkit behavior, and lets the UI adjust `viewport-y` when keyboard selection crosses the visible top or bottom edge.
Alternatives considered: Increasing the launcher height enough to avoid scrolling, leaving help below the repeated rows, keeping a separate bottom status line, or moving scroll state into Rust.
Consequences: Long result lists scroll in the middle of the panel, and the bottom of the launcher no longer contains a default status area. Empty-query show/reset leaves selection on the search input/no row; non-empty queries with matches select the first result. Keyboard navigation can still scroll to keep the selected row visible, while mouse hover selection does not drive viewport scrolling. UI scrolling and row hover/click behavior remain manual verification items instead of brittle unit tests.

## 2026-06-30 - Small best-effort theme-aware desktop app icon resolver

Decision: Resolve launcher app icons with a small helper that uses parsed `.desktop` `Icon` fields, absolute icon paths, configured/common icon themes, simple theme inheritance, and hicolor/pixmaps fallbacks.
Context: Phase 6 needs meaningful row icons without redesigning the launcher, adding heavy dependencies, shelling out, or changing activation behavior.
Reasoning: Most visible value comes from supporting absolute icon paths plus the user's active or common icon theme, especially Papirus-style app icons, before falling back to hicolor and pixmaps. For launcher rows, purpose-made 42x42, 48x48, or nearby app assets are better defaults than always choosing scalable or very large artwork and shrinking it in the UI. Keeping this in `rayslash-core` preserves testability and keeps result activation data separate from display metadata.
Alternatives considered: Adding a full freedesktop icon-theme crate, keeping only hicolor/pixmaps lookup, scanning every installed theme without priority, or keeping all app rows on a generic icon.
Consequences: App icons appear when the `.desktop` `Icon` field points to a supported absolute SVG/PNG/JPG/JPEG file or to a name available in configured/common theme, hicolor, or pixmaps paths. Named icons prefer launcher-sized assets before scalable or very large assets. Full icon-theme metadata handling, symbolic icon recoloring, locale-specific icon fields, and guaranteed current-theme detection on every desktop remain deferred.

## 2026-06-30 - Hidden desktop entry for shortcut-driven launcher

Decision: Ship a Linux desktop entry template at `packaging/linux/dev.rayan6ms.rayslash.desktop` with `Exec=rayslash toggle`, `Icon=dev.rayan6ms.rayslash`, `StartupWMClass=dev.rayan6ms.rayslash`, and `NoDisplay=true`.
Context: Phase 6 needs practical desktop integration files without global shortcut capture or new launcher behavior. The app ID is now `dev.rayan6ms.rayslash`, and the desktop icon name matches that app ID.
Reasoning: `rayslash toggle` is the installed command that starts or contacts the resident launcher, so it is the right desktop integration entry point. The app is primarily opened through a desktop-managed keyboard shortcut, so hiding it from app menus avoids a redundant visible menu item while still allowing local installation and future packaging metadata.
Alternatives considered: Using `Exec=rayslash`, showing the entry in app menus, omitting the icon reference, or waiting for full distro packaging.
Consequences: Users should bind custom keyboard shortcuts directly to `rayslash toggle`. Packagers can install the desktop file as `dev.rayan6ms.rayslash.desktop` and the SVG icon as `dev.rayan6ms.rayslash.svg`; a visible menu entry can wait until there is a concrete reason. The UI sets Slint's XDG app ID to `dev.rayan6ms.rayslash` and sets the window icon from the bundled SVG so desktop panels can match the installed entry and icon. `StartupWMClass` improves matching on X11-style launchers.

## 2026-06-30 - Built-in safe calculator parser

Decision: Implement Phase 4 calculator support with a small recursive-descent parser in `rayslash-core` instead of adding a parser/evaluator dependency.
Context: Calculator queries need to handle common launcher math such as `2^2`, `2**2`, superscript exponents such as `10²`, parentheses, constants, functions, implicit multiplication, and linear equations such as `x + 10 / 2 = 8`, while preserving safe expression detection and useful feedback for invalid math.
Reasoning: The grammar is still small enough to keep local, testable, and dependency-free. The parser evaluates directly from tokens and does not use shell execution, eval-like execution, or external commands. Requiring a calculation signal prevents normal app/project queries such as `calculator` or `pi` from being treated as calculator results. Once a query is math-like, returning typed calculator errors is better than silently hiding the row because users can distinguish formula syntax errors, division by zero, incomplete expressions, unsupported characters, unknown functions, domain errors, oversized results, and unsupported nonlinear equations. Linear equations in one variable are useful and tractable without pretending to be a full symbolic algebra system.
Alternatives considered: Adding a math-expression crate, shelling out to a calculator program, deferring calculator activation until clipboard support is available, or adding a full symbolic algebra dependency.
Consequences: Supported syntax now includes decimal numbers, `+`, `-`, `*`, `/`, `^`, `**`, superscript integer exponents, unary signs, parentheses, `pi`, `e`, common one-argument functions, implicit multiplication, and one-variable linear equations in `x`. Calculator result and calculator error rows rank above app/project results for math-like queries. Normal app/project queries are ignored by the calculator. Enter copies successful calculator results to the clipboard and hides the launcher; calculator errors keep the launcher visible and show the error message.

## 2026-06-30 - Local install from UI crate

Decision: Local user installs use `cargo install --path crates/rayslash-ui`, which installs a binary named `rayslash`.
Context: Desktop shortcut setup needs `rayslash toggle` to resolve without running through Cargo.
Reasoning: The workspace keeps UI and core logic split, but the UI crate is the installable application package. Installing the UI crate preserves the existing app identity and exposes the command used by shortcut docs.
Alternatives considered: Installing from the workspace root, adding a wrapper script, or introducing a packaging tool before distro packaging work.
Consequences: Users should have Cargo's binary directory, usually `~/.cargo/bin`, on `PATH` before binding desktop shortcuts. Future Fedora, Arch/AUR, or AppImage packaging should keep the installed command as `rayslash`.

## 2026-06-29 - App identity

Decision: The app name and binary name are `rayslash`; the desktop app ID is `dev.rayan6ms.rayslash`.
Context: The project needs consistent naming across code, docs, config, and future packaging.
Reasoning: Keeping the app and binary name identical simplifies commands, docs, and desktop shortcut setup.
Alternatives considered: Separate product and binary names.
Consequences: Future packaging and desktop files should preserve this naming unless there is a strong compatibility reason to change it.

## 2026-06-29 - Rust and Slint stack

Decision: Build `rayslash` with Rust and Slint.
Context: The launcher should be lightweight, fast, and native without a browser shell.
Reasoning: Rust provides a strong base for deterministic core logic and Slint provides a compact native UI toolkit.
Alternatives considered: Tauri, Electron, WebView, GTK/libadwaita-first, and Qt.
Consequences: The project uses a Cargo workspace with a Rust core crate and a Slint UI crate.

## 2026-06-29 - Normal desktop window for v1

Decision: Use a normal desktop window instead of layer-shell or raw compositor APIs.
Context: Wayland desktops differ in how they expose overlays and global shortcuts.
Reasoning: A toolkit-managed normal window is the most portable v1 path across GNOME, KDE, Wayland, and X11.
Alternatives considered: `wlr-layer-shell`, `gtk-layer-shell`, raw Wayland protocols, and GNOME/KDE-specific overlay APIs.
Consequences: Global shortcuts are configured by the desktop environment rather than captured by the app.

## 2026-06-29 - Desktop-managed global shortcut

Decision: The intended global shortcut command is `rayslash toggle`.
Context: Modern Wayland sessions restrict arbitrary global shortcut capture.
Reasoning: Delegating the shortcut to GNOME/KDE settings follows desktop security expectations.
Alternatives considered: Global shortcut libraries and compositor-specific protocols.
Consequences: Toggle IPC and resident-process behavior will be implemented in a later phase.

## 2026-06-29 - Cargo workspace split

Decision: Use separate `rayslash-core` and `rayslash-ui` crates in a Cargo workspace.
Context: Search, config, launching, and indexing logic should stay testable outside the UI.
Reasoning: A separate core crate keeps UI code thin and prevents launcher behavior from becoming tied to Slint widgets.
Alternatives considered: A single binary crate.
Consequences: UI calls into core; future tests should mostly live in `rayslash-core`.

## 2026-06-29 - Frameless launcher window

Decision: Hide native window decorations using Slint's built-in `Window.no-frame` property.
Context: Phase 1 UI polish should make the normal toolkit window feel more like a launcher panel without using compositor-specific APIs.
Reasoning: Slint 1.17 exposes `no-frame` as a normal window property for borderless/frameless windows, so this avoids raw Wayland, layer-shell, GNOME-only, or KDE-only code.
Alternatives considered: Keeping native decorations, using winit directly, using layer-shell, or using desktop-specific APIs.
Consequences: The launcher window is frameless while still being a normal Slint desktop window. Move/resize behavior may vary by desktop environment and should be verified manually before adding any custom workaround.

## 2026-06-30 - Project fuzzy matcher

Decision: Use `nucleo-matcher` for Phase 2 project folder fuzzy search.
Context: Project search needs real fuzzy matching and ranking while scanning remains shallow and the implementation stays small.
Reasoning: `nucleo-matcher` is a focused matcher crate with case-insensitive fuzzy scoring, prefix preference, and no need to adopt a larger indexing/search framework for the current small project list.
Alternatives considered: Keeping substring matching and using a skim-style matcher. The `skim-matcher` crate name is not available on crates.io; the broader `fuzzy-matcher` crate could work, but `nucleo-matcher` is lightweight and already exposes the scoring behavior needed here.
Consequences: Project search now depends on `nucleo-matcher` directly. Results are ranked by fuzzy score; no-match project queries return an empty result list.

## 2026-06-30 - Project folder default action

Decision: Enter opens selected project folders with `xdg-open <folder>` on Linux, while Ctrl+Enter opens them with `code <folder>`.
Context: Phase 2 needs a neutral default project action and a useful editor shortcut without introducing configurable actions yet.
Reasoning: Opening the folder with the system file manager is less opinionated than opening VS Code. `xdg-open` is the standard desktop opener available across common Linux desktop environments, and direct process spawning keeps command construction testable and avoids shell quoting issues.
Alternatives considered: Keeping VS Code on Enter, using `gio open`, shelling through `sh -c`, or adding configurable project actions immediately.
Consequences: `xdg-open` is the current Linux opener. Both `xdg-open` and `code` are spawned with separate program and argument values. Configurable project actions remain deferred.

## 2026-06-30 - Conservative desktop Exec launching

Decision: Parse desktop `Exec` lines into direct program/argument command specs, remove field codes, and spawn without a shell. For app results, Ctrl+Enter currently launches the app the same way as Enter.
Context: Phase 3 needs installed app launching without adding IPC, daemon behavior, configurable actions, or shell execution.
Reasoning: Direct process spawning keeps app launches testable and avoids shell quoting and injection behavior. Removing field codes is a conservative first pass because rayslash does not yet provide selected files, URLs, icon args, translated app names, or desktop-file paths to expand into those fields.
Alternatives considered: Shelling through `sh -c`, preserving raw `Exec` strings, implementing the full Desktop Entry `Exec` expansion rules immediately, or making Ctrl+Enter a no-op for apps.
Consequences: Common app entries with simple commands and quoted arguments work, while entries that rely on full Desktop Entry field-code expansion or shell syntax may need later compatibility work. Project Ctrl+Enter remains reserved for opening projects in VS Code.

## 2026-06-30 - Detached child stdio for launch actions

Decision: External app, project folder, and VS Code launch actions set child stdin, stdout, and stderr to `Stdio::null()` before spawning.
Context: When running rayslash from a terminal with `cargo run -p rayslash`, successfully launched GUI apps were inheriting rayslash's terminal streams and printing warnings into the same terminal.
Reasoning: Launch actions should feel detached from the foreground launcher process once spawn succeeds, while spawn failures still need to return normally so the UI can keep the launcher visible and show status text. Centralizing stdio handling in the core spawn helper applies the behavior consistently to desktop `Exec` commands, `xdg-open`, and `code`.
Alternatives considered: Leaving inherited stdio in place, shelling through `sh -c` or `zsh -c`, or only detaching desktop app launches.
Consequences: Child output from successfully spawned GUI actions is discarded. Failures to spawn still surface through the existing error handling path.

## 2026-06-30 - Unix socket IPC for resident toggle

Decision: Use a Unix domain socket at `$XDG_RUNTIME_DIR/rayslash.sock` for local single-instance IPC.
Context: Phase 5 needs `rayslash toggle` to show or hide an already-running launcher process without adding desktop-specific shortcut capture or a larger IPC framework.
Reasoning: A Unix socket is small, Linux-friendly, easy to test without a real desktop session, and fits the single-user local command model. It avoids adding DBus before there is a concrete need for a bus-visible service contract.
Alternatives considered: DBus, lock files plus signals, TCP on localhost, and global shortcut libraries.
Consequences: `rayslash` is currently Unix/Linux-oriented for resident IPC. The app owns only show/toggle behavior; global shortcuts remain configured by the desktop environment with `rayslash toggle`. Startup detects stale socket paths by attempting to connect before unlinking them.
