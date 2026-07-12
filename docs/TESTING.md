# Testing Strategy

The current tests are split between focused inline Rust unit tests and crate-level integration tests with reusable fixtures. That keeps private helper coverage close to implementation while moving public behavior and file-format compatibility out of large source files.

See [REFACTORING.md](REFACTORING.md) for the planned Phase 9 split between inline helper tests, crate-level integration tests, reusable fixtures, and larger module cleanup.

## Current State

- `rayslash-core` has inline tests only for narrow private search/path helpers.
- `rayslash-ui` has inline tests for private CLI parsing, IPC helpers, settings parsing, opener visuals, selected-index/search helper behavior, and extensionless icon format detection.
- `crates/rayslash-core/tests/` covers core Apps/Folders behavior, configuration and version-1 module migration, signed registry and revocation verification, safe package paths, typed action boundaries, desktop discovery, icons, projects, ranking, and an opt-in live installer → host → Calculator integration probe. Extracted official provider logic is tested in each module repository rather than compiled into the app.
- `rayslash-ui` inline tests cover settings parsing for folder sources, aliases, additional search-engine rows, max results, theme/density, effective active-search queries, opener visuals, selected-index/search helper behavior, and extensionless icon format detection.
- `crates/rayslash-core/tests/fixtures/` now has small reusable temp-dir, desktop-entry, icon-theme, config, project/app, and learned-ranking fixture helpers.
- There are no UI crate integration tests yet.
- Manual UI verification is still required for Slint behavior, window focus, scrolling, icon rendering, and desktop shortcut behavior. Use [UI_VERIFICATION.md](UI_VERIFICATION.md) for the current real-desktop checklist.

## Target Layout

Keep small pure unit tests next to the implementation they exercise. Move broader behavior into crate-local integration test directories:

```text
crates/rayslash-core/tests/
crates/rayslash-core/tests/fixtures/
crates/rayslash-ui/tests/
packaging/tests/
```

Rust integration tests should live under the crate they test because the workspace root is not itself a package.

## What Stays Inline

- Tiny private parser/helper cases.
- Narrow path formatting and subtitle helper tests where private helper access is the point of the test.
- UI-crate private helper tests unless the UI crate grows a library API.

## What Moves To Integration Tests

- Config loading with real temporary config files.
- Calculator behavior through `calc::calculate`.
- Action command construction and spawn behavior.
- Desktop entry parsing through the public `apps` API.
- Desktop app discovery from temporary XDG data directories.
- Icon lookup with fixture themes and hicolor fallbacks.
- Project scanning from temporary folder roots.
- Mixed search behavior across apps, projects, calculator, aliases, and learned ranking.
- Usage-history persistence and migration.
- IPC socket behavior where it does not require a real desktop session.
- Public command behavior for `rayslash` and `rayslash toggle`.

## Fixture Plan

Existing core fixtures cover the current refactor scope:

- Temporary directories.
- Valid/invalid `.desktop` file contents.
- Minimal hicolor app icon paths.
- Config shapes.
- Project and app rows.
- Learned-ranking state with repeated launches.

Add more fixtures later for:

- Full icon themes with `index.theme`, inheritance, hicolor fallback, scalable icons, symbolic directories, scaled directories, and fixed-size PNG/SVG examples.
- Config files for future schema versions.
- Default web search and additional search-engine template shapes.
- Unit and currency conversion edge cases.
- Learned-ranking migration files.

Fixtures should be small and synthetic. Do not depend on the host user's real installed apps or icon themes for automated tests.

## CI Checks

Baseline checks:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
packaging/validate-metadata.sh
```

Performance diagnostic probes are opt-in and ignored by default:

```sh
cargo test -p rayslash-core --test performance -- --ignored --nocapture
RAYSLASH_PROFILE=1 cargo run -p rayslash
```

Use them when investigating search, result conversion, model replacement, or UI refresh latency. They are not CI pass/fail checks because timing thresholds vary by machine and desktop session.

Record comparable results in [PERFORMANCE.md](PERFORMANCE.md).

Packaging checks to add when files exist:

```sh
desktop-file-validate packaging/linux/dev.rayan6ms.rayslash.desktop
appstreamcli validate --no-net packaging/linux/dev.rayan6ms.rayslash.metainfo.xml
```

Release checks should also verify that package manifests install the same binary name, app ID, desktop entry name, icon name, and metainfo ID.

The GitHub Actions workflow at `.github/workflows/ci.yml` runs the baseline Rust checks plus `packaging/validate-metadata.sh`.

## Manual Verification Matrix

Manual checks remain necessary for desktop behavior:

- GNOME Wayland.
- GNOME X11 when available.
- KDE Plasma Wayland.
- KDE Plasma X11 when available.
- Fedora, Arch, Ubuntu/Debian, and openSUSE packaging environments as packaging broadens.

Manual scenarios:

- Run the [UI_VERIFICATION.md](UI_VERIFICATION.md) checklist for result scrolling, hover selection, settings autosave, focus loss, and icon rendering.
- `rayslash toggle` starts, shows, hides, and resets correctly.
- Desktop shortcut binding opens the installed binary, not `cargo run`.
- Click-outside hiding behaves on the tested desktop.
- Window icon and panel matching work after installing desktop entry and hicolor icon.
- Long result lists scroll correctly.
- App icons render and fall back cleanly.
- `RAYSLASH_PROFILE=1` timing output is useful and quiet when disabled.
