# Performance

This is the canonical performance baseline for rayslash. The optimization audit was implemented and remeasured on 2026-07-24, then its release-critical probes were repeated on the v0.2.1 tree on 2026-07-25. Compare results only when the build profile, machine, data set, cache state, and metric boundary are equivalent.

## Outcome

All measured P0–P2 bottlenecks from the 2026-07-22 audit have been addressed. The launcher now publishes local results immediately, routes only relevant modules, persists expensive catalogs and compiled WebAssembly code, defers presentation work, avoids synchronous action helpers, and bounds background-process and config-write growth.

The current measured interactions meet their applicable budgets:

| Interaction | Target | 2026-07-22 | 2026-07-24 |
| --- | ---: | ---: | ---: |
| Resident shortcut client exits | p95 < 10 ms | 3.64 ms | Unchanged path |
| Cold launcher ready for event loop | p95 < 150 ms | 137.6 ms | **45.68 ms** |
| Cold launcher first redraw requested | p95 < 150 ms | 247.8 ms | **87.28 ms** |
| Local result compute, 5k catalog | p95 < 16 ms | 5.39 ms worst query | **5.09 ms** worst query |
| Irrelevant query with seven modules installed | p95 < 16 ms | 150–174 ms UI floor; 1.388 s cold fan-out | **0.018 ms** routed warm dispatch |
| Warm local module result | p95 < 16 ms | 0.655 ms seven-host fan-out | **0.157 ms** worst single module |
| Cached network-backed result | p95 < 50 ms | Time: 927 ms | **0.183 ms** Time |
| Local action dispatch | p95 < 10 ms | 9.34 ms app activation | **0.295 ms** app activation |

These numbers do not imply that software can never be optimized further. They mean there is no remaining *measured, actionable bottleneck* in the audited paths on this system. Platform coverage, compositor-presented-frame measurement, and much larger catalogs remain validation work rather than evidence for another code change.

### v0.2.1 post-fix verification

The exact v0.2.1 release profile was rebuilt after the packaging and icon
changes. The launcher is 26,600,568 bytes (SHA-256
`9292d496767686128d817bd0bd1eecf6fb1cf625e26703bae80e04a9c469786f`).
The representative release probes remained within their budgets:

| Probe | 2026-07-25 result |
| --- | ---: |
| 5k local search, worst p95 | 4.995 ms |
| Irrelevant query with seven modules, warm p95 | 0.016 ms |
| Calculator fresh-host p50 / p95 | 14.571 / 15.519 ms |
| Calculator warm-host p50 / p95 | 0.135 / 0.195 ms |
| App activation p50 / p95 | 0.222 / 0.262 ms |
| Desktop reconciliation, 75 apps, p50 / p95 | 52.450 / 58.615 ms |
| Folder discovery, 26 children, p50 / p95 | 0.018 / 0.030 ms |

Calculator's first compiled-cache miss was 1.037 s and the following 19 fresh
hosts produced the reported cached distribution. Desktop reconciliation had one
114 ms outlier in 20 samples; its p50 and mean remained consistent with the
baseline, so this run does not support another code change.

## Measurement environment

| Item | Value |
| --- | --- |
| Date | 2026-07-24 |
| OS/session | Fedora Linux 44, GNOME, Wayland |
| Kernel | 7.1.4-200.fc44.x86_64 |
| CPU | Intel Xeon E5-2697 v3, 14 cores / 28 threads |
| Memory/filesystem | 15 GiB / Btrfs |
| Rust | rustc 1.92.0, Cargo 1.92.0 |
| cargo-component | 0.21.1 |
| Launcher build | release, thin LTO, one codegen unit, stripped symbols, abort panic |
| Module build | release components targeting `wasm32-unknown-unknown` |
| Live catalog | 75 apps, 26 folders |
| Synthetic catalog | 4,000 apps and 1,000 folders |

Distributions exclude explicit warm-up iterations. The percentile convention used by the existing probe is the selected sorted sample at `floor((n - 1) × percentile)`.

## Implemented architecture

### Startup and presentation

- A versioned desktop-app catalog is loaded from `~/.cache/rayslash/desktop-apps-v1.json`, then reconciled in a background thread.
- Initial results use fallback icons. Real icon decoding begins 500 ms after startup.
- Alternate folder-opener choices and their images are built only when Settings opens.
- Results use Slint's `ListView` viewport instead of instantiating an unbounded visible stack.
- Software rendering is the measured default for this small UI. `SLINT_BACKEND` remains available to override it.
- Registry refresh, app refresh, module operations, and remote result delivery wake the Slint event loop directly; no repeating UI polling timers remain.

Ten isolated starts used a primed app catalog, an isolated state/cache/runtime directory connected to the same Wayland compositor, and no installed modules:

| Stage | p50 | p95 | Previous p50 / p95 |
| --- | ---: | ---: | ---: |
| App catalog cache load | about 1.1 ms | about 1.2 ms | Discovery: 56.28 / 61.14 ms |
| Initial result-item build | 0.023 ms | 0.031 ms | 22.60 / 22.93 ms |
| Callbacks and IPC ready | 36.82 ms | 37.59 ms | 119.4 / 127.6 ms |
| Ready for event loop | **44.92 ms** | **45.68 ms** | 129.4 / 137.6 ms |
| First redraw requested | **85.05 ms** | **87.28 ms** | 242.5 / 247.8 ms |

An A/B run after warm-up measured first-redraw requests at roughly 83–95 ms with `winit-software` and 158–215 ms with `winit-femtovg` on this system. The environment override allows platform-specific validation without recompilation.

### Query scheduling

Local Apps/Folders results are computed and published on every query without waiting for modules. A single module scheduler:

1. receives query generations;
2. collapses superseded queued work;
3. applies 150 ms debounce only when a routed module can perform network work;
4. executes only modules whose official query shape or community manifest trigger matches;
5. posts the merged result directly to the UI event loop; and
6. discards stale generations.

The runtime snapshot caches installed state, manifests, revocations, permissions, and settings until the installed-state path or modification time changes. Catalogs are shared rather than cloned for every sleeping query thread.

An unrelated query with all seven official modules installed now starts no module host:

```text
cold dispatch: 0.425 ms
warm dispatch: p50 0.0076 ms, p95 0.0182 ms
```

Previously, any module caused a 150–174 ms UI scheduling floor and queried every enabled host.

### Core search

Release probe, 20 warm-ups and 200 samples over 5,000 synthetic entries:

| Query | p50 | p95 | Previous p50 / p95 |
| --- | ---: | ---: | ---: |
| Empty | 3.110 ms | 3.313 ms | 3.572 / 3.705 ms |
| `app 39` | 4.800 ms | 5.087 ms | 5.208 / 5.390 ms |
| `editor` | 2.703 ms | 2.926 ms | 3.189 / 4.009 ms |
| `project 42` | 1.297 ms | 1.421 ms | 1.456 / 3.037 ms |
| `999 * 42` | 0.886 ms | 0.971 ms | 1.034 / 1.369 ms |

The former top-k/precomputed-index proposal was deliberately not added. The measured 5,000-entry worst case is 5.09 ms p95, well inside the 16 ms budget, and an index would add invalidation complexity and memory. Reconsider only if an environment-qualified 10k–100k test breaches budget.

### Module runtime

The host now enables Wasmtime's persistent compiled-code cache under the module cache directory and reuses one HTTP agent for connection pooling. Hosts are still process-isolated, but idle workers are reaped after five minutes. Calculator is prewarmed after the launcher is responsive; other modules pay their cached startup only when routed.

Module components use size-oriented release profiles. Cold columns below contain five fresh host processes sharing a compiled-code cache: the first cache miss is reported as `first`; p50/p95 describe the subsequent distribution and therefore represent normal cold-process restart after installation-time compilation.

| Module | WASM bytes | Cold p50 | Cold p95 | First cache miss | Warm p50 | Warm p95 | Host RSS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Calculator | 660,452 | 14.64 ms | 14.69 ms | 1.020 s | 0.125 ms | 0.157 ms | 10,716 KiB |
| Units | 74,362 | 3.50 ms | 3.51 ms | 122.5 ms | 0.051 ms | 0.070 ms | 8,352 KiB |
| Time, non-trigger | 1,074,953 | 12.70 ms | 12.84 ms | 280.9 ms | 0.041 ms | 0.052 ms | 9,616 KiB |
| Timers | 44,532 | 5.72 ms | 7.64 ms | 91.9 ms | 0.048 ms | 0.055 ms | 8,432 KiB |
| Currency, non-trigger | 135,729 | 4.60 ms | 4.88 ms | 211.5 ms | 0.042 ms | 0.054 ms | 8,240 KiB |
| Web Search | 107,851 | 3.95 ms | 4.35 ms | 185.5 ms | 0.038 ms | 0.054 ms | 8,412 KiB |
| Aliases | 114,024 | 4.16 ms | 4.41 ms | 194.5 ms | 0.039 ms | 0.054 ms | 8,352 KiB |

Seven host RSS values total 62,120 KiB (60.7 MiB), down from 135.3 MiB. RSS double-counts shared pages, so this remains a conservative process-level comparison.

Time now caches normalized place-to-timezone metadata for seven days, computes displayed time from the host-provided Unix timestamp, and can use stale cache data when offline:

| `time in Tokyo` | Result |
| --- | ---: |
| Initial network + compile miss | 1.292 s |
| Cached fresh-process p50 | 13.35 ms |
| Cached warm-host p50 / p95 | **0.136 / 0.183 ms** |
| Previous repeated warm-host p50 / p95 | 908 / 927 ms |

The initial network lookup is service-dependent. It runs only after the Time trigger and does not hold back local results.

### Catalogs and actions

| Operation | p50 | p95 | Previous p50 / p95 |
| --- | ---: | ---: | ---: |
| Full desktop reconciliation, 75 apps | 51.57 ms | 52.75 ms | 54.04 / 57.21 ms |
| Folder discovery, 26 children | 0.017 ms | 0.032 ms | 0.018 / 0.029 ms |
| Raw process spawn | 0.172 ms | 0.216 ms | 0.177 / 0.218 ms |
| App activation, Wayland | **0.198 ms** | **0.295 ms** | 9.173 / 9.339 ms |

Wayland skips ineffective `wmctrl` probes. X11 uses one window-list process, matches all candidates locally, and performs at most one activation command. Default-browser discovery is cached for 30 seconds. Module actions return after spawn and are reaped/diagnosed asynchronously.

Ranking and app-install state writes use one serialized background writer. Config saves skip identical content, retain at most five backups, and no longer grow the backup directory without bound.

### Artifact size

| Artifact | 2026-07-24 | Previous | Change |
| --- | ---: | ---: | ---: |
| Launcher | 26,600,568 bytes | 39,330,040 | **−32.4%** |
| Module host | 15,844,888 bytes | 14,666,000 | +8.0% |
| Seven root module components | 2,211,903 bytes | 2,771,835 | **−20.2%** |

The host increase buys persistent Wasmtime caching and pooled HTTP. The installed launcher plus modules is still substantially smaller overall.

## Optimization audit closure

| Former roadmap item | Resolution |
| --- | --- |
| Immediate local results / later module merge | Implemented |
| First-redraw reduction and row virtualization | Implemented and under budget |
| Wasmtime cache and idle prewarm | Implemented |
| Time routing and lookup cache | Implemented |
| Persistent desktop catalog and background reconcile | Implemented |
| Single cancellable scheduler and event-driven delivery | Implemented |
| Module memory/idle lifecycle | Implemented with cached isolated hosts and five-minute reap |
| Synchronous action helpers and state writes | Removed from critical path |
| Module runtime metadata cache | Implemented |
| Large-catalog top-k/index | Rejected until measurements justify complexity |
| Release tuning | Implemented |
| Backup/write amplification | Implemented and bounded |

Remaining work is measurement breadth: compositor-presented-frame instrumentation, GNOME/KDE and Wayland/X11 comparisons, packaged-build cold-page-cache tests, aggregate PSS/GPU/power sampling, very large catalogs, and adversarial slow/crashing modules. These can reveal future bottlenecks but are not unimplemented optimizations supported by current evidence.

## Reproducing measurements

Build and run one ignored performance test at a time:

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

Build module artifacts first, then run the individual or installed-path probes:

```sh
cd /home/rayan/Documents/Projects/rslauncher-modules/rayslash-module-host
cargo build --release --locked

cd /home/rayan/Documents/Projects/rslauncher-modules/rayslash-module-calculator
cargo component build --release --target wasm32-unknown-unknown

cd /home/rayan/Documents/Projects/rayslash
RAYSLASH_MODULE_HOST=/home/rayan/Documents/Projects/rslauncher-modules/rayslash-module-host/target/release/rayslash-module-host \
RAYSLASH_MODULE_WASM=/home/rayan/Documents/Projects/rslauncher-modules/rayslash-module-calculator/target/wasm32-unknown-unknown/release/rayslash_module_calculator.wasm \
RAYSLASH_MODULE_QUERY='999 * 42' \
cargo test -p rayslash-core --release --test performance \
  module_host_performance_probe -- --ignored --nocapture

RAYSLASH_MODULE_HOST=/home/rayan/Documents/Projects/rslauncher-modules/rayslash-module-host/target/release/rayslash-module-host \
RAYSLASH_MODULES_ROOT=/home/rayan/Documents/Projects/rslauncher-modules \
cargo test -p rayslash-core --release --test performance \
  installed_module_fanout_performance_probe -- \
  --ignored --nocapture --test-threads=1
```

The installed-path probe's historical name says “fan-out”; routing now means its default unrelated query intentionally measures the no-host path.

For live startup:

```sh
RAYSLASH_PROFILE=1 target/release/rayslash
```

Do not start a second process against an existing resident when measuring cold startup; that measures only the IPC client. Use an isolated `XDG_RUNTIME_DIR` linked to the active compositor socket, or intentionally stop the resident.

## Historical baseline

The complete pre-optimization audit is represented by the 2026-07-22 columns above. Its principal measurements were:

- ready for event loop: 129.4 ms p50 / 137.6 ms p95;
- first redraw requested: 242.5 / 247.8 ms;
- desktop discovery: 54.04 / 57.21 ms;
- initial icon/result conversion: 22.60 / 22.93 ms;
- all-module cold query: 1.388 s;
- repeated Time lookup: 908 / 927 ms;
- app activation: 9.173 / 9.339 ms;
- seven module hosts: 135.3 MiB summed RSS; and
- launcher artifact: 39,330,040 bytes.

Earlier 2026-07-07 search averages and 2026-07-12 migration artifact sizes used different boundaries and remain available in repository history; they should not be treated as directly comparable regression baselines.
