# Roadmap

Phases 0 through 6 describe the first usable launcher and are now complete. New work should be planned as public-readiness phases instead of continuing to pile polish onto the original bootstrap list. Refactoring now has its own pre-packaging phase because packaging should happen after the internal desktop-entry, icon, search, settings, and test boundaries are stable enough to support public Linux integration work.

## Completed V1 Phases

### Phase 0 - Project Bootstrap

- Cargo workspace.
- Slint UI crate.
- Core crate.
- Initial docs.
- Centered launcher window.
- Escape hide/close behavior.

### Phase 1 - Minimal Launcher UI

- Search input.
- Result list.
- Keyboard navigation.
- Enter activation.
- Launcher visual polish.
- GNOME/KDE-friendly dark theme.

### Phase 2 - Project Launcher

- Configurable project roots.
- Shallow project folder scanning.
- Fuzzy project search.
- Default folder opener.
- VS Code secondary action.

### Phase 3 - Desktop App Launcher

- `.desktop` parsing.
- Installed app discovery.
- Fuzzy app search.
- Direct non-shell app launching.
- Conservative `Exec` handling.

### Phase 4 - Calculator

- Math expression detection.
- Calculator result/error rows.
- Clipboard copy for valid results.
- Safe local parser.
- Common functions, constants, superscript exponents, implicit multiplication, and linear equations.

### Phase 5 - Resident Process And Toggle

- Single-instance behavior.
- `rayslash toggle`.
- Hidden resident process.
- Unix socket IPC.
- Desktop shortcut docs.

### Phase 6 - Polish And Packaging Notes

- Local install docs.
- Desktop entry template.
- Fedora, Arch/AUR, and AppImage notes.
- Config docs.
- Header/result icons.
- Bounded scrollable result viewport.
- Mouse activation and click-outside hiding.
- Opt-in performance profiling.

## Phase 7 - Public Settings And Customization

Goal: make the app useful to people who do not want to rebuild it or edit source code.

- Replace the header `preview` text with a settings button on the same header line.
- Add a settings surface for folder sources, enabled providers, alternate folder opener command, theme/density, result count, and learned-ranking controls.
- Expand config docs before implementation so the schema stays deliberate.
- Keep settings backed by TOML so manual editing remains supported.
- Add validation and diagnostics for config paths, state paths, app count, project count, icon lookup, socket path, and installed desktop entry status.
- Decide whether settings open inside the launcher panel, as a second normal window, or as a compact preferences dialog.

Phase 7 work uses an in-launcher settings panel and implements folder sources, a folder picker, current provider toggles including aliases, optional alternate folder opener with an installed-app picker, result count, theme/density controls, and basic config/state/socket/count/icon diagnostics. Packaging and optional providers beyond aliases were deferred to their explicit phases; the first Phase 12 pass later added Web, Units, Currency, and Time toggles. Learned-ranking controls were added with the Phase 8 foundation.

## Phase 8 - Learned Ranking And Aliases

Goal: make search feel personal while staying predictable and private.

- Persist usage-based ranking data under the XDG state directory.
- Track stable result IDs, use count, last-used time, and query prefixes.
- Boost frequently selected apps/projects for matching queries, similar to the user's ArcMenu expectation and Ulauncher-style previous-choice learning.
- Add settings to disable learning and clear learned state.
- Add alias/quick-link support for common URLs, files, folders, and commands.
- Keep calculator rows deterministic and above app/project rows for math-like queries.
- Document the ranking formula before implementation.

Phase 8 implements the conservative local learned-ranking foundation plus aliases/quick links. Ranking persists `ranking.toml` under the XDG state directory, records successful app/folder launches, applies a bounded prefix-gated boost, and adds learning on/off and clear-history controls. Aliases support URLs, files, folders, and explicit no-shell commands, with provider toggle and row editing in settings.

## Phase 9 - Refactoring And Internal Boundaries

Goal: make the current codebase easier to package, test, and extend without changing user-visible behavior.

- Split large implementation files in reviewable slices, starting with `apps.rs`, `search.rs`, and `main.rs`.
- Extract desktop-entry parsing, app discovery, and icon lookup before deeper freedesktop compatibility work.
- Extract search result types and matching/provider orchestration before aliases or optional providers broaden the result model.
- Add crate-level integration test directories and reusable fixtures for desktop entries, icon themes, config, project folders, and learned ranking state.
- Keep small parser/helper tests inline where that remains clearer.
- Preserve config/state compatibility, launch semantics, ranking behavior, and the current UI flow while moving code.
- Split the Slint UI into component files only if the build remains straightforward.

This phase turns [REFACTORING.md](REFACTORING.md) from a general cleanup note into planned public-readiness work. The intent is not a rewrite or plugin system. It is a set of internal boundaries that reduce packaging risk and make later desktop-entry, icon-theme, aliases, optional providers, and CI work easier to verify.

Phase 9 implementation is complete. Core app handling, icon lookup, calculator internals, core search, UI callback/helper code, and Slint settings/result components have been split. Broad action, calculator, config, desktop-entry, icon lookup, ranking, project search, mixed-search, and desktop app behavior now has crate-level integration coverage with reusable fixtures.

## Phase 10 - Packaging And Linux Integration

Goal: make public installation reliable across common distributions.

- Create a source-of-truth packaging inventory for binary name, app ID, desktop entry name, icon name, metainfo ID, config path, state path, cache path, and runtime socket path.
- Add AppStream/metainfo metadata.
- Validate desktop entry and AppStream metadata in CI.
- Implement one primary public package path first, with Flatpak as the strongest candidate for broad Linux distribution.
- Add complete Fedora RPM and Arch/AUR packaging after the metadata and install layout are stable.
- Revisit AppImage after the normal install layout, desktop entry, and icon model are validated.
- Improve standards compliance for desktop discovery and icon lookup before packaging broadly.

Phase 10 implementation is complete for the current public-readiness pass. The repo now has a packaging inventory, AppStream/metainfo metadata, a metadata validation script, CI validation, a Flatpak prototype manifest, Fedora RPM spec, Arch/AUR `PKGBUILD`, and an AppImage deferral note. Desktop-entry discovery now parses localized labels, keywords, desktop actions, and `DBusActivatable`, while icon lookup covers more app icon directory shapes and WPS-style reverse-DNS suffix fallbacks.

## Phase 11 - Project Maturity

Goal: make the project maintainable by contributors and future agents.

- Add CI for `cargo fmt`, `cargo clippy`, `cargo test`, `cargo build`, and package metadata validation. This is implemented in `.github/workflows/ci.yml`.
- Keep the cleanup backlog in [CLEANUP_AUDIT.md](CLEANUP_AUDIT.md) visible alongside feature work.
- Add release notes/changelog process.
- Add contribution docs after the public package path is chosen.
- Add manual verification matrix for GNOME/KDE, Wayland/X11, and major distro families.
- Keep [TESTING.md](TESTING.md) and [REFACTORING.md](REFACTORING.md) current as the codebase evolves beyond the initial split.

## Phase 12 - Optional Provider Expansion

Goal: grow useful capabilities without making the launcher heavy by default.

- Add explicit web searches with URL templates.
- Add unit conversion if it can stay local and testable.
- Add currency conversion with a free/no-key rate source if it can stay fast, cacheable, and explicit.
- Add time lookup with a free/no-key place and timezone source if it can stay explicit and cacheable.
- Consider clipboard history and snippets only as disabled-by-default providers with clear storage and clear-history controls.
- Consider script providers only after command execution policy is documented.
- Consider window switching only after a reliable cross-desktop strategy is chosen.

The first Phase 12 provider pass implements default-enabled explicit default-browser `search`, configurable additional search engines, local unit conversion, currency conversion through Frankfurter pair rates, `time in <place>` lookup through Open-Meteo place/timezone metadata, and built-in reboot/shutdown/logout plus timer/reminder commands. Alias and additional search-engine rows can be edited in Settings while TOML editing remains supported. Unit conversion is local/tested, and network-backed utility providers cache fetched metadata in the resident process.

## Phase 13 - Installable Module Ecosystem

Goal: complete the migration from virtual bundled modules to optional, verified packages that community authors can build against a stable API.

- Keep only Apps, Folders, the module manager/runtime integration, ranking, config/state, IPC, and UI infrastructure in a fresh core install.
- Build the signed static registry, local package manager, complete Modules catalog UI, author SDK/docs, and optional sandboxed WASM host.
- Extract Calculator, Units, Currency, Time, Web Search, Timers, and Aliases into independently released official repositories.
- Install no optional module on a fresh configuration and provide a confirmed one-time path for existing virtual-module users.
- Complete integrity, permission, rollback, revocation, offline, adversarial, packaging, performance, and Linux matrix verification.

The authoritative owner steps, implementation phases, required information, and acceptance checklist are in [manual_migration.md](manual_migration.md).
