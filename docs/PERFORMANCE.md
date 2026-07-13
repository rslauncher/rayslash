# Performance

## Module migration measurements (2026-07-12)

Release builds used the same local x86_64 Rust release profile and dependency cache:

| Artifact | Bytes | Approximate size |
| --- | ---: | ---: |
| Pre-migration app at `25f8315` | 37,913,912 | 36.2 MiB |
| Migrated core app | 37,228,544 | 35.5 MiB |
| Required module host | 14,659,904 | 14.0 MiB |

The core is 685,368 bytes smaller (1.8%) even after adding signature verification, compressed package installation, registry networking/cache, and module lifecycle UI. The 14.0 MiB Wasmtime host is required infrastructure in supported app packages, so the complete fresh installation pays that fixed cost while still containing no optional module packages.

The live Calculator conformance probe measures both cold and warm behavior through the verified installer and real host. Cold startup initializes Wasmtime once; the persistent-host second query must complete in under 250 ms. Installed modules are queried in parallel so one slow module does not serialize all providers.

This file records repeatable performance checks and observed results so regressions can be compared over time.

## How To Measure

Use the optimized binary for user-facing smoothness checks:

```sh
cargo build --release
RAYSLASH_PROFILE=1 target/release/rayslash
```

`target/debug/rayslash` is useful for development, but it is built with the `dev` profile and is unoptimized. It can stutter in places where the release binary is smooth.

Run the synthetic core search probe with:

```sh
cargo test -p rayslash-core --test performance -- --ignored --nocapture
cargo test -p rayslash-core --release --test performance -- --ignored --nocapture
```

The probe creates 4,000 synthetic apps and 1,000 synthetic folders, then repeats several queries 40 times. It measures core search only; it does not measure Slint rendering, input latency, real icon loading, compositor behavior, or scroll paint cost.

For live UI diagnosis, run with:

```sh
RAYSLASH_PROFILE=1 target/release/rayslash
```

The live profiler prints startup stages, settings app refresh, core search, result-item conversion, model replacement, UI property updates, and total result refresh time.

## History

### 2026-07-07

Environment: local development machine, synthetic probe, current workspace.

Debug command:

```sh
cargo test -p rayslash-core --test performance -- --ignored --nocapture
```

Debug results:

```text
query="" avg=6.96ms
query="app 39" avg=101.73ms
query="editor" avg=40.25ms
query="project 42" avg=30.15ms
query="999 * 42" avg=17.85ms
```

Release command:

```sh
cargo test -p rayslash-core --release --test performance -- --ignored --nocapture
```

Release results:

```text
query="" avg=2.64ms
query="app 39" avg=5.24ms
query="editor" avg=2.74ms
query="project 42" avg=1.39ms
query="999 * 42" avg=1.04ms
```

Interpretation: the broad fuzzy query `app 39` is about 19x slower in debug than release in this probe. This strongly suggests that perceived lag in `target/debug/rayslash` should be checked against `target/release/rayslash` before attributing it to UI behavior.
