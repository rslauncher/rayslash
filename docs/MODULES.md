# Modules Strategy

This document plans a module system for `rayslash`. It is intentionally a strategy document, not an implementation spec. The goal is to make optional features installable and community-extensible without weakening the launcher's current performance, privacy, packaging, or security model.

The implementation target was refined on 2026-07-11. [manual_migration.md](manual_migration.md) is the authoritative owner runbook and completion checklist. In particular, the final system uses deterministic GitHub Release assets, a signed static GitHub Pages registry with raw-GitHub and verified-cache fallbacks, and an optional separately installed WASM host. Older alternatives retained as prior-art discussion do not override those decisions.

Final API v1 correction: only sandboxed WASM modules are supported. The declarative-first sections below are retained as design history; no declarative format was sufficiently specified to publish reliably, so the SDK, registry, and app reject that reserved kind in API v1.

## Goal

Move selected toggleable features behind installable modules so the launcher can stay small while users can search, install, update, remove, and open/star extra capabilities from Settings.

The first user-facing module surface should support:

- Official modules shown separately from community modules.
- Community modules searchable from Settings after being submitted by GitHub repository URL.
- Install, update, remove, enable, and disable flows.
- Module metadata imported from the module manifest and GitHub: icon, name, short description, author, repository owner, repository URL, GitHub stars, version, license, permissions, and update channel.
- A star action that opens the GitHub repository so GitHub remains the first source of social proof.
- A reserved official author identity, `rayslash`, that community publishers cannot claim.

## Product Fit

`rayslash` should not become a general app store. Modules should extend launcher search providers, result actions, and small utilities. They should not own the launcher window, replace core app/folder search, or run arbitrary shell code by default.

The existing docs already draw the right boundary: internal provider boundaries should mature before a third-party plugin marketplace. The module system should therefore ship in phases:

1. Internal provider abstraction.
2. Local module manifest/runtime for official modules.
3. Remote registry and install/update/remove UI.
4. Community publishing with review, signing, permissions, and abuse controls.

## Terms

- **Provider**: code that receives a query and returns launcher results.
- **Module**: an installable package that may contribute one or more providers, actions, settings, icons, and metadata.
- **Registry**: a catalog of approved GitHub-backed modules plus cached metadata.
- **Package**: a versioned snapshot downloaded from a GitHub branch, tag, commit, or release asset.
- **Official module**: a module signed by the rayslash official signing key and published under the reserved `rayslash` author.
- **Community module**: a module signed by its author and listed in the community registry.

## Ulauncher Prior Art

Ulauncher is the best reference project for this feature because it has already built a mature Linux launcher extension ecosystem. Its v5 distribution model and current v6 architecture are distinct pieces of prior art and should not be described as one unchanged system.

Ulauncher v5 provides the older GitHub-catalog model that originally inspired this plan:

- Its installer accepted GitHub repositories, read `versions.json`, selected the last compatible API range entry, resolved the entry's branch, tag, or hash to a commit, and downloaded that snapshot. The pinned implementation is [`GithubExtension.py` from the v5 branch](https://github.com/Ulauncher/Ulauncher/blob/a594805ca4abf44c9b0eb05525accbe899bbad3a/ulauncher/api/server/GithubExtension.py#L47-L150).
- Its public extension site accepted a GitHub project URL and exposed GitHub-star sorting. The pinned frontend sources are the [GitHub submission step](https://github.com/Ulauncher/ext.ulauncher.io/blob/d1b99a77904359678a43689ab6df54f76eb6c0a7/src/extAdd/Step1.js#L7-L44) and [catalog sorting controls](https://github.com/Ulauncher/ext.ulauncher.io/blob/d1b99a77904359678a43689ab6df54f76eb6c0a7/src/extBrowse/Browse.js#L16-L25).

The [current Ulauncher v6 development line](https://github.com/Ulauncher/Ulauncher/blob/44d2194c7edecf7550dbedaf2c27a4956ae3fd5c/README.md#L5-L9) provides different implementation lessons:

- It no longer consults v5's `versions.json`. It discovers Git refs named for API compatibility, such as `apiv3` and `apiv3.1`, selects the highest compatible ref, and resolves it to a commit before download. See [`ExtensionRemote.get_compatible_hash`](https://github.com/Ulauncher/Ulauncher/blob/44d2194c7edecf7550dbedaf2c27a4956ae3fd5c/ulauncher/modes/extensions/extension_remote.py#L211-L247).
- Its installer normalizes GitHub, GitLab, and Codeberg URLs, accepts local repositories, and can use other Git remotes when Git is installed. See [`parse_extension_url`](https://github.com/Ulauncher/Ulauncher/blob/44d2194c7edecf7550dbedaf2c27a4956ae3fd5c/ulauncher/modes/extensions/extension_remote.py#L332-L390).
- Its manifest has first-class `triggers` separate from ordinary preferences, rather than requiring a keyword-shaped preference. See [`ExtensionManifest`](https://github.com/Ulauncher/Ulauncher/blob/44d2194c7edecf7550dbedaf2c27a4956ae3fd5c/ulauncher/modes/extensions/extension_manifest.py#L24-L110).
- Its generic result model carries display fields and named actions. See [`Result`](https://github.com/Ulauncher/Ulauncher/blob/44d2194c7edecf7550dbedaf2c27a4956ae3fd5c/ulauncher/internals/result.py#L7-L81).
- Its internal [`Mode` contract](https://github.com/Ulauncher/Ulauncher/blob/44d2194c7edecf7550dbedaf2c27a4956ae3fd5c/ulauncher/modes/mode.py#L8-L71) lets a provider own query handling and activation, while the [central core](https://github.com/Ulauncher/Ulauncher/blob/44d2194c7edecf7550dbedaf2c27a4956ae3fd5c/ulauncher/core.py#L30-L230) owns routing, trigger conflicts, result ownership, and action dispatch.
- Extensions run as separate same-user processes over Unix socket pairs, but they are explicitly not sandboxed. See Ulauncher's pinned [extension IPC architecture](https://github.com/Ulauncher/Ulauncher/blob/44d2194c7edecf7550dbedaf2c27a4956ae3fd5c/docs/architecture/extension-ipc.md#L1-L59).

For `rayslash`, this suggests a better default than a custom module store:

- Let GitHub repositories be the canonical source for community modules.
- Let users submit a GitHub URL.
- Let the registry crawl and validate the repository.
- Let the client discover an API-version ref, resolve it to an immutable commit, validate the manifest at that commit, and install that snapshot.
- Use GitHub stars as the primary community ranking and trust signal.
- Make the UI star button open the GitHub repository instead of building a custom star backend first.
- Show both manifest author and GitHub owner/repository so users can distinguish a display name from the actual source owner.

The GitHub-only catalog, GitHub stars, reserved author rules, declarative-first packages, and later WASM sandbox are deliberate `rayslash` product decisions. They are not claims about the current Ulauncher client, which supports more Git hosts and executes Python extensions without a sandbox. The narrower `rayslash` model keeps infrastructure small and gives community modules a familiar open-source workflow: issues, pull requests, releases, stars, forks, maintainers, and source browsing all happen on GitHub.

The main adaptation for `rayslash` is security. Separate processes are a useful crash boundary, but Ulauncher's same-user processes are not a permission sandbox. `rayslash` should prefer declarative modules first and sandboxed WASM later, because arbitrary native or script execution would be a poor fit for the current Rust launcher and its conservative no-shell action model.

## What Should Become Modules

Keep these in the launcher core:

- Desktop app discovery and launching.
- Folder source scanning and opening.
- Config, state, cache, IPC, package identity, settings shell, ranking core, and module manager.
- Result list rendering, keyboard/mouse behavior, and activation dispatch.

These can become official modules once the runtime exists:

- Calculator.
- Unit conversion.
- Currency conversion.
- Time lookup.
- Web search templates and default browser search.
- Reboot, shutdown, logout, timer, and reminder commands.
- Aliases/quick links, if the module API can preserve local config cleanly.

These are good community-module candidates later:

- Snippets.
- Clipboard history, disabled by default with explicit storage and clear-history controls.
- Window switching, only after a reliable cross-desktop strategy exists.
- File browser mode.
- Extra web/search integrations.
- Developer utilities such as GitHub issue search, package lookup, docs search, and project-specific commands.

Avoid in the early module system:

- Arbitrary native-code modules.
- Shell-script modules that run automatically while typing.
- Modules that capture global shortcuts.
- Modules that store sensitive history without a visible setting and clear storage path.
- Modules that need broad desktop or filesystem access without a permission prompt.

## Recommended Runtime Direction

Use a staged runtime. Do not start with arbitrary dynamic libraries.

### Stage 1: Internal Provider Trait

Create an internal provider boundary in `rayslash-core` before any installable module work.

Each provider should own:

- Stable provider ID.
- Display metadata.
- Enabled/disabled state.
- Provider-specific config.
- Query matching.
- Result construction.
- Activation/action construction.
- Ranking eligibility.
- Diagnostics.
- Permission requirements, even if all first-party providers initially grant implicitly.

This converts today's hard-coded provider booleans and `SearchResultKind` branches into a shape that external modules can eventually plug into.

### Stage 2: Declarative Modules

Support a restricted declarative module format first. This covers useful extensions without executing third-party code.

Good first declarative module types:

- Web search engines.
- Static aliases/quick links.
- URL templates with query encoding.
- Documentation search providers.
- Command templates that require explicit activation and show their exact command/arguments before enabling.

This stage can reuse much of the current alias and web-search behavior while moving metadata, install/update/remove, and registry logic into the module manager.

### Stage 3: WASM Modules

Use WebAssembly for executable community modules if declarative modules are not enough.

Preferred shape:

- WASI-style sandbox.
- No filesystem/network/process access by default.
- Host-provided capabilities for query input, result output, icons, cache reads/writes, and approved network requests.
- Timeouts per query.
- Result count limits.
- Memory limits.
- No UI rendering API; modules return typed results only.

Rust can host this through a WASM runtime later. The module API should be versioned separately from the app version.

### Stage 4: Native Modules Only If Needed

Native dynamic modules should be last resort because they inherit the user's process permissions and complicate packaging, crashes, ABI stability, and trust. If native modules ever exist, they should be disabled by default, clearly labeled as full-trust, and outside the normal community registry.

## Package Layout

Use a GitHub repository for source and a deterministic `.tar.zst` GitHub Release asset for each installable version. A community submission still starts with a repository URL, but clients install the registry-pinned release asset rather than a moving branch or GitHub-generated source archive.

Example repository:

```text
module.toml
icon.svg
README.md
LICENSE
module.wasm
```

The registry and installer should:

1. Discover published releases and their module package assets.
2. Download a candidate asset into registry validation, never directly into an active install.
3. Verify the package layout and `module.toml` API constraint.
4. Record the source commit, release/asset URL, byte size, and SHA-256 digest in the signed registry.
5. Choose the highest non-yanked version compatible with the host module API.
6. Revalidate the manifest, package limits, and digest in the client before atomic activation.

Release tags and assets must be treated as immutable. Replacing an asset requires a new module version. A digest change for an already indexed version is a registry validation failure, not an update.

`apiv` refs and `versions.json` remain useful Ulauncher prior art but are not part of rayslash package discovery. Compatibility belongs in the versioned manifest and signed registry record.

Example `module.toml`:

```toml
id = "community.example.docs"
name = "Docs Search"
description = "Search project documentation from rayslash."
author = "example"
version = "1.0.0"
api_version = "^1.0.0"
license = "MIT"
homepage = "https://example.com/docs-search"
source = "https://github.com/example/rayslash-docs-search"
icon = "icon.svg"
kind = "wasm"

[permissions]
network = ["https://docs.example.com"]
cache = true
commands = false
filesystem = false

[[providers]]
id = "docs"
name = "Docs"
description = "Search documentation."
```

Rules:

- `id` is globally unique and reverse-DNS-like.
- `author = "rayslash"` is reserved.
- `source` must match the submitted GitHub repository URL for community modules.
- `description` should be short enough for the Settings list.
- Installed module versions should record the source URL, release asset URL, source commit, package digest, and fetched manifest hash.
- Installed packages are immutable by version and package digest.
- Release assets are the required package source for the public catalog.

## Local Storage

Follow the existing XDG model:

- Installed modules: `~/.local/share/rayslash/modules/`
- Module config: `~/.config/rayslash/modules.toml`
- Module state: `~/.local/state/rayslash/modules/`
- Module cache: `~/.cache/rayslash/modules/`
- Registry cache: `~/.cache/rayslash/module-registry/`

Keep module install state separate from the main `config.toml`. The existing settings autosave rewrites known config fields, so module metadata should not live inside the main config until the config model can preserve unknown fields.

Suggested files:

```text
~/.config/rayslash/modules.toml
~/.local/share/rayslash/modules/<module-id>/<version>/
~/.local/state/rayslash/modules/installed.toml
~/.cache/rayslash/module-registry/index.json
```

## Registry Hosting

Use a GitHub-backed catalog. The registry should store module listings and cached GitHub metadata, not become the canonical package host.

Required first hosting stack:

- **Registry index**: signed static JSON on GitHub Pages, with raw GitHub output and the last verified local cache as fallbacks.
- **Submission flow**: user submits a GitHub repository URL.
- **Metadata crawler**: validates GitHub Release assets, `module.toml`, icon path, license, API compatibility, source commit, package digest, package limits, and reserved author rules.
- **Package source**: deterministic GitHub Release assets pinned by URL and SHA-256 inside the signed registry.
- **Social proof**: GitHub repository stars, owner, source URL, license, and last update.
- **Moderation**: pull requests and GitHub review controls. No required application backend or database.

Why this split:

- Static registry reads are fast, cacheable, cheap, and resilient.
- GitHub remains the source of truth for community module code, authorship, stars, issues, pull requests, releases, and source browsing.
- The registry can stay thin: it approves and indexes GitHub repositories rather than hosting a full package marketplace.
- GitHub stars remove the need for a custom star/rating backend in the first version.
- Public GitHub Pages/Actions and Releases avoid a required paid service, while a static raw-GitHub path and verified client cache provide read fallbacks.

Fallback behavior:

- If the registry is unavailable, Settings should show installed modules and cached registry results.
- If GitHub metadata refresh is unavailable, search/install should still work with cached star counts.
- If a GitHub download fails, show the repository URL and keep the installed version unchanged.
- If a release asset no longer matches its signed digest, installation/update fails and the currently active version remains unchanged.

## Submission Flow

The first community submission flow combines Ulauncher v5's simple GitHub URL catalog with immutable, registry-pinned release packages:

1. Author creates a public GitHub repository from the SDK template.
2. Repository includes `module.toml`, an icon, README, license, tests, and the standard package/release workflow.
3. Author publishes a deterministic module package as a GitHub Release asset.
4. Author submits the GitHub URL through a pull request to the registry.
5. Pull-request validation downloads the asset, validates the package and source metadata, and checks reserved identities without access to signing secrets.
6. A maintainer reviews declared permissions and implementation/release provenance; accepted changes merge to the protected default branch.
7. The post-merge workflow regenerates and signs the catalog, caches GitHub metadata, and publishes Pages.
8. The rayslash Settings Modules page can search, install, update, remove, and open the repository.

Submission validation should check:

- Repository is public and reachable.
- At least one immutable release package is compatible with a supported rayslash module API.
- The package identifies its source commit and matches the submitted repository.
- Manifest has non-empty name, description, author, id, version, icon, and provider metadata.
- Community manifest does not claim `author = "rayslash"` or a reserved `rayslash.*` module ID.
- Icon path is local to the repository and uses a supported image format.
- README and license are present.
- Permission declarations are present.
- Executable module kinds are flagged as unreviewed until a review policy exists.

## Registry Index

Use one signed top-level index and smaller shard files for search.

Suggested files:

```text
index.json
modules/a/community.example.docs.json
modules/r/rayslash.calculator.json
icons/community.example.docs.svg
```

`index.json` should contain:

- Registry schema version.
- Generated timestamp.
- Minimum supported rayslash version.
- Official signing public key IDs.
- Module summaries for fast search.
- Per-module metadata URL.
- Registry signature.

A module summary should include:

- `id`
- `name`
- `description`
- `author`
- `github_owner`
- `github_repo`
- `github_url`
- `official`
- `latest_version`
- `api_version`
- `icon_url`
- `keywords`
- `categories`
- `github_stars`
- `github_forks`
- `github_open_issues`
- `updated_at`
- `review_status`
- `quality_flags`
- `metadata_url`

The full per-module file should include:

- All summary fields.
- Submitted GitHub URL.
- Release tags/assets and source commit hashes for indexed versions.
- Package byte sizes and SHA-256 digests.
- Version history.
- Optional package URLs and mirrors.
- Optional checksums/signatures for reviewed versions.
- Changelog URL.
- Permission manifest.
- Compatibility constraints.
- Deprecation/replacement info.

## Search And Ranking

Module search should use a weighted score, not stars alone.

Suggested ranking inputs:

- Exact ID/name match.
- Prefix match.
- Fuzzy name/keyword score using the existing matcher.
- Official module boost.
- Installed/update-available boost in the installed section only.
- GitHub star count with logarithmic weight.
- GitHub repository owner/name match.
- Recent update freshness, bounded.
- Compatibility with the current app/module API.
- Quality flags such as reviewed, unreviewed executable, deprecated, archived, or reported.

Suggested order:

1. Installed modules with update/enable/disable/remove actions.
2. Official modules not installed.
3. Community results.

Community ranking formula should be documented and conservative. A module with many stars should not outrank a direct name match for a smaller module.

## Stars

Use GitHub stars first. Do not build a separate rayslash star system until the GitHub-backed model proves insufficient.

Recommended first implementation:

- The module row shows the cached GitHub star count.
- The star button opens the GitHub repository in the user's browser.
- The details view includes the GitHub source URL.
- The registry crawler refreshes star counts on a schedule.
- Search ranking treats GitHub stars as a weak signal, not a replacement for text relevance or official/reviewed status.

Privacy:

- Do not send query text.
- Do not send installed module lists.
- Do not send usernames, hostnames, paths, or desktop app data to a rayslash service.
- Opening the repository is an explicit user action, and GitHub receives that browser request.

Tradeoffs:

- GitHub stars are familiar and hard enough to abuse compared with anonymous local votes.
- They also bias results toward older or more publicized repositories.
- They require users to have or create a GitHub account if they want to actually star a repository.
- A module can have high stars for reasons unrelated to quality or safety.

Possible later custom rating layer:

- Add install-count telemetry only if it is explicit and privacy-preserving.
- Add a rayslash-specific star only if GitHub stars are not enough.
- Use optional GitHub OAuth only if the catalog needs authenticated reviews or abuse-resistant voting.

## Official Author Protection

The author name `rayslash` must be reserved.

Enforce this in three layers:

- Registry publishing rejects community modules with `author = "rayslash"`.
- The client only shows the official badge when the module is signed by the official rayslash key.
- The client treats any unsigned or community-signed module claiming the `rayslash` author as invalid.

Official module IDs should use a reserved namespace:

```text
rayslash.calculator
rayslash.units
rayslash.currency
rayslash.time
rayslash.web-search
rayslash.timers
```

Community module IDs should use reverse-DNS or account namespaces:

```text
io.github.author.module-name
community.author.module-name
```

## Trust And Security

The module manager should assume community modules are untrusted.

Required before community executable modules:

- Trusted registry index generation.
- GitHub repository URL validation.
- Immutable release-asset discovery and API compatibility selection.
- Source commit, release URL, byte size, and package digest recording for installed versions.
- Signed registry verification plus package digest verification for every catalog install.
- Immutable version install directories.
- Permission display before install and when permissions change on update.
- Query-time execution timeout.
- Result count cap.
- Memory cap.
- Network allowlist.
- No process spawning from query-time code.
- Explicit activation for actions that open URLs, write files, copy text, or run commands.
- Clear uninstall that removes package files, with a separate option to remove state/cache.

Updates:

- Safe patch/minor updates can be one-click if permissions do not expand.
- Permission-expanding updates must require confirmation.
- Major-version updates should show release notes or at least a short changelog.
- If a module is removed from the registry for abuse, the client should mark it as unavailable and stop auto-updating it.
- Published versions use immutable release assets; an asset/digest replacement requires a new version and review.

## Settings UI

Use the existing Modules page inside Settings rather than mixing every module into the current provider toggle list.

Suggested layout:

- Search field.
- Tabs or segmented control: `Installed`, `Official`, `Community`.
- Result rows with icon, name, short description, author, GitHub owner/repo, version, GitHub star count, and status.
- Row actions: install, update, remove, enable/disable, open GitHub/star.
- Details drawer/panel for permissions, source, license, changelog, installed path, and diagnostics.

Default/official modules:

- Show before community modules.
- Use existing built-in fallback icons where possible.
- Author displays as `rayslash`.
- Use concise descriptions, for example:
  - Calculator: `Calculate expressions and linear equations.`
  - Units: `Convert common units locally.`
  - Currency: `Convert currencies with cached live rates.`
  - Time: `Check local time for places.`
  - Web Search: `Search the web with keyword triggers.`
  - Timers: `Schedule timers, reminders, and power actions.`

Community modules:

- Show the real author name.
- Show the GitHub owner/repository.
- Never allow the official badge unless the official signature validates.
- Show a warning for unreviewed executable modules.
- The star button opens the GitHub repository.

## Config Model

Do not add arbitrary module fields to the main `Config` struct immediately. Use a separate module config file so settings autosave does not drop unknown module data.

Example `modules.toml`:

```toml
version = 1

[registry]
url = "https://OWNER.github.io/rayslash-registry/v1/root.json"

[modules."rayslash.calculator"]
enabled = true
version = "1.0.0"
channel = "stable"

[modules."community.example.docs"]
enabled = true
version = "1.0.0"
channel = "stable"
```

Provider toggles in `config.toml` should eventually map to installed official modules, but keep compatibility:

- Existing provider booleans continue to load.
- Migration installs/enables matching official modules or creates virtual built-in module entries.
- Settings can show legacy providers as official modules during the transition.
- Saving should not break older config files until a documented migration phase.

## Migration Plan

### Phase A: Provider Boundary

- Define internal `ProviderId`, `ProviderMetadata`, `ProviderConfig`, `ProviderResult`, and `ProviderAction`.
- Move calculator, units, currency, time, web search, aliases, apps, and folders behind the internal boundary.
- Keep user-visible behavior unchanged.
- Keep current tests passing and add provider-boundary tests.

### Phase B: Module Manager Without Remote Registry

- Add local installed-module state.
- Add official built-in module descriptors for existing optional providers.
- Add a Settings Modules page that can enable/disable official modules.
- Keep implementation backed by current built-in code.

This phase gives the UI and config shape without remote execution risk.

### Phase C: Declarative Remote Modules

- Add static registry fetch/cache.
- Add GitHub repository submission, release-asset validation, signed digest verification, install, update, and remove.
- Support declarative web-search/alias/doc-search modules.
- Add official/community separation.
- Use GitHub stars and repository metadata in the module search UI.

### Phase D: Review, Integrity, And Ranking

- Cache GitHub stars and repository metadata in registry.
- Add conservative ranking formula.
- Add moderation flags and reserved author enforcement.
- Add optional checksums/signatures for reviewed tags or release assets.

### Phase E: Optional WASM Host

- Add the sandboxed executable module runtime as a separately installed host process so a fresh core installation does not carry the runtime cost.
- Start with official WASM modules.
- Expand to reviewed community modules after permissions, timeouts, crash handling, and tests are reliable.

## Implementation Status

The production implementation uses the signed registry, verified atomic package installer, optional no-WASI host, and separately released official modules described above. Fresh installs contain only Apps and Folders. Version-1 virtual entries are retained only as opt-in `Restore` choices; they do not represent bundled code and do not download automatically. Current repository/release details and the remaining owner merge/deployment steps are recorded in [manual_migration.md](manual_migration.md).

## Testing Strategy

Core tests:

- Manifest parsing and validation.
- Release/package parsing, API-version matching, and source-commit validation.
- Reserved author rejection.
- GitHub URL normalization and source matching.
- Release asset/source commit/package-digest install state.
- Registry signature and package-digest verification.
- Registry index parsing, caching, and stale fallback.
- Install/update/remove state transitions.
- Permission-change update blocking.
- Search ranking formula.
- Provider result ordering and stable IDs.
- Backward compatibility with current provider config.

UI/manual checks:

- Module list search and tab behavior.
- Install/update/remove flows.
- Open-GitHub/star action opens the expected repository URL.
- Long module names/descriptions do not overlap.
- Permission text remains readable in compact launcher settings.
- Offline/cached registry behavior.
- Invalid package/signature errors.

Security tests:

- Repository URL validation.
- Package path traversal attempts.
- Oversized manifest/package rejection.
- Invalid signatures.
- Hash mismatch.
- Registry replay/downgrade handling.
- WASM timeout and memory limit behavior once WASM exists.

## Remaining Policy Inputs

- Permanent GitHub owner and repository URLs.
- Registry signing public key/key ID after the repository key-generation script exists.
- Maintainers, support/security contacts, and any changes to the recommended moderation policy.
- Initial architecture/package targets for the optional module host.

Package delivery, hosting, ratings, executable review, and default-install behavior are no longer open: packages use `.tar.zst` GitHub Release assets, the signed catalog uses GitHub Pages with raw/cache fallbacks, GitHub stars remain the initial weak popularity signal, WASM submissions receive manual permission/source review, and fresh installations contain no optional modules.

## Next Implementation Slice

The provider boundary and virtual Modules settings surface are complete. The next slice begins after the owner supplies the permanent repository URLs and policy inputs from [manual_migration.md](manual_migration.md): freeze API/package schemas, scaffold the SDK/registry/host repositories, add the registry key-generation script and unsigned pull-request validation, then pause only for the owner to install the protected signing secret and enable Pages.

After that bootstrap is verified, implement declarative package install/update/remove with atomic rollback before accepting executable community modules through the optional sandbox host.

## Hosting References Checked

- The pinned [Ulauncher v5 GitHub installer](https://github.com/Ulauncher/Ulauncher/blob/a594805ca4abf44c9b0eb05525accbe899bbad3a/ulauncher/api/server/GithubExtension.py#L47-L150) documents the legacy `versions.json` selection and commit-resolution model.
- The pinned Ulauncher extension-site [submission](https://github.com/Ulauncher/ext.ulauncher.io/blob/d1b99a77904359678a43689ab6df54f76eb6c0a7/src/extAdd/Step1.js#L7-L44) and [GitHub-star sorting](https://github.com/Ulauncher/ext.ulauncher.io/blob/d1b99a77904359678a43689ab6df54f76eb6c0a7/src/extBrowse/Browse.js#L16-L25) sources provide prior art for a thin GitHub catalog.
- The current v6 [remote resolver](https://github.com/Ulauncher/Ulauncher/blob/44d2194c7edecf7550dbedaf2c27a4956ae3fd5c/ulauncher/modes/extensions/extension_remote.py#L211-L247) and [URL normalization](https://github.com/Ulauncher/Ulauncher/blob/44d2194c7edecf7550dbedaf2c27a4956ae3fd5c/ulauncher/modes/extensions/extension_remote.py#L332-L390) provide the preferred API-ref and resolved-commit model while demonstrating broader Git-host support than rayslash currently plans.
- The current v6 [`Mode`](https://github.com/Ulauncher/Ulauncher/blob/44d2194c7edecf7550dbedaf2c27a4956ae3fd5c/ulauncher/modes/mode.py#L8-L71), [`Result`](https://github.com/Ulauncher/Ulauncher/blob/44d2194c7edecf7550dbedaf2c27a4956ae3fd5c/ulauncher/internals/result.py#L7-L81), and [extension IPC architecture](https://github.com/Ulauncher/Ulauncher/blob/44d2194c7edecf7550dbedaf2c27a4956ae3fd5c/docs/architecture/extension-ipc.md#L1-L59) inform the provider/action boundary and the decision not to treat process separation as sandboxing.
- GitHub Pages supports public repositories on GitHub Free and HTTPS for Pages sites.
- GitHub Releases support release assets and document no bandwidth limit for release assets.
- GitHub Pages documents public-repository availability on GitHub Free plus soft site/bandwidth/build limits; clients therefore retain raw-GitHub and last-verified-cache fallbacks.
- GitHub Actions documents standard hosted runners as free for public repositories.
- GitHub's generated source archives do not guarantee stable outer archive bytes and GitHub recommends Release assets when archive security matters.
