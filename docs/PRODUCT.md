# Product

`rayslash` is a lightweight Linux application launcher focused on opening desktop applications, finding project folders, and running small utility actions from a fast keyboard-first UI.

## Target User

The current target user is a Linux desktop user on GNOME, KDE, Fedora, Arch, or similar environments who wants a fast launcher without Electron, Tauri, or compositor-specific integrations.

The public target should broaden this slightly:

- Users who want a small native launcher that respects Linux desktop standards.
- Developers who want project search and configurable editor actions.
- Power users who want aliases, quick links, optional providers, and predictable local state.
- Users who need enough settings to adapt the launcher without rebuilding from source.

## Main Goals

- Open quickly from a desktop-managed shortcut.
- Search and launch desktop applications.
- Search configured project folders.
- Open project folders in a configurable editor or the system file manager.
- Keep calculator and small utility actions deterministic and testable.
- Learn from selected results while preserving predictable base ranking.
- Keep privacy-sensitive features opt-in and easy to clear.
- Use a small centered UI that feels native enough on Wayland and X11.
- Package cleanly across common Linux distributions.

## Current Feature Set

- Cargo workspace with separate core and UI crates.
- Slint desktop UI with a compact launcher panel.
- Resident process and `rayslash toggle` IPC.
- Desktop app discovery through `.desktop` entries.
- Best-effort app icon lookup.
- Project folder search.
- Safe calculator expressions and linear equations.
- Default-browser web search and configurable additional search engines.
- Local unit conversion, currency conversion, and time lookup.
- Built-in reboot, shutdown, logout, timer, and reminder commands.
- Keyboard and mouse activation.
- Local install docs, desktop shortcut docs, and packaging notes.

## Product Principles

- Prefer standards and explicit config over user-machine assumptions.
- Keep the UI fast, small, and keyboard-first.
- Keep core search/action logic testable outside the UI.
- Do not shell out for desktop entry activation unless a future feature explicitly opts into command execution.
- Make optional features easy to enable, disable, and explain.
- Treat learned ranking, clipboard history, snippets, and command providers as user-owned local data.

## Non-Goals For The Next Public Cycle

- No in-app global shortcut capture on Wayland.
- No compositor-specific overlay APIs unless a specific desktop integration problem proves unsolvable with the normal window model.
- No Electron, Tauri, or WebView shell.
- No third-party plugin marketplace before the internal provider model, settings, and security boundaries are mature.
- No privacy-sensitive history collection without a visible setting and clear storage path.

## Planning References

- Launcher comparison and product lessons: [COMPARISON.md](COMPARISON.md).
- Post-v1 roadmap: [ROADMAP.md](ROADMAP.md).
- Concrete tasks: [TASKS.md](TASKS.md).
- Pre-packaging refactoring plan: [REFACTORING.md](REFACTORING.md).
- Cleanup audit and maintenance backlog: [CLEANUP_AUDIT.md](CLEANUP_AUDIT.md).
