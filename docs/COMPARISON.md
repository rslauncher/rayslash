# Launcher Comparison

This document records product lessons from adjacent launchers and command palettes. It is planning input, not a requirement to clone every feature.

## Summary

`rayslash` already covers the smallest useful launcher loop: show quickly, search apps/projects/calculations, activate the selected result, and hide. The next public steps should focus on configurability, learned ranking, a pre-packaging refactoring pass, packaging reliability, and a few carefully chosen optional providers.

The most relevant benchmark is Ulauncher: it is Linux-native, supports fuzzy app search, remembers previous choices, exposes shortcuts and extensions, supports custom themes, and has a file browser mode. Albert and KRunner show the mature version of the same idea: providers/plugins are independently configurable and can expose actions, syntax, and settings. Raycast and Alfred are not Linux targets, but they are useful product references for aliases, action panels, snippets, clipboard history, and privacy-sensitive feature toggles.

## Comparison Matrix

| Tool | Relevant strengths | Lessons for rayslash |
| --- | --- | --- |
| [Ulauncher](https://ulauncher.io/) | Fuzzy Linux app search, previous-pick learning, custom themes, shortcuts, extensions, directory browser. | Add usage-based ranking, aliases/quick links, theme settings, and a user-facing preferences surface before attempting a broad plugin ecosystem. |
| [Albert](https://albertlauncher.github.io/) | Fast plugin-based launcher with curated plugins and plugin APIs. | Model future feature areas as providers that can be enabled, disabled, configured, and ranked independently. Keep bundled providers curated. |
| [KRunner](https://develop.kde.org/docs/plasma/krunner/) | Runtime runner plugins that return matches and actions from a query string, with metadata for runners. | Treat each provider as a typed search source with its own settings, actions, IDs, and performance boundaries. |
| [ArcMenu](https://extensions.gnome.org/extension/3628/arcmenu/) | GNOME app menu with multiple layouts, GNOME search integration, system shortcuts, settings-heavy customization. | Make customization approachable, but do not turn rayslash into a full app menu replacement. Use the user's ArcMenu result-learning expectation as a product target. |
| [rofi](https://davatorium.github.io/rofi/1.7.3/rofi.1/) | Minimal fast launcher, multiple modes, combi mode, script/dmenu integration, deep theme/config surface. | Keep the UI low-distraction and consider a later script provider for power users, but avoid shell execution in core activation paths by default. |
| [Raycast](https://manual.raycast.com/command-aliases-and-hotkeys) | Aliases, hotkeys, root search, command settings, and an [action panel](https://manual.raycast.com/action-panel). | Add predictable aliases and a settings entry point. A small action panel can scale better than overloading modifier shortcuts. |
| [Alfred](https://www.alfredapp.com/help/workflows/) | Workflows, [clipboard history](https://www.alfredapp.com/help/features/clipboard/), snippets, and custom web searches. | Clipboard/snippet features should be opt-in, visibly configurable, and easy to clear because they store sensitive user data. |

## Feature Implications

### Settings and customization

The first settings UI is a Settings button in the header, replacing the previous `preview` text on the same line as the icon and title. Settings should stay small and practical:

- Folder sources.
- Alternate folder opener command.
- Enabled providers: apps, folders, and calculator first; aliases/quick links and future optional providers only after those providers exist.
- Appearance: result count first; theme and density later.
- Search learning: enabled/disabled, clear learned history.
- Diagnostics: config path, state path, socket path, app count, project count, profile toggle instructions.

Settings should write the same TOML config that power users can edit by hand.

### Learned ranking

The search result order should learn from selected results. This is the clearest parity gap with mature launchers. Ulauncher explicitly documents remembering previous picks; the user's ArcMenu example points to the same expectation.

Implemented first-pass model:

- Store learned state under the XDG state directory, separate from config.
- Key records by stable result ID, not just display title.
- Track launch count, last launched time, and query prefixes that led to successful app/folder launches.
- Apply a bounded boost after the deterministic fuzzy score only for low-risk title-prefix matches, so strong textual matches and calculator rows still behave predictably.
- Provide settings to disable learning and clear learned state.

Still deferred:

- Aliases and quick links.
- Decay or cleanup of stale entries.
- Broader provider-specific ranking controls.

### Provider model

The current code already has implicit providers: calculator, desktop apps, projects, placeholders/no-results. Public configurability will be easier if those become explicit provider concepts in docs and eventually in code.

Provider requirements:

- Stable provider ID.
- Enabled/disabled state.
- Provider-specific config.
- Result IDs stable enough for ranking history.
- Primary and secondary actions.
- Optional provider health/diagnostic status.

This does not require a third-party plugin API immediately. A public plugin API should wait until the internal provider boundary is stable.

### Optional feature candidates

Good near-term candidates:

- Aliases and quick links for URLs, files, folders, and commands.
- Richer alternate folder opener action templates beyond the current command picker.
- Unit conversion if it can stay local, deterministic, and testable; currency and time lookup if the network/API dependency is explicit, trigger-based, and configurable.
- Recent/frequent result section for empty queries.
- Configurable web searches with explicit URL templates.

Good later candidates:

- Clipboard history and snippets, disabled by default.
- Script provider for advanced users, with explicit opt-in and clear security boundaries.
- Window switching, only after choosing a reliable cross-desktop approach.
- File browser mode, if project search is not enough.

Features to avoid for now:

- Full extension marketplace before provider APIs, settings, and security policy are mature.
- In-app global shortcut capture on Wayland.
- Shell-based activation of `.desktop` commands.
- Privacy-sensitive history collection without an obvious setting and clear storage path.

## Packaging and Standards Inputs

The packaging direction should use standards as source of truth rather than hardcoded local assumptions:

- [Desktop Entry Specification](https://specifications.freedesktop.org/desktop-entry/latest/recognized-keys.html): app name, `Exec`, `Icon`, `NoDisplay`, `Hidden`, `TryExec`, `OnlyShowIn`, `NotShowIn`, `DBusActivatable`, keywords, and desktop actions.
- [Icon Theme Specification](https://specifications.freedesktop.org/icon-theme/latest/): icon base directories, theme inheritance, hicolor fallback, accepted formats, and lookup order.
- [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir/): config, data, state, cache, and runtime paths.
- [Flatpak conventions](https://docs.flatpak.org/en/latest/conventions.html): reverse-DNS app ID, desktop/appstream/icon conventions for broad Linux distribution.
- [Flatpak desktop integration](https://docs.flatpak.org/en/latest/desktop-integration.html): portal-aware integration expectations for sandboxed distribution.

## Product Positioning

`rayslash` should stay a small native Linux launcher, not become a complete desktop shell. The public value proposition should be:

- Fast native keyboard launcher.
- Linux desktop app search that respects standards.
- Project launcher for developers.
- Calculator and small utilities.
- Configurable enough for different users.
- Predictable and private by default.
