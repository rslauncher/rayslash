# Install

These notes cover local development runs, a simple user install from the current checkout, and local desktop entry setup. Distro package implementations are still future work.

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

Install the UI crate as the `rayslash` binary:

```sh
cargo install --path crates/rayslash-ui
```

Cargo installs the binary into Cargo's user binary directory, usually `~/.cargo/bin`. That directory must be on your `PATH` for desktop shortcuts and shell commands to find `rayslash`.

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

Linux packaging notes and metadata validation commands live in [PACKAGING.md](PACKAGING.md). Current package artifacts include Fedora RPM packaging, an Arch/AUR `PKGBUILD`, a Flatpak prototype manifest, AppStream/metainfo metadata, and an AppImage deferral note.
