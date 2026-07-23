# Performance

This is the canonical performance baseline and optimization backlog for rayslash. The current measurements were taken on 2026-07-22. Keep historical results, but do not compare numbers unless the scenario, build profile, machine, data set, and metric boundary are equivalent.

## Executive summary

The resident architecture is working: a warm shortcut client connects, writes its request, and exits in 3.47 ms median. A module-free empty startup does not start Wasmtime. Core search is also healthy: even an intentionally oversized 5,000-item catalog completes its slowest tested query in 5.21 ms median in release mode.

The largest user-visible costs are outside core fuzzy matching:

1. Any enabled installed module causes **every** non-empty query, including app and folder queries, to wait for a 150 ms debounce. Results are then collected by a 24 ms polling timer. The effective local-result floor is therefore approximately 150–174 ms even though the underlying work is usually below 6 ms.
2. A cold query across all seven official modules takes 1.388 s. Calculator alone takes 1.341 s median to start, compile, handshake, and answer. The same seven-host fan-out is only 0.580 ms median once warm.
3. `time in Tokyo` takes 908 ms median even with a warm host because the Time module performs an uncached network lookup for every query.
4. Desktop discovery takes 54.0 ms median in the standalone probe and 56.3 ms median during measured startup. Initial result-icon conversion takes another 22.6 ms median. Startup reaches the event-loop boundary in 129.4 ms median, but the first redraw is not requested until 242.5 ms median. First-frame work is therefore a real bottleneck hidden by the old marker.
5. App activation takes 9.17 ms median before the target app can begin opening, principally because it launches several sequential `wmctrl` processes. A raw process spawn is only 0.177 ms median.
6. Seven warm module hosts use about 135 MiB RSS in addition to the launcher. The launcher measured 104 MiB RSS, 54.6 MiB PSS, with no modules in the isolated run.

The first optimization should be a two-phase result pipeline: publish local Apps/Folders results immediately, route only relevant modules, and merge module/network results later. That removes more perceived latency than micro-optimizing the matcher.

## Performance targets

These are proposed product budgets, not current guarantees.

| Interaction | Target | Current state |
| --- | ---: | ---: |
| Resident shortcut client exits | p95 under 10 ms | 3.64 ms p95 |
| Cold launcher ready for event loop | p95 under 150 ms | 137.6 ms p95 |
| Cold launcher first redraw requested | p95 under 150 ms | Fails: 247.8 ms p95 |
| Local result update after a keystroke | p95 under 16 ms | Compute is under budget; module policy imposes 150–174 ms |
| Warm local module result | p95 under 16 ms | Seven-module fan-out 0.655 ms p95, before UI polling |
| First cold module result | p95 under 250 ms | Fails: full fan-out 1.388 s; Calculator 1.359 s p95 |
| Cached network-backed result | p95 under 50 ms | Currency passes after cache; Time has no cache |
| Uncached network result | p95 under 1 s with progressive status | Time 0.927 s p95 in a five-sample run, one 1.143 s outlier |
| Local action dispatch | p95 under 10 ms | App activation 9.34 ms p95; raw spawn 0.218 ms |

“Ready for event loop” means `ui.show()` has returned and the resident is about to enter Slint's event loop. It is not proof that the compositor has painted the first frame. The profiler now also records the first redraw request; first completed presentation still needs dedicated instrumentation.

The roughly 113 ms median gap from event-loop readiness to first redraw request is not yet subdivided. The empty query supplies 36 result rows, and `result_list.slint` uses a repeater over the complete model with a visually complex row and decoded app icons. A smaller initial model, visible-row virtualization, and deferred icon texture work are high-value A/B tests, but the audit does not yet prove which part dominates that gap.

## Measurement environment

| Item | Value |
| --- | --- |
| Date | 2026-07-22 |
| OS/session | Fedora Linux 44, GNOME, Wayland |
| Kernel | 7.1.4-200.fc44.x86_64 |
| CPU | Intel Xeon E5-2697 v3, 14 cores / 28 threads, 2.60 GHz base |
| Memory | 15 GiB |
| Filesystem | Btrfs |
| Rust | rustc 1.92.0, Cargo 1.92.0 |
| cargo-component | 0.21.1 |
| Launcher build | `cargo build --release --locked` |
| Module build | release components targeting `wasm32-unknown-unknown` |
| Live catalog | 75 apps, 26 folders, 1 configured folder root |
| Synthetic catalog | 4,000 apps and 1,000 folders |

Unless noted otherwise, distributions exclude warm-up iterations and report min, p50, p95, max, and arithmetic mean from `Instant`/monotonic-clock samples. Release results are the user-facing baseline. Debug results are diagnostic only.

## Current results

### Startup and resident activation

Ten isolated resident starts used the real app/folder configuration, isolated runtime/state/cache directories, no installed modules, and warm OS filesystem caches. First-redraw numbers came from a second ten-run series with the same setup.

| Stage | p50 | p95 | Notes |
| --- | ---: | ---: | --- |
| Backend selection and app ID | 29.51 ms | 30.46 ms | Slint/winit backend setup |
| UI construction | 3.45 ms | 4.60 ms | Approximate from ten samples |
| Config load | 0.64 ms | 0.72 ms | Includes runtime module config setup |
| Project scan | 0.057 ms | 0.065 ms | 26 folders |
| Desktop app discovery | 56.28 ms | 61.14 ms | 75 apps, including icon resolution |
| Initial core search | 0.213 ms | 0.272 ms | Empty query |
| Initial result-item/icon build | 22.60 ms | 22.93 ms | 36 rows; first icon decode |
| Callbacks and IPC ready | 119.4 ms | 127.6 ms | Cumulative |
| `ui.show()` call | 9.99 ms | 10.27 ms | Does not guarantee completed presentation |
| Ready for event loop | **129.4 ms** | **137.6 ms** | Cumulative; mean 131.5 ms |
| First redraw requested | **242.5 ms** | **247.8 ms** | Cumulative; mean 246.2 ms; one 279.0 ms max |

A separate empty-data isolated run reached the event loop in 130.8 ms and used:

| Memory metric | Value |
| --- | ---: |
| RSS | 106,448 KiB (104.0 MiB) |
| PSS | 55,873 KiB (54.6 MiB) |
| Anonymous RSS | 27,472 KiB |
| Private clean + dirty | 48,108 KiB (47.0 MiB) |
| Threads | 17 |

The launcher RSS includes shared UI, graphics, font, and system libraries; PSS is a better approximation of the launcher's apportioned physical cost. Heap allocation and GPU memory were not separated in this audit.

The warm IPC client was measured against a minimal local Unix-socket server so the number includes fresh process creation, dynamic loading, CLI parsing, `connect`, write, and exit, but excludes UI event-loop handling:

```text
samples=200 min=3.239ms p50=3.467ms p95=3.642ms max=3.735ms mean=3.469ms
```

The prior `<0.01s` observation was directionally correct but too coarse to serve as a regression baseline.

Ten alternating requests against a real isolated resident showed an empty-query reset itself at roughly 1.0–1.3 ms and show requests queued-to-handled at roughly 1.9–2.15 ms in the non-outlier samples. Adding the 3.47 ms median client process gives roughly 5.5 ms to the end of the UI handler, excluding the following redraw/presentation. Hide was generally below 0.5 ms after warm-up, with one 4.91 ms outlier.

### Catalog construction

| Operation | Samples | p50 | p95 | Mean |
| --- | ---: | ---: | ---: | ---: |
| Desktop discovery, 75 apps | 20 | 54.04 ms | 57.21 ms | 54.63 ms |
| Folder discovery, 1 root / 26 children | 100 | 0.018 ms | 0.029 ms | 0.019 ms |

Desktop discovery currently repeats filesystem traversal, desktop-file parsing, locale selection, executable availability checks, icon-theme discovery, a `gsettings` subprocess, and icon path probing. It is also called synchronously when Settings is opened after the ten-second freshness interval, so it can create a roughly 54 ms UI-thread stall outside startup.

### Core search

The release probe warms each query 20 times, then measures 200 repetitions over 5,000 synthetic items.

| Query | Results | p50 | p95 | Mean |
| --- | ---: | ---: | ---: | ---: |
| Empty | 5,000 | 3.572 ms | 3.705 ms | 3.572 ms |
| `app 39` | 355 | 5.208 ms | 5.390 ms | 5.220 ms |
| `editor` | 1,334 | 3.189 ms | 4.009 ms | 3.258 ms |
| `project 42` | 28 | 1.456 ms | 3.037 ms | 1.596 ms |
| `999 * 42` | 1 | 1.034 ms | 1.369 ms | 1.085 ms |

This is healthy for a catalog far larger than the measured live catalog. It measures core result construction, fuzzy matching, learned boost, and sorting. It does not include modules, Slint conversion, icon decoding, model replacement, paint, debounce, or polling.

### Module runtime

Each cold sample starts a new production module-host process, performs the protocol handshake, and runs the first query. Each warm sample reuses one persistent host. Network modules use a non-triggering query in the main table to isolate runtime overhead.

| Module | WASM bytes | Cold p50 | Cold p95 | Warm p50 | Warm p95 | Warm-host RSS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Calculator | 1,075,385 | 1.341 s | 1.359 s | 0.130 ms | 0.173 ms | 33,300 KiB |
| Units | 91,891 | 129.8 ms | 131.2 ms | 0.052 ms | 0.075 ms | 15,608 KiB |
| Time, non-trigger | 1,095,852 | 264.6 ms | 269.3 ms | 0.041 ms | 0.062 ms | 20,668 KiB |
| Timers | 56,300 | 90.7 ms | 92.8 ms | 0.048 ms | 0.067 ms | 15,236 KiB |
| Currency, non-trigger | 167,879 | 226.7 ms | 229.7 ms | 0.040 ms | 0.062 ms | 18,440 KiB |
| Web Search | 137,916 | 197.9 ms | 203.3 ms | 0.047 ms | 0.064 ms | 17,360 KiB |
| Aliases | 146,612 | 205.0 ms | 208.6 ms | 0.079 ms | 0.106 ms | 17,908 KiB |

All seven production paths queried in parallel:

```text
cold: 1.388s
warm: samples=200 min=0.525ms p50=0.580ms p95=0.655ms max=0.913ms mean=0.588ms
```

The seven persistent host RSS values total 138,520 KiB (135.3 MiB). RSS double-counts pages that processes share, so a future audit should also collect summed PSS before setting a hard memory target.

Network-backed trigger measurements:

| Scenario | Result |
| --- | --- |
| Time `time in Tokyo`, cold host + live network | p50 1.167 s, 3 samples |
| Time `time in Tokyo`, warm host + live network | p50 0.908 s, p95 0.927 s, max 1.143 s, 5 samples |
| Currency `25 BRL to USD`, cold/cache-mixed | p50 229 ms, max 409 ms, 3 samples |
| Currency `25 BRL to USD`, warm host + cached rate | p50 0.068 ms, p95 0.087 ms, 20 samples |

The Time result is service/network dependent and should not be compared across dates as a CPU regression. It does prove that repeated identical queries receive no module-level cache benefit. Currency demonstrates the desired cached behavior.

### Query scheduling and time to result

The current UI does not publish local results first. `query_execution_hint` returns a 150 ms debounced-network hint when **any** installed module is enabled, without considering the query or module trigger. The callback clones config, ranking, app state, folders, and apps, starts a thread, sleeps 150 ms, queries core and every enabled module, sends through a channel, and waits for a 24 ms repeated UI timer to notice it.

Derived end-to-end floors before rendering are therefore:

| State | Approximate time after last keystroke |
| --- | ---: |
| No installed module, live-sized local catalog | Usually below a frame; exact end-to-end paint not yet captured |
| Any module enabled, warm hosts, local app query | 150–174 ms + search/model work |
| Seven modules enabled, first cold query | About 1.54–1.56 s |
| Warm Time live lookup | About 1.06–1.08 s median |

These are derived from the explicit debounce, polling interval, and measured work. They are not compositor presentation measurements.

Rapid typing also creates one sleeping OS thread and clones the catalogs for each query generation. Old generations are discarded after waking, but the allocations, clones, and threads have already occurred.

### Actions

| Operation | Samples | p50 | p95 | Meaning |
| --- | ---: | ---: | ---: | --- |
| Spawn `true` | 100 | 0.177 ms | 0.218 ms | Process spawn acknowledgement only |
| Activate synthetic app | 20 | 9.173 ms | 9.339 ms | Failed focus probes plus spawn acknowledgement |
| One unsuccessful `wmctrl` probe | 100 | 2.094 ms | External process measurement |
| `xdg-settings get default-web-browser` | 100 | 18.922 ms | Paid synchronously on each default web-search action |

Normal app activation can run up to four sequential `wmctrl` attempts (`StartupWMClass`, desktop ID, ID without `.desktop`, and app name) before spawning the target. This explains the activation result. The launcher later waits 250 ms and probes again in a detached thread; that does not delay the initial spawn but adds process churn.

Module URL/path/command actions call `spawn_command_checked`, which polls a child for up to 150 ms in 10 ms intervals before considering it successfully detached. This path needs a direct distribution test with representative `xdg-open`, `notify-send`, and approved commands; structurally, its worst-case acknowledgement is already 150 ms.

App-state and learned-ranking TOML updates are serialized and atomically written synchronously on the UI thread after a successful activation and before hiding. Their current files are small, but this should be moved off the interaction-critical path.

### Artifact size

| Artifact | Bytes | Approximate size |
| --- | ---: | ---: |
| Launcher | 39,330,040 | 37.5 MiB |
| Module host | 14,666,000 | 14.0 MiB |
| All seven module components | 2,771,835 | 2.64 MiB |

The current launcher is 2,101,496 bytes larger than the 2026-07-12 migrated-core artifact. This audit did not bisect the growth. The workspace has no explicit release profile for LTO, symbol stripping, panic strategy, or codegen units.

## Ranked optimization roadmap

Ordering considers both observed need and plausible user-visible improvement. Estimates must be remeasured after implementation.

### P0 — largest measured latency wins

#### 1. Publish local results immediately; merge modules later

- Run Apps/Folders synchronously or on a dedicated low-latency worker and update the model immediately.
- Route module results as a second phase. Preserve generation IDs so stale module responses cannot replace newer local results.
- Apply the 150 ms debounce only to a module that can actually make a network request for the current query.
- Expected effect: ordinary app/folder time-to-result falls from a derived 150–174 ms floor to roughly the actual compute/model cost, likely below 10 ms on the live catalog.
- Risk: define stable merge, selection, and exclusivity behavior so a late exclusive result does not make the selection jump unexpectedly.

#### 2. Cut first-redraw work

- Do not populate and render 36 empty-query rows before the first frame. Start with a small recent/favorite set, a bounded visible slice, or an empty lightweight shell and fill it immediately after first presentation.
- Virtualize result rows so the UI creates only the viewport and a small overscan, rather than one complex component per model item.
- Defer non-visible icon decoding/upload and alternate-opener model/icon construction until idle or Settings is opened.
- Add sub-stage timing around event-loop entry, row instantiation, font/layout, renderer setup, redraw completion, and compositor presentation before choosing the final design.
- Expected effect: attack the measured 113 ms median event-loop-to-first-redraw gap and move cold visible response from 242.5 ms toward the 150 ms target.

#### 3. Remove cold Wasmtime compilation from the first relevant query

- Enable and validate Wasmtime's on-disk compilation cache, or distribute safely precompiled artifacts tied to the exact host/Wasmtime/CPU compatibility contract.
- Consider idle prewarming after the window is responsive, prioritizing Calculator. Do not block cold UI startup on all modules.
- Reduce Calculator and Time component code size with LTO, `opt-level = "z"`/`"s"` experiments, symbol stripping, and `wasm-opt` where component compatibility is preserved.
- Expected effect: seven-module first query from 1.388 s toward warm execution; Calculator offers the largest single win.
- Tradeoff: eager prewarming spends CPU and memory even if the user never invokes a module.

#### 4. Cache Time lookups and route them by trigger

- Cache normalized place query → place/timezone results with a TTL; compute the displayed clock locally from the cached timezone.
- Coalesce identical in-flight requests and retain stale data for offline fallback, with age visible in diagnostics if needed.
- Query Time only for `time in …`, never for arbitrary app-search text.
- Expected effect: repeated identical lookup from about 908 ms median to warm local-module latency; fewer requests while typing.

### P1 — startup, scheduling, action, and memory improvements

#### 5. Cache the desktop catalog and make discovery incremental

- Persist a versioned desktop index with source path, size/mtime, parsed fields, availability, and icon resolution.
- Load the last valid index before showing the window, then reconcile changes in the background.
- Watch application directories or debounce refresh events instead of rescanning on Settings open.
- Cache locale preferences, `PATH` availability by executable, current desktop, configured icon theme, theme inheritance, and negative icon lookups once per scan.
- Resolve/decode only visible result icons; do not eagerly construct all alternate-opener images before first interaction.
- Expected upper bound: remove most of the measured 56 ms discovery and 23 ms initial icon cost from the blocking startup path; prevent the ~54 ms Settings-open stall.

#### 6. Replace timer polling and per-keystroke threads with one scheduler

- Deliver completed work directly to the Slint event loop instead of polling every 24 ms.
- Use one cancellable debounce timer/worker, not one sleeping OS thread per query.
- Keep immutable catalogs behind `Arc` and pass a query/generation rather than cloning every app and folder on each keystroke.
- Expected effect: remove 0–24 ms (about 12 ms average) after completed background work and reduce typing burst overhead.

#### 7. Reduce module resident memory without restoring cold-query latency

- Prototype one host process with one shared Wasmtime `Engine` and isolated per-module stores/linkers/capabilities. Measure fault containment and permission isolation before adopting it.
- Alternative: keep process isolation but reap least-recently-used hosts after an idle timeout and retain compiled-code cache so restart is cheap.
- Record aggregate PSS, not only RSS, and set a budget for 0, 1, and 7 enabled modules.
- Current need: seven hosts total 135 MiB RSS, more than the launcher's 104 MiB RSS.

#### 8. Remove synchronous helper processes from action dispatch

- Query the window list once and match all class/name candidates locally instead of starting up to four `wmctrl` processes.
- On Wayland environments where `wmctrl` cannot achieve the desired focus behavior, skip it or use a desktop-appropriate activation path; validate GNOME and KDE separately.
- Cache the default-browser desktop ID and invalidate it on settings change rather than paying 18.9 ms per web action.
- Spawn module actions promptly and reap/check them asynchronously instead of waiting up to 150 ms on the UI thread.
- Move ranking/app-state persistence after hide or onto a serialized state writer.

### P2 — scaling and footprint improvements

#### 9. Cache module runtime metadata

- Keep parsed installed state, module config, verified revocations, manifests, permissions, and settings JSON in a runtime snapshot.
- Update the snapshot only on install/remove/enable/config/catalog events.
- Reuse fixed workers instead of spawning scoped fan-out threads for every query.
- Need is modest today: full warm seven-module fan-out is already 0.580 ms median. Do this after the scheduling changes.

#### 10. Bound core search work for very large catalogs

- Precompute normalized/UTF-32 searchable fields when the catalog changes.
- Retain only the best `max_results` candidates with a top-k structure instead of constructing and sorting every match.
- Avoid lowercasing titles repeatedly during tie-breaking and learned-prefix checks.
- Consider prefix/token indexes only after testing 10k–100k catalogs; the 5k probe is already under 5.4 ms p95.

#### 11. Tune release artifacts

- Benchmark `lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`, and `panic = "abort"` independently and together.
- Audit Slint, image, networking, and platform feature sets for unused code before removing features.
- Compare file size, clean build time, startup, symbolication/debuggability, and packaging behavior. Do not accept size-only changes that regress startup or diagnostics.

#### 12. Bound config-backup and write amplification

- The measured user config directory contained 184 `config.toml.backup-*` files.
- Debounce autosaves, skip semantically identical writes, keep a small rotation, and move backup cleanup off the UI-critical path.
- Batch state updates through one writer to avoid repeated TOML serialization, directory sync work, and unbounded file accumulation.

### P3 — measurement gaps and lower-priority polish

- Record input event → local model update → redraw requested → compositor-presented frame. The last boundary requires platform-aware frame presentation instrumentation or an external high-speed capture.
- Add representative action probes for `xdg-open`, `notify-send`, terminal launch, DBus activation, clipboard creation, Flatpak `flatpak-spawn --host`, and failed/fallback paths.
- Measure cold page-cache startup, AppImage, Flatpak, RPM/DEB installs, x86_64 and ARM64, GNOME and KDE, and slower storage.
- Measure module-host aggregate PSS, heap allocation, GPU memory, CPU time, wakeups while hidden, and power use.
- Add catalog sizes at 100, 1k, 5k, 10k, and 100k, plus long localized strings and missing icons.
- Test slow, hung, crashing, and oversized-output modules. The current five-second per-module deadline is a correctness guard but is far above an interactive latency budget.
- Make registry/favicons/install progress event-driven and verify background work never contends with first show.
- Add benchmark result export (JSON/CSV) and compare p50/p95 against a stored environment-qualified baseline. Avoid brittle pass/fail timing assertions on shared CI runners.

## Reproducing the measurements

### Core, discovery, and actions

```sh
cargo build --release --locked

cargo test -p rayslash-core --release --test performance \
  mixed_search_performance_probe -- --ignored --nocapture

cargo test -p rayslash-core --release --test performance \
  live_catalog_performance_probe -- --ignored --nocapture

cargo test -p rayslash-core --release --test performance \
  action_dispatch_performance_probe -- --ignored --nocapture

cargo test -p rayslash-core --release --test performance \
  app_activation_performance_probe -- --ignored --nocapture
```

Run one exact ignored test at a time. Running every ignored test without the required module environment variables will fail intentionally.

### Module host

Build the production host and each component first:

```sh
cd /home/rayan/Documents/Projects/rslauncher-modules/rayslash-module-host
cargo build --release --locked

cd /home/rayan/Documents/Projects/rslauncher-modules/rayslash-module-calculator
cargo component build --release --target wasm32-unknown-unknown
```

Single-module example:

```sh
cd /home/rayan/Documents/Projects/rayslash
RAYSLASH_MODULE_HOST=/home/rayan/Documents/Projects/rslauncher-modules/rayslash-module-host/target/release/rayslash-module-host \
RAYSLASH_MODULE_WASM=/home/rayan/Documents/Projects/rslauncher-modules/rayslash-module-calculator/target/wasm32-unknown-unknown/release/rayslash_module_calculator.wasm \
RAYSLASH_MODULE_QUERY='999 * 42' \
cargo test -p rayslash-core --release --test performance \
  module_host_performance_probe -- --ignored --nocapture
```

Optional controls are `RAYSLASH_MODULE_SETTINGS_JSON`, comma-separated `RAYSLASH_MODULE_NETWORK_ORIGINS`, `RAYSLASH_MODULE_COLD_SAMPLES`, `RAYSLASH_MODULE_WARMUPS`, and `RAYSLASH_MODULE_WARM_SAMPLES`.

Full installed-path fan-out example:

```sh
RAYSLASH_MODULE_HOST=/home/rayan/Documents/Projects/rslauncher-modules/rayslash-module-host/target/release/rayslash-module-host \
RAYSLASH_MODULES_ROOT=/home/rayan/Documents/Projects/rslauncher-modules \
cargo test -p rayslash-core --release --test performance \
  installed_module_fanout_performance_probe -- \
  --ignored --nocapture --test-threads=1
```

The fan-out probe builds an isolated, valid installed-module tree in a temporary directory and restores its process environment. It requires all seven release components to exist.

### Live UI

```sh
RAYSLASH_PROFILE=1 target/release/rayslash
```

The profiler reports backend setup, component construction, config/state load, folder and app discovery, search, result conversion, callback/IPC readiness, `show()`, event-loop readiness, first redraw request, synchronous result refresh stages, remote query end to end, and IPC queued-to-handled time.

Do not benchmark by launching a second process against an existing resident when measuring cold startup; that only measures the IPC client. Use an isolated `XDG_RUNTIME_DIR` connected to the same compositor, or stop the resident intentionally.

## Historical measurements

### 2026-07-21

Environment: local Fedora GNOME/Wayland session, optimized build, 75 discovered apps, 26 folders, and installed optional modules.

An empty startup query previously initialized and queried module hosts before the event loop. Empty queries now skip module execution, and the resident uses the daemon-style event loop.

```text
before: initial search 1.35s; startup before event loop 1.46s
after:  initial search 0.25ms; startup before event loop 136.83ms
warm IPC show client: <0.01s
warm IPC hide client: <0.01s
```

The old “startup before event loop” marker was emitted immediately after initial result-model construction, before callback registration, IPC startup, `ui.show()`, and the actual event-loop call. Treat 136.83 ms as a historical internal marker, not as directly comparable to the corrected 2026-07-22 ready boundary.

### 2026-07-12 module migration artifact sizes

| Artifact | Bytes | Approximate size |
| --- | ---: | ---: |
| Pre-migration app at `25f8315` | 37,913,912 | 36.2 MiB |
| Migrated core app | 37,228,544 | 35.5 MiB |
| Required module host | 14,659,904 | 14.0 MiB |

The migrated core was 685,368 bytes smaller than the pre-migration app, but the complete installation also required the host. Current artifacts are listed above and have grown since this snapshot.

### 2026-07-07 synthetic core search

The earlier probe used 40 repetitions and reported averages only.

| Query | Debug average | Release average |
| --- | ---: | ---: |
| Empty | 6.96 ms | 2.64 ms |
| `app 39` | 101.73 ms | 5.24 ms |
| `editor` | 40.25 ms | 2.74 ms |
| `project 42` | 30.15 ms | 1.39 ms |
| `999 * 42` | 17.85 ms | 1.04 ms |

The broad fuzzy query was about 19× slower in debug. Always reproduce perceived lag with the release binary before assigning it to UI rendering or provider behavior.
