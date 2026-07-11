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
- Evaluate calculations and linear equations
- Convert units and currencies, and look up local times
- Open quick links, files, folders, and explicit commands through aliases
- Search the web with configurable keyword triggers
- Run timers, reminders, and common system actions
- Learn from app and folder selections using local ranking data
- Configure providers, appearance, aliases, and search engines from the settings panel

## Installation

Building requires a recent Rust toolchain and the Fontconfig development files for your distribution.

```sh
git clone https://github.com/rslauncher/rayslash.git
cd rayslash
cargo install --path crates/rayslash-ui
```

Make sure Cargo's binary directory, usually `~/.cargo/bin`, is on your `PATH`, then start rayslash:

```sh
rayslash
```

For regular use, bind a desktop shortcut such as `Super+\` to:

```sh
rayslash toggle
```

Global shortcuts are managed by the desktop environment rather than captured by rayslash. A desktop entry and icon are available under [`packaging/linux`](packaging/linux) for local or package installations.

## Usage

Start typing to search applications, folders, and aliases. The built-in tools use explicit queries such as:

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

Folder sources, aliases, web searches, enabled providers, appearance, and ranking behavior are configurable. User state and cached data follow the XDG base directory conventions.

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
