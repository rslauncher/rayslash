<!-- markdownlint-disable MD033 MD041 -->
<h1 align="center">rayslash</h1>

<p align="center">
  <img src="./icons/rayslash-icon.svg" alt="rayslash icon" width="128" height="128">
</p>

<p align="center">
  <a href="https://github.com/rslauncher/rayslash/releases">
    <img src="https://img.shields.io/github/downloads/rslauncher/rayslash/total?style=flat-square" alt="Total downloads">
  </a>
  <a href="https://github.com/rslauncher/rayslash/stargazers">
    <img src="https://img.shields.io/github/stars/rslauncher/rayslash?style=flat-square" alt="GitHub stars">
  </a>
  <a href="https://github.com/rslauncher/rayslash/releases/latest">
    <img src="https://img.shields.io/github/v/release/rslauncher/rayslash?display_name=tag&sort=semver&style=flat-square" alt="Latest release">
  </a>
</p>

<p align="center">
  rayslash is a lightweight, keyboard-first application launcher for Linux. It uses a compact native <a href="https://slint.dev/">Slint</a> interface and works on both Wayland and X11 without compositor-specific integrations.
</p>
<!-- markdownlint-enable MD033 MD041 -->

## Features

- Search and launch installed desktop applications
- Find folders from configurable source directories
- Install Calculator, Units, Currency, Time, Web Search, Timers, Aliases, and community modules only when wanted
- Verify a signed registry and every package digest before activation
- Run executable modules without WASI in an automatically delivered sandbox host
- Learn from app and folder selections using local ranking data
- Configure providers, appearance, aliases, and search engines from the settings panel

## Installation

Building requires a recent Rust toolchain and the Fontconfig development files for your distribution.

```sh
git clone https://github.com/rslauncher/rayslash.git
cd rayslash
packaging/install-user.sh
```

This one command builds the launcher and installs the digest-pinned module host. Make sure Cargo's binary directory, usually `~/.cargo/bin`, is on your `PATH`, then start rayslash:

```sh
rayslash
```

For regular use, bind a desktop shortcut such as `Super+\` to:

```sh
rayslash toggle
```

Global shortcuts are managed by the desktop environment rather than captured by rayslash. A desktop entry and icon are available under [`packaging/linux`](packaging/linux) for local or package installations.

The app installs the [`rayslash-module-host`](https://github.com/rslauncher/rayslash-module-host) runtime so Settings → Modules can install and run modules immediately. The host is infrastructure, not a module: no official or community module is installed until you choose it. Fedora and Arch express the separately maintained host package as a required dependency, and the Flatpak includes the host executable.

## Usage

Start typing to search applications and configured folders. After installing the corresponding modules, queries include:

```text
2 * (3 + 4)
10 km to mi
25 brl to usd
time in Sao Paulo
search rust slint
timer 10min take a break
```

Use the arrow keys or `Tab` to select a result, `Enter` to open it, `Ctrl+Enter` to use the alternate folder opener, and `Escape` to hide the launcher.

## Configuration

Settings can be changed from the launcher or by editing:

```text
~/.config/rayslash/config.toml
```

Folder sources, module-owned alias/web-search settings, appearance, and ranking behavior are configurable. Module configuration is stored separately in `~/.config/rayslash/modules.toml`; packages, state, and caches follow the XDG base directory conventions.

## Modules

Module authors can start with the [SDK quickstart](https://github.com/rslauncher/rayslash-module-sdk/blob/main/docs/AUTHORING.md). The SDK contains the stable WIT contract, manifest schema, validator/packager, and release template. Community submissions are pull requests to the [signed registry](https://github.com/rslauncher/rayslash-registry); no paid service or custom backend is required.

## Development

Run the project from the workspace with:

```sh
cargo run -p rayslash
```

The main checks used by CI are:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
packaging/validate-metadata.sh
```

## License

[MIT](LICENSE)
