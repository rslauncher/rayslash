# UI Verification

Use this checklist when changing Slint layout, result interaction, settings autosave, focus behavior, icon loading, or packaging metadata. These checks require a real desktop session and complement the Rust test suite.

Record the desktop environment, session type, distro, rayslash commit, and date before testing.

## Setup

Run the development binary from the repository:

```sh
RAYSLASH_PROFILE=1 cargo run -p rayslash
cargo run -p rayslash -- toggle
```

Use a config with at least one folder source containing more folders than fit in the result viewport, and verify that at least a few desktop apps have visible icons.

## Result Scrolling

1. Open rayslash with an empty query.
2. Use Down until selection reaches the last visible row.
3. Continue pressing Down through several off-screen rows.
4. Use Up back through the list.

Pass criteria:

- The hint/status row stays visible.
- The selected row remains fully visible during keyboard navigation.
- The result list scrolls inside the viewport and does not resize the launcher.
- No text overlaps or clips in a way that changes row height.

## Hover Selection

1. Scroll until the bottom row is partially clipped.
2. Move the pointer over only the clipped part of that partial row.
3. Confirm the list does not scroll as a result of that hover.
4. Move the pointer over a fully visible row.
5. Click the clipped partial row.

Pass criteria:

- Hovering the clipped partial row selects/highlights it.
- Hovering a clipped row at the top or bottom edge does not scroll the viewport.
- Hovering a fully visible row selects it.
- Clicking the clipped partial row still activates that clicked row intentionally.

## Settings Autosave

1. Open Settings.
2. Toggle a provider setting.
3. Change Max results and press Enter.
4. Change the alternate folder opener command and move focus away.
5. Change folder sources with the folder picker.
6. Scroll to the diagnostics at the bottom of Settings.
7. Switch Theme between Dark, Dim, and Light with the segmented control.
8. Switch Density between Comfort and Compact with the segmented control.
9. Hover each provider toggle.
10. Hover a toggle row for at least 800ms.
11. Disable Tooltips and hover a toggle row again.
12. Open Modules and toggle Web Search, Units, Currency, Time, Calculator, Timers, and Aliases.
13. Confirm each module card uses its result-style glyph, including `U` for Units and `$` for Currency.
14. Search Modules by name, description, author, category, and a mixed-case query.
15. Enter a query with no matches on Official and Community and confirm a compact “No modules found” result appears without replacing the tab helper line.
16. Try Name (A–Z), Name (Z–A), and Most starred sorting, then apply Updates only and Saved data filters.
17. Remove an installed module without deleting its data and confirm it remains in Installed with a single primary Restore action.
18. On that removed module, choose Delete data once, verify the warning, then use Cancel or choose Confirm delete to finish.
19. Open Source code and Report issues and confirm they open the repository and issue tracker respectively.
20. Scroll to the final module card and confirm the viewport stops at its bottom edge without a large blank tail.
21. Switch to Modules, press Escape, and confirm rayslash returns to the main launcher just as it does from General.
22. Select text in the launcher search, module search, folder source, alternate opener, Max results, alias, and search-engine inputs in both light and dark themes.
23. Install an older module version from a test catalog, refresh to a catalog containing a newer compatible version, and use Update.
24. Open the new search-engine editor, fill one field, switch to another window to copy a URL, and return to rayslash.
25. Cancel that editor and confirm no row was added, then create a YouTube engine using `https://www.youtube.com/results?search_query=%s`.
26. Edit the YouTube engine, disable and re-enable it, then delete it from the editor.
27. Add an alias, fill part of the editor, switch to another window, and return to rayslash.
28. Cancel that editor and confirm no row was added, then add a complete URL alias.
29. Edit the alias, switch its type, save it, and delete it from the editor.
30. Remove Web Search and Aliases while retaining their data, then reinstall them.

Pass criteria:

- Each valid change saves without closing Settings.
- Settings content scrolls to the diagnostics and does not push outside the launcher panel.
- Diagnostics uses one bordered card: config/state locations share equal aligned columns above a divider, while Folders, Apps, Icons, and the runtime socket use aligned cells below it without overlap or irregular gaps.
- Toggle label/description text remains readable, and hovering the full toggle row for at least 800ms shows the full label/description detail above other settings content.
- Disabling Tooltips suppresses delayed detail tooltips for settings toggles and result rows.
- Max results is a compact control in the Launcher content row next to Apps and Folders; Tooltips sits next to Alt opener.
- Theme and density read as mutually exclusive segmented controls, not unrelated buttons.
- Module toggles fit without text overlap, save immediately, and use provider-specific result glyphs.
- Module search is case-insensitive, sorting and filters immediately change the visible list, and the controls remain aligned at the normal launcher size.
- The module toolbar has visible padding around both rows, and the search, sort, and filter controls do not touch the tab row or outer border.
- Empty tabs and searches with no matches show a compact “No modules found” result while retaining the contextual helper copy above the toolbar.
- Removed modules with retained data stay in Installed. Restore occupies the same primary action slot as Install, while permanent data deletion is separate and requires confirmation.
- Normal module cards omit the capabilities row; compact action buttons end at the Source code/Report issues text baseline rather than extending beneath it.
- Module scrolling ends with the final visible card, and hidden or filtered cards do not add blank scroll range.
- Operation feedback appears as a temporary bottom notification rather than changing failed card heights; success uses a green check, errors use a red indicator, and in-progress warnings use orange. Short messages remain for at least 4.2 seconds and longer messages receive additional reading time.
- Escape closes Settings from both General and Modules, including after using the module search field.
- Source code uses the normal link color; Report issues uses the distinct issue color and opens the repository issue tracker.
- Zero-star modules keep a muted star, while positive star counts use the highlighted star color.
- Text selection uses neutral, theme-aware foreground/background colors instead of the toolkit accent color.
- Update shows the installed and target versions, verifies the signed package metadata and digest, probes the replacement before committing it, preserves the module's enabled state, and leaves the old version active if installation fails.
- Alias and search-engine Add/Edit actions use compact in-window editors; switching applications leaves them open with entered values intact.
- Cancel does not persist a new row, invalid values keep the editor open, and Delete appears only when editing a removable existing engine.
- Alias list rows show an icon, name and target, centered keyword/type flairs, and a centered Edit button; Delete is available only inside the editor.
- Search-engine list rows show a compact favicon container, name and URL, mildly rounded keyword flair, centered switch, and centered Edit button without a Delete or warning action.
- Search-engine switches are vertically centered and use the same sharp track/knob proportions as normal feature switches.
- After the YouTube favicon is fetched, it appears in the settings card and the `Search YouTube for...` result; the keyword fallback remains available if fetching fails.
- Alias and search-engine editors fit inside the settings panel and reject invalid rows with a clear status message instead of overwriting config.
- Alias and search-engine configuration sections are visible only while their modules are installed. Removing a module with retained data hides its section; reinstalling it restores the saved rows.
- Light mode keeps header, search, result rows, settings fields, toggles, and diagnostics readable.
- Invalid Max results and empty enabled alternate opener values show a clear status message and do not save.
- Successful saves refresh the current result count and diagnostics.
- Existing `config.toml` files get timestamped `config.toml.backup-...` siblings before replacement.
- If `config.toml` has a parse error before startup, settings saves are blocked until the file is fixed and rayslash is restarted.

## Web Search

1. Type a query that does not match a local app or folder.
2. Confirm it shows `No results` instead of a default web search row.
3. Type `search manhattan`.
4. Activate the default web search row.
5. Type `search`, press Space, then type search terms.
6. Clear the search terms and press Backspace.
7. Add an enabled YouTube engine with keyword `yt` and URL `https://www.youtube.com/results?search_query=%s`.
8. Type `yt`, press Space, then type search terms.
9. Clear the search terms and press Backspace.
10. Type `yt`, press Tab, then type search terms.
11. Disable the `yt` row in Settings and repeat the keyword trigger.

Pass criteria:

- Default web search appears only for the built-in `search` command or active `Search` pill.
- `Web Search` renders its configured `%s` URL and opens it through the desktop default browser; changing its URL affects every browser family.
- Space and Tab turn an enabled keyword into a compact pill before the typed search terms.
- The custom search result shows the cached favicon when available, retains the keyword fallback when unavailable, and opens the configured URL with percent-encoded terms.
- Backspace on an empty active custom search clears the pill.
- Disabled custom engines do not trigger a pill or a custom search row.

## Focus Loss

1. Open rayslash.
2. Click another application window.
3. Open Settings and use the folder picker.
4. Cancel the folder picker, then click another application window.
5. Open the search-engine editor, switch to another application, then return and cancel the editor.

Pass criteria:

- Ordinary focus loss hides the launcher.
- Opening the folder picker does not permanently break focus-loss hiding.
- After the picker closes, ordinary focus loss hides the launcher again.
- Focus loss does not hide rayslash while the search-engine editor is open; explicit Save, Cancel, or Delete closes the editor.

## Resident Toggle Latency

1. Start rayslash once with `RAYSLASH_PROFILE=1 target/release/rayslash toggle`.
2. Hide it with `target/release/rayslash toggle`.
3. Confirm the original process and `$XDG_RUNTIME_DIR/rayslash.sock` remain alive.
4. Show and hide it several more times through `Super+\`.

Pass criteria:

- Empty startup/show queries do not launch optional module hosts.
- Hiding the launcher does not terminate the resident process.
- After the first start, shortcut toggles show and hide through IPC without visible startup delay.

## Icon Rendering

1. Search for desktop apps with SVG, PNG, named theme icons, and missing icons if available.
2. Toggle Settings open to refresh desktop app discovery.
3. Install or remove a desktop app icon if testing a packaging or icon-theme change, then open Settings or restart rayslash.

Pass criteria:

- Supported app icons render at row size without stretching the layout.
- Missing icons use the fallback app icon cleanly.
- WPS 2019-style icons such as `com.wps.Office.kprometheus` resolve when a theme provides a matching suffix icon such as `wps-office2019-kprometheus`.
- Alternate folder opener picker icons match the refreshed app list.
- Ctrl-held folder rows show either the configured opener app icon or compact command fallback without text overlap.
- Newly discovered apps show a compact `New` flair after the app name without changing row height or overlapping long app names.

## Result Cap Tip

1. Set Max results to a value lower than the number of available results, such as `36`.
2. Open rayslash with an empty query or a broad query.
3. Scroll to the final visible row.
4. Move selection with the keyboard at the bottom of the capped list.

Pass criteria:

- The real results stop at the configured cap.
- A separate non-selectable tip after the last real result states the active max result count and only appears after scrolling to the end.
- Keyboard navigation stops at the last real result and never selects the max-results tip.
- Keyboard navigation and scrolling still keep selected result rows visible.

## Result Detail Tooltips

1. Hover a folder result row for at least 800ms.
2. Hover an app result row with an app description for at least 800ms.
3. Search for a query with no matches and hover the `No results` row for at least 800ms.
4. Disable Tooltips in Settings and repeat the hover.
5. Move the pointer away from the row.

Pass criteria:

- Folder path and app description tooltips appear above row content after the delay when hovering anywhere on the result row.
- No-results detail appears after the delay while the visible row subtitle remains short.
- The tooltip width fits the displayed detail text, subject to the launcher width, and long tooltip text wraps to no more than three visible lines.
- Disabling Tooltips suppresses result detail tooltips.
- Moving away from the row hides the detail tooltip.
