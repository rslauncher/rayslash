# Diagnostics and privacy

Rayslash diagnostics are designed to explain why Linux desktop applications disappear during
discovery. They are not general product analytics: searches, launches, folders, and other user
behavior are outside the telemetry model.

## User controls

Automatic remote diagnostics are off by default. Enable **Send anonymous diagnostics** in
Settings → Diagnostics to opt in. Turn the same switch off to stop future submissions. Changing
the switch takes effect immediately and is persisted as
`diagnostics.send_anonymous_diagnostics` in `config.toml`.

The latest aggregate scan summary is available locally whether or not remote diagnostics are
enabled. **Copy diagnostic report** copies a safe, inspectable text report suitable for a GitHub
issue. Normal logs contain one aggregate summary line per completed scan, not one line per
desktop entry.

## What an automatic event contains

Rayslash submits at most one Sentry event for an application scan. An event contains:

- Rayslash version and telemetry schema version;
- CPU architecture;
- an allowlisted Linux distribution identifier and numeric major version, or `other`/`unknown`;
- an allowlisted desktop environment, or `other`/`unknown`;
- session type (`wayland`, `x11`, `tty`, or `unknown`);
- coarse installation type (`flatpak`, `appimage`, `snap`, or `system_or_source`);
- aggregate candidate and source-error counts;
- aggregate final-outcome counts, globally and by coarse application source;
- the Sentry SDK's event timestamp, random per-event identifier, protocol, and SDK version.

Application sources are coarse categories: user/system XDG, user/system Flatpak, Snap, host XDG,
or other. Exact source paths are not part of the diagnostic type. Identical summaries are
suppressed for ten minutes.

Like any HTTPS service, the Sentry server and intervening network infrastructure can observe the
connection's IP address. Rayslash does not place an IP address or a persistent device/user ID in
the event payload and does not ask Sentry to store a user identity.

## What is never submitted

Automatic diagnostics cannot carry application names, desktop filenames, filesystem paths,
usernames, hostnames, home-directory names, search queries, launch history, folder contents,
document names, clipboard contents, `Exec` commands, environment-variable contents, desktop-entry
contents, raw parser/IO errors, stack traces, breadcrumbs, panic reports, or persistent identifiers.

The telemetry boundary accepts only the strongly typed aggregate scan statistics. Sentry default
integrations are disabled, `send_default_pii` is false, and a final event scrub removes user,
request, server-name, context, breadcrumb, exception, thread, module, and stacktrace fields.

## Scan accounting

Every enumerated `.desktop` candidate gets exactly one final outcome:

- indexed;
- duplicate;
- hidden or `NoDisplay`;
- unsupported desktop-entry type;
- missing name or `Exec`;
- malformed desktop entry;
- excluded by desktop-environment rules;
- invalid `TryExec` or missing executable;
- invalid text encoding or read failure;
- metadata failure.

The invariant is `candidates == sum(final outcomes)`, both globally and for every source category.
Directory traversal failures are counted separately as source errors because no candidate path may
have been obtained. Optional XDG directories that do not exist are not errors.

Rayslash's parser is intentionally tolerant of unknown keys and malformed irrelevant lines. A file
without a `[Desktop Entry]` group is a parse failure; unknown keys in an otherwise usable entry are
not. Source classification is path-based locally, but only the coarse enum is reported.

## Backend and development configuration

The backend is the Rust Sentry SDK with only its `ureq` transport enabled. Panic, backtrace,
automatic OS/device contexts, debug-image, logging, tracing, metrics, and session integrations are
not built or enabled. This is important for release builds, which retain `panic = "abort"`—explicit
aggregate events are the only supported telemetry mechanism.

Set `RAYSLASH_SENTRY_DSN` while compiling a production build to embed the project DSN:

```sh
RAYSLASH_SENTRY_DSN='https://public-key@example.invalid/project' cargo build --release -p rayslash
```

A Sentry DSN embedded in a desktop application is a public client routing identifier, not an
administrative secret. Never embed API/auth tokens or organization credentials. Builds without a
DSN work normally and never submit events. Runtime `RAYSLASH_SENTRY_DSN` is also supported for
developer testing. Debug builds additionally require `RAYSLASH_ENABLE_DEV_TELEMETRY=1`; tests never
submit events.

Official GitHub release builds read the optional repository Actions variable
`RAYSLASH_SENTRY_DSN`. The release workflow supplies it to native, RPM, and Flatpak compilation;
the Flatpak manifest only passes that named value into the build sandbox and does not retain it as
a runtime environment variable. If the repository variable is absent or empty, all packages are
built normally with remote submission unavailable.

Submission is best-effort through Sentry's background transport. Offline, blocked, proxied, or
sandboxed networking does not affect discovery, startup, search, or launching. The existing Flatpak
manifest already permits network access for the signed module catalog and reviewed network modules;
diagnostics add no sandbox permission.

Distribution/package detection is deliberately conservative. RPM and DEB cannot currently be
distinguished reliably at runtime, so both report `system_or_source` rather than guessing.
