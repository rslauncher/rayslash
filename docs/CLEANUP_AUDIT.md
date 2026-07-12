# Cleanup Audit

Last updated: 2026-07-02

This document records bugs, inconsistencies, optimization opportunities, redesign candidates, and odd implementation details found during a cleanup-focused pass over the current codebase. It complements the roadmap and task list, which are mostly organized around new feature phases.

Current verification state:

- `cargo fmt --check` passes.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- `cargo test --workspace` passes.
- The findings below are not compiler failures. They are behavior, maintainability, product, and public-readiness issues to handle deliberately.

## Highest Priority Cleanup

### Provider empty states ignore provider settings

Resolved on 2026-07-02.

`crates/rayslash-core/src/search.rs` now builds empty placeholder rows from the enabled provider settings. Disabling calculator removes the `Calculate` placeholder, calculator-only empty states show calculator guidance, and the all-disabled case keeps a separate `No providers enabled` row.

### Result hover behavior matches clipped-row intent

Resolved on 2026-07-02.

`crates/rayslash-ui/ui/result_list.slint` now lets hover or pointer movement select visible rows, including clipped partial rows at the viewport edge, without moving the result scroll position. Keyboard navigation remains responsible for scrolling selected rows fully into view.

### Alternate opener command line is parsed without a shell

Resolved on 2026-07-02.

`actions.alternate_folder_opener_command` now accepts a small command line that is parsed into direct program/argument values without invoking a shell. A value such as `code --reuse-window` spawns `code` with `--reuse-window` and the selected folder path as arguments. The built-in `xdg-terminal-exec` special case still receives no implicit folder path and runs with the folder as the working directory.

### Settings saves can overwrite hand-authored config shape

Resolved on 2026-07-02.

Settings saves now create a timestamped `config.toml.backup-...` sibling before replacing an existing config file, so hand-authored comments, ordering, formatting, and unknown fields remain recoverable. If startup failed to read or parse `config.toml` and the UI fell back to defaults, the settings save path is blocked until the file is fixed and rayslash is restarted.

### Synchronous refresh work is duplicated and UI-thread heavy

Resolved on 2026-07-02.

Settings-open uses a throttled desktop-app refresh helper that clears the icon cache, rebuilds opener choices, refreshes alternate opener visuals and settings diagnostics, and profiles actual refresh work under `RAYSLASH_PROFILE=1`. Query changes, show/reset, settings saves, and ranking clears now share result-view refresh helpers. Settings save callbacks build a validated config through a shared settings parser before persistence and runtime application.

## Bugs And Behavioral Risks

### Provider-aware wording is missing

Resolved on 2026-07-02.

`No results` subtitles now list only enabled provider names and use `folders` terminology. Empty placeholders and the search-box placeholder copy were updated to the same terminology.

### Relative folder sources are normalized before scanning

Resolved on 2026-07-02.

Config normalization now expands `~` and converts relative folder sources to absolute paths using the current working directory. Settings saves serialize the normalized form, so subsequent scans, stable IDs, and folder actions use absolute paths.

### Config and state writes are atomic

Resolved on 2026-07-02.

`config.toml` and `ranking.toml` are now saved by writing a temporary sibling file, syncing it, and renaming it into place. Settings saves also create timestamped backups of an existing `config.toml` before replacement.

### Ranking state is pruned on learned launches

Resolved on 2026-07-02.

Learned ranking now prunes on successful learned app/folder launches. Entries whose app/folder IDs are no longer in the current index are removed, entries older than 180 days are removed, each entry keeps at most 64 query prefixes by count, and the state keeps at most 1000 entries by recency.

### Desktop entry parsing has two behavior levels

`parse_desktop_entry` filters `Type`, `Hidden`, `NoDisplay`, name, and exec shape. Discovery additionally filters `TryExec`, missing exec targets, and current desktop compatibility. The public function name can read like it returns launchable apps, but only discovery applies availability and desktop-session filtering.

Suggested direction: make this distinction explicit in names or docs, and add tests for desktop-session filtering in the discovery path.

### Desktop Exec handling can leave odd arguments

Field-code removal is conservative and tested, but an argument like `--url=%U` becomes `--url=`. That avoids shell execution and unsupported expansion, but some applications may receive surprising empty option values.

Suggested direction: decide whether to drop arguments that become empty-valued field-code wrappers, implement fuller Desktop Entry expansion, or keep the current behavior documented as a compatibility limit.

### IPC temp fallback uses a user-specific directory

Resolved on 2026-07-02.

When `XDG_RUNTIME_DIR` is unavailable, the socket now falls back to a user-specific temp subdirectory such as `/tmp/rayslash-1000/rayslash.sock`, and socket binding creates the parent directory before binding.

### Icon image cache can go stale

The UI icon cache is keyed by original path and stores failed loads as well as loaded images. Regular icon file updates are not reloaded until process restart. Extensionless icon files compute a content-based cache path, but the top-level cache can still prevent rechecking the original path.

Suggested direction: clear icon cache during desktop-app refresh, or include file metadata in the in-memory cache key.

## Optimization Opportunities

- Profile settings-open app refresh. Added on 2026-07-02.
- Cache desktop-entry scan results behind directory mtimes or move refresh to a background task if profiling shows visible hitches.
- Avoid lowercasing titles repeatedly during every sort by storing normalized search/display keys.
- Rebuild fewer Slint `ResultItem` values on each query, especially for unchanged app icons.
- Add diagnostics for ignored project roots and desktop entry parse/filter counts.
- Consider limiting icon theme discovery to configured/current themes plus hicolor before scanning every discovered theme.

## Redesign Candidates

### Provider pipeline

Search already behaves like it has providers, but provider behavior is split across `search.rs`, `providers.rs`, activation code, and settings. A typed internal provider pipeline would let each provider own result generation, empty state, diagnostics, enabled state, ranking eligibility, and activation actions.

This should stay internal until action safety, config, privacy, and test boundaries are stable. It is not a plugin-marketplace prerequisite.

### Action model

Actions still carry project-era names such as `open_project_in_editor`, even though the product now talks about folders and alternate openers. The action layer also lacks a general command template, which makes the alternate opener setting less capable than the UI wording suggests.

Suggested direction: introduce typed actions such as `OpenFolderDefault`, `OpenFolderWithAlternate`, `LaunchDesktopApp`, `CopyCalculatorResult`, and `DismissNoResults`, with explicit command construction rules.

### Settings model

Settings callbacks currently mix UI extraction, validation, persistence, project rescan, result refresh, opener visual updates, diagnostics, and status messages. This makes autosave edge cases hard to reason about.

Suggested direction: use a transient settings draft, validate it, persist it, then apply a single runtime update path. Keep "close settings" separate from "save settings" so Esc/cancel semantics are clear.

### Desktop standards layer

Desktop entry parsing, availability filtering, icon lookup, and folder-opener filtering are all local implementations. They are testable and small, but public packaging will increase the cost of partial freedesktop behavior.

Suggested direction: before broad packaging, either improve fixtures toward freedesktop compatibility or evaluate maintained crates for desktop entries and icon themes.

### UI verification

Resolved on 2026-07-02.

[UI_VERIFICATION.md](UI_VERIFICATION.md) now records a real-desktop checklist for result scrolling, hover behavior, settings autosave, focus-loss hiding, icon rendering, and related layout pass criteria. This is still manual verification, but it gives Slint/layout behavior an explicit workflow instead of leaving it implicit.

## Odd Or Surprising Details To Revisit

- The folder picker replaces the whole folder-source field with the selected folder instead of appending to existing semicolon-separated sources.
- Settings text fields save on Enter and focus loss, while settings cancel resets UI fields to current config. This makes "cancel" mean "discard unsaved field text" only if autosave has not already run.
- App results use Ctrl+Enter the same as Enter, while the shortcut hint only describes alternate folder opening.
- Successful launch status text is set immediately before hiding the window, so users usually will not see it.
- Project/folder terminology is still mixed across code, docs, tests, and user-facing strings.
- `open_project_in_vscode_command` remains public for compatibility/tests even though VS Code is no longer the public default alternate opener.

## Suggested Cleanup Order

The cleanup audit backlog in [TASKS.md](TASKS.md) is complete as of 2026-07-02. Continue freedesktop parser/icon compatibility only after stronger fixtures are in place.
