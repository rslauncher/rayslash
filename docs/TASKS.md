# Tasks

## Phase 0

- [x] Create Cargo workspace.
- [x] Add basic Slint UI crate.
- [x] Add core crate.
- [x] Create docs.
- [x] App opens a simple launcher window.
- [x] Basic keyboard handling: Escape closes or hides the window.

## Phase 1

- [x] Search input.
- [x] Static result list.
- [x] Search input readable before and after focus.
- [x] Cleaner launcher-like spacing and result row styling.
- [x] Native window decorations hidden through Slint `no-frame`.
- [x] Keyboard navigation.
- [x] Enter activates selected placeholder item.
- [x] Neutral dark theme palette.
- [x] Neutral grey dark theme palette.
- [x] Tab and Shift+Tab placeholder navigation.
- [x] Result placeholder icons vertically centered.
- [x] Increased rounded corners and bottom padding.
- [x] Extra bottom breathing room below results.
- [x] Clean rounded frameless corners using Slint transparency/clipping.
- [x] Existing show animation/transition feels good enough for v1.
- [x] Theme that looks clean on GNOME/KDE.

## Phase 2 - complete

- [x] Configurable project roots.
- [x] Scan folders under roots.
- [x] Fuzzy-search project folders. Substring search was an intermediate step and has been replaced by fuzzy matching.
- [x] Harmless selected-project preview in UI.
- [x] Enter opens selected folder with system file manager.
- [x] Ctrl+Enter opens selected project with `code <folder>`.
- [x] Hide launcher after successfully opening selected project action.
- [x] Compact project path display with `~/...` under home.

## Phase 3 - complete

- [x] Parse `.desktop` files.
- [x] Fuzzy-search installed apps.
- [x] Launch apps correctly.
- [x] Handle icons later if not practical at first.

## Phase 4

- [x] Detect math expressions.
- [x] Show result as first item.
- [x] Enter copies calculator result and hides the launcher without launching external commands.
- [x] Keep implementation small and safe.
- [x] Support exponent operators, common functions, constants, and implicit multiplication.
- [x] Support superscript exponents such as `10²`.
- [x] Show calculator error messages in calculator rows.
- [x] Solve linear equations in `x`, such as `x + 10 / 2 = 8`.
- [x] Add focused tests for calculator success cases, calculator error cases, and calculator/search result rows.

## Phase 5

- [x] Single-instance behavior.
- [x] `rayslash toggle`.
- [x] Hidden resident process for fast opening.
- [x] Local IPC.
- [x] Desktop shortcut documentation.

## Phase 6

- [x] Local install documentation.
- [x] Linux desktop entry template.
- [x] Local desktop entry install documentation.
- [x] Fedora packaging notes.
- [x] Arch/AUR packaging notes.
- [x] Optional AppImage notes.
- [x] Config docs.
- [x] Linux desktop icon template integration notes.
- [x] Runtime XDG app ID and window icon metadata.
- [x] Runtime `StartupWMClass` desktop metadata.
- [x] Basic launcher icon polish:
  - [x] rayslash icon in the header.
  - [x] Calculator, project folder, generic app, and placeholder row icons.
  - [x] Best-effort app icons from `.desktop` `Icon` fields.
  - [x] Prefer launcher-sized hicolor app icon assets when available.
  - [x] Check configured/common icon themes such as Papirus before hicolor fallback.
  - [x] Render real and fallback row icons at the same target size.
- [x] Real launcher UI/UX fixes:
  - [x] Single-line shortcut/status hint below the search input.
  - [x] Lighter shortcut key labels inside the hint line.
  - [x] Keep shortcut descriptions visually adjacent to their key labels.
  - [x] Removed unnecessary bottom default status area.
  - [x] Trimmed the remaining idle bottom strip.
  - [x] Scrollable result area for longer app/project lists.
  - [x] No selected row when opening with an empty query.
  - [x] First result selected after typing a non-empty matching query.
  - [x] Show a no-results row instead of an empty list when real entries exist but nothing matches.
  - [x] Enter on a no-results row hides the launcher.
  - [x] Keyboard navigation keeps the selected row visible.
  - [x] Mouse hover selects/highlights visible result rows, including clipped partial rows.
  - [x] Mouse selection at clipped viewport edges does not auto-scroll.
  - [x] Mouse click activates rows.
  - [x] Ctrl+click uses the same secondary action path as Ctrl+Enter.
  - [x] Click outside hides the launcher through the winit focus-lost window event.
  - [x] Query and IPC show/reset behavior reset result scrolling to the top.
  - [x] Header icon vertically centered with the title.
  - [x] Real app icons render larger and unframed in result rows.
- [x] Ctrl-held VS Code row icon switching with a targeted transition.
- [x] Opt-in performance profiling for startup discovery and query updates.
- [x] Keep broader app animations minimal for now.

## Phase 7 - public settings and customization

- [x] Replace the header `preview` text with a settings icon button on the same line as the icon and title.
- [x] Decide the settings UI shape: in-launcher panel, separate preferences dialog, or compact normal window.
- [x] Document the initial config schema expansion before implementation.
- [x] Add configurable folder sources in the settings UI.
- [x] Add a native folder picker for choosing the folder source.
- [x] Add configurable alternate folder opener command instead of hardcoding `code`.
- [x] Add an installed-app picker for the alternate folder opener command.
- [x] Add feature toggles for current user-facing features: apps, folders, calculator, and alternate folder opener.
- [x] Add provider toggles for aliases/quick links and future optional providers when those providers exist.
- [x] Add appearance setting for result count.
- [x] Add appearance settings for theme and density.
- [x] Add learned-ranking controls after Phase 8 defines learned ranking.
- [x] Add diagnostics/settings readout for config path, state path, socket path, app count, and folder count.
- [x] Add basic icon lookup diagnostics.
- [x] Preserve TOML as the editable source behind the settings UI.

## Phase 8 - learned ranking and aliases

- [x] Define stable result IDs for current app, folder, calculator, and no-results rows.
- [x] Store usage history under the XDG state directory, separate from config.
- [x] Track launch count, last launched time, and query prefixes for selected app/folder results.
- [x] Document and test the ranking formula before wiring it into UI search.
- [x] Boost frequently selected apps/folders without overriding calculator-first behavior for math-like queries.
- [x] Add settings to disable search learning.
- [x] Add a clear-history action.
- [x] Add alias/quick-link config for URLs, files, folders, and explicit commands.
- [x] Add tests for learned ranking, history persistence, disabled learning, disabled provider behavior, calculator precedence, corrupted state fallback, and history clearing.

## Phase 9 - refactoring and internal boundaries

- [x] Split `crates/rayslash-core/src/apps.rs` into desktop-entry parsing, app discovery, and icon lookup modules before deeper desktop-entry or icon-theme compatibility work.
- [x] Split `crates/rayslash-core/src/search.rs` into result types, matching/ranking orchestration, and provider-specific result construction while preserving current search behavior.
- [x] Split `crates/rayslash-core/src/calc.rs` into a small public calculator API plus parser, equation, and error modules while preserving calculator behavior.
- [x] Split `crates/rayslash-core/src/apps/icon_lookup.rs` into icon-theme discovery and path-resolution modules while preserving icon lookup behavior.
- [x] Split `crates/rayslash-ui/src/main.rs` into runtime state, settings wiring, result item conversion, activation handling, diagnostics, and opener visual helpers.
  - [x] Move runtime search/selection/profiling helpers to `crates/rayslash-ui/src/runtime_state.rs`.
  - [x] Move show/hide and IPC visibility helpers to `crates/rayslash-ui/src/window_state.rs`.
  - [x] Move result item conversion and icon image cache to `crates/rayslash-ui/src/result_items.rs`.
  - [x] Move alternate opener visual helpers to `crates/rayslash-ui/src/opener_visual.rs`.
  - [x] Move settings diagnostics/readouts and settings field parsing to `crates/rayslash-ui/src/settings.rs`.
  - [x] Move settings callback wiring to `crates/rayslash-ui/src/settings_callbacks.rs`.
  - [x] Move activation handling to `crates/rayslash-ui/src/activation.rs`.
- [x] Split `crates/rayslash-ui/ui/rayslash.slint` into component files only if Slint include/build support stays simple and generated Rust names remain predictable.
  - [x] Move shared Slint data structs to `crates/rayslash-ui/ui/models.slint`.
  - [x] Move result viewport and row rendering to `crates/rayslash-ui/ui/result_list.slint`.
  - [x] Move the settings surface to `crates/rayslash-ui/ui/settings_panel.slint`.
- [x] Create `crates/rayslash-core/tests/` for integration tests.
- [x] Create reusable desktop-entry, config, icon-theme, project-folder, and learned-history fixtures.
- [x] Move broad cross-module tests out of implementation files where that improves readability.
- [x] Move action, calculator, config load/save, desktop-entry, icon lookup, ranking persistence, project scanning/search, and stable-result-ID behavior tests into crate-level integration tests.
- [x] Keep small private parser/helper tests inline when useful.
- [x] Add focused regression tests around current behavior before each module move.
- [x] Preserve config/state compatibility and current launch semantics throughout the split.
- [x] Update [ARCHITECTURE.md](ARCHITECTURE.md), [REFACTORING.md](REFACTORING.md), and [TESTING.md](TESTING.md) as module boundaries change.

## Cleanup audit backlog

These tasks come from [CLEANUP_AUDIT.md](CLEANUP_AUDIT.md) and are intentionally tracked outside the feature-first phases.

- [x] Make placeholder and empty-state rows respect enabled provider settings.
- [x] Make no-results and placeholder wording provider-aware and consistently use folder terminology.
- [x] Fix or re-document clipped partial row hover selection after manual UI verification.
- [x] Rename or redesign alternate folder opener command handling so users cannot mistake a program field for a shell command line.
- [x] Centralize settings save/apply/refresh flow instead of duplicating refresh logic in multiple UI callbacks.
- [x] Add config/state atomic write behavior and decide whether settings saves should create backups.
- [x] Avoid overwriting parse-broken or hand-authored config with defaults from the settings UI.
- [x] Normalize folder sources to absolute paths or reject relative folder source paths.
- [x] Add ranking history pruning for stale result IDs, old entries, and oversized query-prefix maps.
- [x] Refresh derived opener visuals, settings diagnostics, and icon cache state when desktop apps are rediscovered.
- [x] Add profiling around settings-open app refresh.
- [x] Revisit the temp-directory IPC fallback so unusual environments do not share a plain `/tmp/rayslash.sock`.
- [x] Add Slint/UI verification for scrolling, hover selection, settings autosave, focus loss, and icon rendering.

## Phase 10 - packaging and Linux integration

- [x] Create a packaging inventory that records binary name, app ID, desktop entry name, icon name, metainfo ID, config path, state path, cache path, and runtime socket path.
- [x] Add AppStream/metainfo metadata.
- [x] Add metadata validation commands to docs and CI.
- [x] Decide the first public package target, with Flatpak as the strongest broad-distribution candidate to evaluate.
- [x] Implement complete Fedora RPM packaging after install layout is stable.
- [x] Implement complete Arch/AUR packaging after install layout is stable.
- [x] Revisit AppImage after desktop entry, icon, and update expectations are clear.
- [x] Improve Desktop Entry compatibility for `TryExec`, `OnlyShowIn`, `NotShowIn`, missing `Exec` targets, and `MimeType`/`Categories`-based folder opener filtering.
- [x] Improve Desktop Entry compatibility for keywords, localized names, desktop actions, and `DBusActivatable`.
- [x] Improve icon-theme lookup toward freedesktop behavior or choose a maintained crate.

## Phase 11 - project maturity

- [ ] Add `docs/TESTING.md` to the docs index once a README exists.
- [x] Add a refactoring/structure plan for larger files, test placement, and future rewrites.
- [x] Add CI for `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo build --workspace`.
- [x] Add desktop-entry and AppStream validation to CI when metadata files exist.
- [ ] Add manual verification matrix for GNOME/KDE, Wayland/X11, Fedora, Arch, Ubuntu/Debian, and openSUSE.
- [ ] Add release notes/changelog process.

## Phase 12 - optional provider expansion

- [x] Add configurable web search templates.
- [x] Add default-browser web search rows that let supported browsers choose their configured search engine.
- [x] Require the built-in default-browser search command to use `search` before the query.
- [x] Add settings editing for aliases and additional search engines.
- [x] Add Space/Tab keyword activation for additional search engines.
- [x] Evaluate and add local unit conversion.
- [x] Add currency conversion using a free no-key rate API.
- [x] Add explicit `time in <place>` lookup using a free no-key place/timezone API.
- [x] Add built-in reboot, shutdown, logout, timer, and reminder commands.
- [ ] Evaluate snippets as an opt-in provider.
- [ ] Evaluate clipboard history as an opt-in provider with clear privacy controls.
- [ ] Evaluate script providers after command execution policy is documented.
- [ ] Evaluate window switching only after a reliable cross-desktop strategy is chosen.

## Phase 13 - installable module ecosystem

- [x] Audit the virtual/bundled module state and choose the free hosting/package/runtime architecture.
- [x] Write the owner migration runbook and final acceptance checklist in [manual_migration.md](manual_migration.md).
- [x] Complete the owner repository, identity, signing, moderation, support, and packaging prerequisites.
- [x] Freeze module API v1 and publish schemas, WIT, conformance fixtures, SDK, templates, and author documentation.
- [x] Implement the signed static registry and GitHub pull-request submission pipeline.
- [ ] Implement safe local install, update, rollback, remove, cache, revocation, and key-rotation behavior.
- [ ] Complete the Modules catalog/details/permission/offline UI.
- [x] Implement and package the optional sandboxed WASM host.
- [x] Extract and independently release all seven official modules.
- [x] Migrate existing version-1 virtual-module users as explicit Restore choices without installing optional modules for fresh users.
- [ ] Update native and Flatpak packaging so fresh packages contain no official optional modules.
- [ ] Complete automated adversarial tests, performance/size budgets, documentation checks, and the Linux manual matrix.
