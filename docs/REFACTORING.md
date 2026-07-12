# Refactoring Plan

The project has grown past the original bootstrap shape. Refactoring is now Phase 9 public-readiness work and should happen before broad packaging. The goal is to stabilize internal boundaries and tests while preserving current behavior, not to redesign the product.

## Principles

- Keep behavior covered before moving code.
- Prefer small module splits over rewrites.
- Move tests when it improves readability or fixture reuse; keep tiny parser/helper tests inline when they are clearer next to the code.
- Keep UI behavior, core search behavior, config/state formats, and launch semantics independently testable.
- Do not introduce a plugin system or broad provider framework as a refactor. Build internal boundaries first.

## Phase 9 Priorities

Do these before packaging work that depends on stable desktop metadata, icon behavior, or Linux integration:

1. Add focused regression coverage or fixtures for the behavior being moved.
2. Split `apps.rs` enough that desktop-entry parsing, app discovery, and icon lookup can be improved independently.
3. Split `search.rs` enough that result types, matching/ranking orchestration, and provider-specific result construction are separate.
4. Split `main.rs` enough that settings, activation, result item conversion, diagnostics, and opener visuals are not all owned by the binary entry point.
5. Split `calc.rs` after apps/search/UI are smaller if the public calculator API can stay stable and parser/equation behavior can remain covered from integration tests.
6. Split the Slint file only if it is low-risk with the current Slint include/build setup.

Done means `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` pass after the splits, and docs match the new module layout.

## Current Large Files

As of the completed Phase 9 split, the largest remaining files are:

- `crates/rayslash-ui/ui/settings_panel.slint`: about 575 lines.
- `crates/rayslash-core/src/calc/equation.rs`: about 380 lines.
- `crates/rayslash-ui/src/main.rs`: about 370 lines.
- `crates/rayslash-core/src/ranking.rs`: about 320 lines, with persistence tests moved out.
- `crates/rayslash-core/src/calc/parser.rs`: about 310 lines.
- `crates/rayslash-core/src/config.rs`: about 300 lines, with load/save compatibility tests moved out.
- `crates/rayslash-ui/ui/rayslash.slint`: about 385 lines.
- `crates/rayslash-ui/ui/result_list.slint`: about 270 lines.
- `crates/rayslash-core/src/apps/icon_lookup/themes.rs`: about 260 lines.

Line count alone should not drive the work. Prioritize files where independent behavior is already mixed together and future packaging/provider work will need clearer boundaries.

## Near-Term Splits

### UI crate

`crates/rayslash-ui/src/main.rs` now owns process startup, resident setup, Slint component construction, initial state loading, reset/query callback registration, focus-loss handling, and IPC server hookup. Settings callbacks, activation handling, result conversion, icon image caching, alternate opener visuals, settings diagnostics/parsing, runtime search truncation, profiling helpers, and window show/hide helpers have been split out.

Candidate modules:

- `runtime_state.rs`: profiling, runtime ranking fallback, result truncation, and selected-index policy.
- `window_state.rs`: show/hide and IPC visibility handling.
- `settings.rs`: settings property binding, diagnostics, and field parsing.
- `settings_callbacks.rs`: settings save/cancel/open, folder picker, alternate opener picker, and clear-ranking callbacks.
- `result_items.rs`: core result to Slint item conversion and image cache.
- `activation.rs`: selected-result activation, status messages, clipboard copy, launch dispatch, and learned-ranking recording.
- `opener_visual.rs`: alternate opener app matching, icon/background selection, and picker item construction.

Keep `main.rs` responsible for process startup, Slint component construction, IPC hookup, and high-level callback registration.

### Slint UI

`crates/rayslash-ui/ui/rayslash.slint` now remains the main `AppWindow` composition file. Focused Slint pieces have moved to:

- `ui/models.slint`: shared `ResultItem` and `AppChoiceItem` structs.
- `ui/components.slint`: search box, settings toggle, and inline gear icon.
- `ui/settings_panel.slint`: in-launcher settings view, app picker, diagnostics, and settings save wiring.
- `ui/result_list.slint`: result viewport, row rendering, fallback icons, hover/click behavior, and selection scroll handling.

The generated Rust `AppWindow`, `ResultItem`, and `AppChoiceItem` surface remains stable. Further Slint splitting should be driven by specific UI changes; `settings_panel.slint` is still large, but it is now a coherent settings surface rather than being mixed into the main window file.

### Core search/providers

`crates/rayslash-core/src/search.rs` now keeps the public API and mixed search orchestration while helper modules own result types, provider-specific result construction, and matcher/ranking helpers. Broad mixed-search behavior tests now live under `crates/rayslash-core/tests/`; only narrow private path/subtitle helper tests remain inline.

Do not add a public provider/plugin API in this phase. The immediate goal is to separate current result construction from shared ordering and learned-ranking behavior.

### Desktop apps and icons

`crates/rayslash-core/src/apps.rs` now keeps the public API and shared `DesktopApp` type while helper modules own desktop-entry parsing, discovery, and icon lookup.

`crates/rayslash-core/src/apps/icon_lookup.rs` is now a small module root. `apps/icon_lookup/themes.rs` owns icon-theme directory discovery and configured/common theme ordering. `apps/icon_lookup/paths.rs` owns absolute/named icon path resolution and supported extension checks.

This split has happened before deeper freedesktop icon-theme compatibility work.

Keep the public core API stable during the first split so UI code does not need to change at the same time. `TryExec`, `OnlyShowIn`, `NotShowIn`, missing executable filtering, `MimeType` parsing, `Categories`-based folder opener filtering, localized names, keywords, desktop actions, `DBusActivatable`, and WPS-style icon fallback now have focused coverage. Future compatibility work should improve deeper icon-theme fixtures before changing behavior.

### Calculator

`crates/rayslash-core/src/calc.rs` now keeps the public `Calculation` type, `calculate()` entry point, math-like query detection, and result formatting. Calculator internals are split into:

- `calc/error.rs`: typed calculator errors and user-facing messages.
- `calc/parser.rs`: recursive-descent numeric expression parser, functions, constants, implicit multiplication, and superscript exponents.
- `calc/equation.rs`: linear equation parsing and solving for `x`.

Broad calculator behavior tests now live in `crates/rayslash-core/tests/calculator.rs`, so parser/equation refactors are checked through the public API.

## Test Structure

Keep these inline:

- Tiny parser/helper edge cases where private helpers need direct access.
- Narrow ordering/path formatting tests where fixture indirection would obscure the behavior.

Move or add these under crate-level `tests/` as they grow:

- Calculator public behavior.
- Config load/save compatibility.
- Desktop entry fixtures.
- Icon theme lookup fixtures.
- Action command construction and detached-spawn behavior.
- Project folder discovery behavior.
- Learned ranking state persistence.
- Cross-module mixed search behavior.
- UI-adjacent pure Rust helpers such as result item conversion and opener visual selection.

Add reusable fixture builders for desktop apps, project folders, config files, icon themes, and ranking state before expanding integration tests heavily.

`crates/rayslash-core/tests/` now exists with reusable fixtures for desktop entries, icon themes, project/app rows, config shapes, and learned-ranking state. Add `crates/rayslash-ui/tests/` only for pure Rust UI-crate helpers or CLI/IPC behavior that does not need a live desktop session.

## Rewrite Guidance

Do not rewrite the app around a new architecture until these are true:

- Current behavior is covered by tests or clear manual verification notes.
- The replacement boundary is smaller and easier to reason about than the current code.
- Config and state compatibility are documented.
- The change can land in reviewable slices.

Good rewrite candidates later:

- A typed internal provider pipeline after aliases exist.
- A settings model that separates TOML config, generated state, and transient UI edits.
- A more complete freedesktop app/icon model before public packaging.

Poor rewrite candidates now:

- A public plugin system.
- Replacing Slint.
- Replacing the whole search stack.
- Moving everything out of inline tests without improving fixture reuse.
