# Install

These notes cover release packages, local development runs, a user install from the current checkout, and local desktop entry setup. See [PACKAGING.md](PACKAGING.md) for reproducible build details.

## Release Packages

The [latest GitHub release](https://github.com/rslauncher/rayslash/releases/latest) provides x86_64 and ARM64 RPM, DEB, AppImage, and Flatpak downloads plus one `SHA256SUMS` manifest.

Verify a downloaded file from the directory containing `SHA256SUMS`:

```sh
sha256sum --check --ignore-missing SHA256SUMS
```

- Fedora: download the architecture-matched `rayslash` and `rayslash-module-host` RPMs, then install both in one `dnf install ./...` transaction.
- Debian/Ubuntu: install the matching `.deb` with `sudo apt install ./rayslash_*.deb`. The module host is included.
- AppImage: mark it executable and bind the full AppImage path plus `toggle` to the desktop shortcut. The module host is included.
- Flatpak: install the matching bundle with `flatpak install --user ./rayslash-*.flatpak`, then use `flatpak run dev.rayan6ms.rayslash toggle` for the shortcut. The module host is included.

## Development Runs

Start the launcher from the workspace:

```sh
cargo run -p rayslash
```

Send a toggle request from the workspace:

```sh
cargo run -p rayslash -- toggle
```

These commands are useful while developing, but desktop shortcuts should not use `cargo run`.

## Local User Install

Install the UI crate and the required, digest-pinned module host:

```sh
packaging/install-user.sh
```

The script installs `rayslash` into Cargo's user binary directory, usually `~/.cargo/bin`, and the host into `~/.local/libexec/rayslash`. It does not install any official or community module. Cargo's binary directory must be on your `PATH` for desktop shortcuts and shell commands to find `rayslash`.

## Verify PATH

Confirm your shell can find the installed binary:

```sh
command -v rayslash
```

Then confirm the installed command can start or contact the resident launcher:

```sh
rayslash toggle
```

After this works, bind your desktop shortcut to the installed command:

```sh
rayslash toggle
```

Shortcut setup steps live in [SHORTCUTS.md](SHORTCUTS.md).

## Local Desktop Entry

The repository includes a desktop entry template at:

```sh
packaging/linux/dev.rayan6ms.rayslash.desktop
```

Install it for the current user with:

```sh
mkdir -p ~/.local/share/applications
cp packaging/linux/dev.rayan6ms.rayslash.desktop ~/.local/share/applications/
```

Install the local icon with the matching desktop icon name:

```sh
mkdir -p ~/.local/share/icons/hicolor/scalable/apps
cp icons/rayslash-icon.svg ~/.local/share/icons/hicolor/scalable/apps/dev.rayan6ms.rayslash.svg
```

If the cache refresh tools are available, refresh the user icon and desktop databases:

```sh
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache ~/.local/share/icons/hicolor
command -v update-desktop-database >/dev/null && update-desktop-database ~/.local/share/applications
```

The desktop entry uses `Exec=rayslash toggle`, `Icon=dev.rayan6ms.rayslash`, `StartupNotify=false`, and `StartupWMClass=dev.rayan6ms.rayslash`, matching the resident toggle-style launcher model and desktop panel identity. It also uses `NoDisplay=true` because `rayslash` is primarily opened through a custom keyboard shortcut rather than from app menus.

Your custom keyboard shortcut should still point directly to:

```sh
rayslash toggle
```

## Packaging

Linux packaging notes and metadata validation commands live in [PACKAGING.md](PACKAGING.md). Release tags build and publish RPM, DEB, AppImage, and Flatpak artifacts for x86_64 and ARM64. Arch/AUR metadata remains available for source-based packaging.
