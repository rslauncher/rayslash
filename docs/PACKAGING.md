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

The GitHub Actions workflow in [../.github/workflows/ci.yml](../.github/workflows/ci.yml) runs formatting, clippy, tests, build, desktop-entry validation, AppStream validation, inventory consistency checks, and frozen/offline Fedora rebuilds on x86_64 and aarch64. Each Fedora job fetches the architecture-matched host RPM from the immutable host v0.1.2 release, verifies its pinned checksum, runs RPM digest and rpmlint validation, and proves with a DNF dry run that installing `rayslash` resolves the separate host dependency.

[release.yml](../.github/workflows/release.yml) can run without publishing from a branch for pre-release verification. Semantic-version tags run the same native x86_64 and ARM64 RPM, DEB, AppImage, and Flatpak builds, verify that Cargo, RPM, and Arch versions match the tag, check the expected ten user-facing binaries, create a single `SHA256SUMS`, and publish the GitHub release. Source RPMs, debuginfo/debugsource packages, per-file checksum sidecars, and transient build products remain CI-internal instead of cluttering the public release.

## Standards To Follow

- Desktop files should follow the freedesktop Desktop Entry Specification.
- App icons should follow the freedesktop Icon Theme Specification and install into hicolor.
- Config, state, cache, data, and runtime paths should follow the XDG Base Directory Specification.
- Public Linux app distribution should include AppStream/metainfo metadata.
- Sandboxed builds, especially Flatpak, should be checked for portal and desktop integration constraints before release.

## Public Distribution Strategy

GitHub releases provide Fedora 44 RPM, DEB, AppImage, and Flatpak downloads for x86_64 and ARM64. RPM keeps the separately maintained host as a package dependency and publishes the matching host RPM beside the app RPM. DEB, AppImage, and Flatpak embed the digest-pinned host so each is a single app download. No format bundles optional modules.

Arch/AUR packaging remains available as a source recipe. The Flatpak manifest lives at:

```sh
packaging/flatpak/dev.rayan6ms.rayslash.yml
```

The direct Flatpak bundle is deliberately not presented as a Flathub submission: a general desktop launcher needs read access to host application metadata and configured folders, plus `org.freedesktop.Flatpak` access to execute selected host applications. Those permissions are explicit in the manifest and should be reconsidered if a narrower launcher portal becomes available.

## Fedora

Fedora RPM packaging lives at:

```sh
packaging/fedora/rayslash.spec
```

The spec builds the Rust workspace and installs the UI crate binary as `rayslash`. Fedora/mock builds have no network access, so the SRPM contains a deterministic vendor archive generated from the committed `Cargo.lock`; the generated dependency tree is not committed to Git.

Known build requirements discovered during local development:

- `gcc`
- `pkgconfig(fontconfig)`

Rust and Cargo are required to build the project. The current spec keeps the build direct and should be adjusted to Fedora Rust packaging conventions if submitted to Fedora proper.

### Preparing an offline Fedora SRPM

Run the source-preparation helper from a clean checkout. It accepts an output directory and an optional committed Git reference:

```sh
sources_dir="$(mktemp -d)"
packaging/fedora/prepare-sources.sh "$sources_dir" HEAD
```

The helper runs `cargo vendor --locked --versioned-dirs`, creates `rayslash-0.2.0.tar.gz` from the selected commit, and creates a deterministic `rayslash-0.2.0-vendor.tar.xz`. It prints both SHA-256 hashes. Network access is allowed only during this source-preparation step so Cargo can populate missing registry packages. `Cargo.lock` remains authoritative and source preparation fails if its dependency graph cannot be vendored.

Build the SRPM from a literal copy of the checked-in spec in a fresh top directory:

```sh
topdir="$(mktemp -d)"
mkdir -p "$topdir"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
cp "$sources_dir"/* "$topdir/SOURCES/"
cp packaging/fedora/rayslash.spec "$topdir/SPECS/"
rpmbuild -bs \
  --define "_topdir $topdir" \
  "$topdir/SPECS/rayslash.spec"
```

Do not write `rpmspec -P` output into `SPECS`; preprocessing expands machine-specific paths. The literal spec plus explicit `_topdir` keeps the SRPM portable.

Rebuild in a clean Fedora 44 chroot:

```sh
resultdir="$(mktemp -d)"
mock \
  -r fedora-44-x86_64 \
  --resultdir="$resultdir" \
  --rebuild "$topdir/SRPMS/rayslash-0.2.0-1.fc44.src.rpm"
```

The spec installs `packaging/fedora/cargo-config.toml`, which replaces crates.io with the unpacked `vendor` directory and enables Cargo offline mode. Both `%build` and `%check` use `--frozen`, so a missing/stale vendor entry or lockfile change fails instead of accessing the registry. They cap Cargo at two jobs because clean Slint builds can otherwise run enough concurrent compiler processes to exhaust a 16 GiB workstation. `%check` uses the release profile so it reuses the packaged build's optimized dependency graph instead of compiling the full Slint stack a second time in the debug profile.

Mock 6.7 on Fedora 44 may log repeated `unknown tag: "pkgid"` messages while its `package_state` plugin runs an RPM query containing `%{pkgid}`. This occurs before project build commands and comes from `/usr/lib/python3.14/site-packages/mockbuild/plugins/package_state.py`; RPM 6.0.1 no longer recognizes that query tag. It is harmless mock/plugin compatibility noise, not a rayslash spec or source error.

Expected install outputs:

- Binary: `/usr/bin/rayslash`
- Desktop entry: `/usr/share/applications/dev.rayan6ms.rayslash.desktop`
- Icon: `/usr/share/icons/hicolor/scalable/apps/dev.rayan6ms.rayslash.svg`
- Required module host: `/usr/libexec/rayslash/rayslash-module-host`, supplied by the `rayslash-module-host` dependency.

The desktop entry should keep `Exec=rayslash toggle` unless the runtime model changes.

The package installs AppStream/metainfo metadata.

### Official Fedora package set

The [Rayslash v0.2.0 release](https://github.com/rslauncher/rayslash/releases/tag/v0.2.0) publishes the app RPMs together with the verified, separately packaged host RPM for x86_64 and aarch64. The same host RPMs remain independently available from the [host v0.1.2 release](https://github.com/rslauncher/rayslash-module-host/releases/tag/v0.1.2). No optional module is included in either RPM.

Download the app RPM, the matching host RPM, and `SHA256SUMS`. Verify them in the download directory and install both official files in one DNF transaction:

```sh
sha256sum --check --ignore-missing SHA256SUMS
sudo dnf install \
  ./rayslash-module-host-0.1.2-1.fc44."$(uname -m)".rpm \
  ./rayslash-0.2.0-1.fc44."$(uname -m)".rpm
```

This installs the host as a dependency-owned infrastructure package. It does not install Calculator, Units, Currency, Time, Web Search, Timers, Aliases, or any community module.

## Arch/AUR

Arch/AUR packaging lives at:

```sh
packaging/arch/PKGBUILD
```

The `PKGBUILD` builds the Rust workspace and installs the resulting binary as `rayslash`.

`pkgver` is `0.2.0` and `pkgrel` is `1`.

Expected package behavior:

- Install the `rayslash` binary into `/usr/bin/rayslash`.
- Install `packaging/linux/dev.rayan6ms.rayslash.desktop` into `/usr/share/applications/dev.rayan6ms.rayslash.desktop`.
- Install `icons/rayslash-icon.svg` into `/usr/share/icons/hicolor/scalable/apps/dev.rayan6ms.rayslash.svg`.
- Preserve `Exec=rayslash toggle` so desktop-managed shortcuts and desktop metadata use the resident toggle path.
- Install AppStream/metainfo metadata.
- Require `rayslash-module-host`, supplied directly or through the `rayslash-module-host-bin` provider.

## DEB

`packaging/debian/build-deb.sh` creates an architecture-native Debian package containing the launcher, module host, desktop entry, icon, metainfo, license, and install notes. The release workflow builds on Ubuntu 22.04 and validates both package metadata and installed paths with `dpkg-deb`.

Install a release package with:

```sh
sudo apt install ./rayslash_0.2.0_amd64.deb
```

## Flatpak

Flatpak packaging lives at `packaging/flatpak/dev.rayan6ms.rayslash.yml`.

The manifest installs the pinned host release at `/app/libexec/rayslash/rayslash-module-host`. It contains no official or community module package.

The sandbox shares network access because the app must fetch the signed catalog and user-selected package assets, and because reviewed modules such as Currency and Time use narrowly allowlisted HTTPS services through the host. It reads host desktop entries/icons and executes selected external actions through `flatpak-spawn --host`. Module-level origin checks still apply; Flatpak network access does not grant a guest module ambient sockets.

Install and invoke the direct bundle with:

```sh
flatpak install --user ./rayslash-0.2.0-x86_64.flatpak
flatpak run dev.rayan6ms.rayslash toggle
```

## AppImage

`packaging/appimage/build-appimage.sh` assembles the standard install layout in an AppDir and invokes a digest-pinned linuxdeploy build. The bundled `AppRun` forwards arguments and resolves the included module host. Invoke it with:

```sh
chmod +x rayslash-0.2.0-x86_64.AppImage
./rayslash-0.2.0-x86_64.AppImage toggle
```

AppImages are explicitly download-and-replace artifacts; no silent updater or zsync metadata is published. See [APPIMAGE.md](APPIMAGE.md).
