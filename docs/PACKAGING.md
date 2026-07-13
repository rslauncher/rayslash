# Packaging Notes

These notes record expected Linux packaging behavior and the current package metadata files.

## Shared Expectations

- The installed binary should be named `rayslash`.
- Desktop shortcuts should invoke:

```sh
rayslash toggle
```

- The desktop entry template lives at:

```sh
packaging/linux/dev.rayan6ms.rayslash.desktop
```

- The packaging inventory lives at:

```sh
packaging/linux/inventory.toml
```

- AppStream/metainfo metadata lives at:

```sh
packaging/linux/dev.rayan6ms.rayslash.metainfo.xml
```

- The desktop app ID is `dev.rayan6ms.rayslash`.
- The desktop entry uses `Icon=dev.rayan6ms.rayslash`.
- The desktop entry uses `StartupWMClass=dev.rayan6ms.rayslash` for X11-style panel matching.
- The icon source lives at `icons/rayslash-icon.svg`.
- The desktop entry uses `NoDisplay=true` because `rayslash` is primarily shortcut-driven. It should be available to desktop databases and packaging metadata without adding a mostly redundant app-menu item. Users or packages can remove `NoDisplay=true` later if a visible menu launcher becomes useful.

## Source Of Truth

Public packaging should avoid duplicating identity values across files by hand. The source-of-truth packaging inventory is [../packaging/linux/inventory.toml](../packaging/linux/inventory.toml) and records:

- Binary name: `rayslash`
- App ID: `dev.rayan6ms.rayslash`
- Desktop entry file: `dev.rayan6ms.rayslash.desktop`
- Icon name: `dev.rayan6ms.rayslash`
- AppStream/metainfo ID: `dev.rayan6ms.rayslash`
- Config directory: `~/.config/rayslash`
- State directory: `~/.local/state/rayslash`
- Cache directory: `~/.cache/rayslash`
- Runtime socket: `$XDG_RUNTIME_DIR/rayslash.sock`, with a user-specific temp fallback when `XDG_RUNTIME_DIR` is unavailable.

Packaging files are checked against that inventory by:

```sh
packaging/validate-metadata.sh
```

When the tools are installed, the same script also runs:

```sh
desktop-file-validate packaging/linux/dev.rayan6ms.rayslash.desktop
appstreamcli validate --no-net packaging/linux/dev.rayan6ms.rayslash.metainfo.xml
```

The GitHub Actions workflow in [../.github/workflows/ci.yml](../.github/workflows/ci.yml) runs formatting, clippy, tests, build, desktop-entry validation, AppStream validation, and inventory consistency checks.

## Standards To Follow

- Desktop files should follow the freedesktop Desktop Entry Specification.
- App icons should follow the freedesktop Icon Theme Specification and install into hicolor.
- Config, state, cache, data, and runtime paths should follow the XDG Base Directory Specification.
- Public Linux app distribution should include AppStream/metainfo metadata.
- Sandboxed builds, especially Flatpak, should be checked for portal and desktop integration constraints before release.

## Public Distribution Strategy

The first public package targets are Fedora RPM and Arch/AUR on x86_64 and aarch64. Their app packages require the separately maintained host package, so module installation needs no additional user setup. Flatpak remains a prototype and bundles the digest-pinned host executable. A local prototype manifest lives at:

```sh
packaging/flatpak/dev.rayan6ms.rayslash.yml
```

The manifest is intentionally a prototype because host desktop-entry discovery and launching host applications from a sandbox still need real testing.

Suggested order:

1. Stabilize install layout, app ID, desktop entry, icon, and metainfo.
2. Add validation for desktop entry and AppStream metadata.
3. Build and verify Fedora RPM packaging in `packaging/fedora/rayslash.spec`.
4. Build and verify Arch/AUR packaging in `packaging/arch/PKGBUILD`.
5. Evaluate and harden the Flatpak prototype and bundled-host boundary.
6. Keep AppImage deferred until update, desktop integration, and shortcut documentation expectations are clear.

This order can change if the project chooses distro-native packages as the first public path.

## Fedora

Fedora RPM packaging lives at:

```sh
packaging/fedora/rayslash.spec
```

The spec builds the Rust workspace and installs the UI crate binary as `rayslash`.

Known build requirements discovered during local development:

- `gcc`
- `pkgconfig(fontconfig)`

Rust and Cargo are required to build the project. The current spec keeps the build direct and should be adjusted to Fedora Rust packaging conventions if submitted to Fedora proper.

Expected install outputs:

- Binary: `/usr/bin/rayslash`
- Desktop entry: `/usr/share/applications/dev.rayan6ms.rayslash.desktop`
- Icon: `/usr/share/icons/hicolor/scalable/apps/dev.rayan6ms.rayslash.svg`
- Required module host: `/usr/libexec/rayslash/rayslash-module-host`, supplied by the `rayslash-module-host` dependency.

The desktop entry should keep `Exec=rayslash toggle` unless the runtime model changes.

The package installs AppStream/metainfo metadata.

## Arch/AUR

Arch/AUR packaging lives at:

```sh
packaging/arch/PKGBUILD
```

The `PKGBUILD` builds the Rust workspace and installs the resulting binary as `rayslash`.

Expected package behavior:

- Install the `rayslash` binary into `/usr/bin/rayslash`.
- Install `packaging/linux/dev.rayan6ms.rayslash.desktop` into `/usr/share/applications/dev.rayan6ms.rayslash.desktop`.
- Install `icons/rayslash-icon.svg` into `/usr/share/icons/hicolor/scalable/apps/dev.rayan6ms.rayslash.svg`.
- Preserve `Exec=rayslash toggle` so desktop-managed shortcuts and desktop metadata use the resident toggle path.
- Install AppStream/metainfo metadata.
- Require `rayslash-module-host`, supplied directly or through the `rayslash-module-host-bin` provider.

## Flatpak

Flatpak packaging has a prototype manifest at `packaging/flatpak/dev.rayan6ms.rayslash.yml`. It should be evaluated before broad public release because it can provide a single distribution path across many Linux distros.

The manifest installs the pinned host release at `/app/libexec/rayslash/rayslash-module-host`. It contains no official or community module package.

Open questions:

- Whether the resident `rayslash toggle` model works cleanly with the Flatpak command wrapper.
- How users should bind desktop shortcuts to the Flatpak command.
- Whether clipboard access through `arboard` behaves as expected in the sandbox.
- Whether app discovery should inspect host desktop entries from inside Flatpak, use portals, or require a different package model.
- Whether launching host apps from inside a sandbox is acceptable or too limited for this launcher.

Flatpak may reveal that distro-native packages are a better first public target. That decision should be based on a prototype, not assumptions.

## AppImage

AppImage packaging is deferred. Do not build or publish an AppImage from this pass. The current revisit note lives at:

```sh
packaging/appimage/README.md
```

A future AppImage should still expose the `rayslash` command internally and should document how users bind their desktop shortcut to the AppImage equivalent of:

```sh
rayslash toggle
```
