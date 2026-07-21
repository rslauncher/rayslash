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
14. Open the new search-engine editor, fill one field, switch to another window to copy a URL, and return to rayslash.
15. Cancel that editor and confirm no row was added, then create a YouTube engine using `https://www.youtube.com/results?search_query=%s`.
16. Edit the YouTube engine, disable and re-enable it, then delete it from the editor.
17. Add, edit, and delete an alias row using `name | keyword | kind | target`.

Pass criteria:

- Each valid change saves without closing Settings.
- Settings content scrolls to the diagnostics and does not push outside the launcher panel.
- Toggle label/description text remains readable, and hovering the full toggle row for at least 800ms shows the full label/description detail above other settings content.
- Disabling Tooltips suppresses delayed detail tooltips for settings toggles and result rows.
- Tooltips sits next to Alt opener, and Max results appears after those toggles before Appearance.
- Theme and density read as mutually exclusive segmented controls, not unrelated buttons.
- Module toggles fit without text overlap, save immediately, and use provider-specific result glyphs.
- Add and Edit use the same compact in-window editor; switching applications leaves it open with entered values intact.
- Cancel does not persist a new row, invalid values keep the editor open, and Delete appears only when editing a removable existing engine.
- Search-engine list rows show a compact favicon container, name and URL, mildly rounded keyword flair, centered switch, and centered Edit button without a Delete or warning action.
- Search-engine switches are vertically centered and use the same sharp track/knob proportions as normal feature switches.
- After the YouTube favicon is fetched, it appears in the settings card and the `Search YouTube for...` result; the keyword fallback remains available if fetching fails.
- Alias fields and the search-engine editor fit inside the settings panel and reject invalid rows with a clear status message instead of overwriting config.
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
