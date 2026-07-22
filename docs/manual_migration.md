# Manual Module Migration Runbook

This runbook records the migration decisions and the owner bootstrap completed in July 2026. The repository, signing, release, and deployment steps below are historical; do not repeat them. Owner bootstrap is complete and no additional owner input is currently required, but the migration itself is not complete until every acceptance item below is verified. Remaining work belongs to implementation, automated ecosystem tests, packaging validation, and the real-desktop/package matrix.

The instructions separate work that requires repository/account ownership from work Codex can implement after you return the requested information. You should not need to design code, write registry files, or write module documentation manually.

## 1. Outcome

The migration is complete only when all of the following are true:

- A fresh rayslash installation contains only the launcher core, module manager, sandbox host, and core Apps/Folders providers.
- Calculator, Units, Currency, Time, Web Search, Timers, and Aliases are not installed or compiled into a fresh installation.
- Official and community modules are downloaded only when a user chooses to install them.
- Removing a module removes its package code. Keeping or deleting its config/state is a separate user choice.
- Installed modules can be searched, installed, enabled, disabled, updated, and removed in Settings.
- An unavailable registry does not break installed modules. Cached catalog data remains readable.
- A failed install or update never damages the currently installed version.
- Module packages and registry metadata are verified before use.
- Community authors can create, validate, package, test, release, and submit a module by following public documentation.
- Community module code cannot directly access the network, filesystem, clipboard, notifications, or process execution. It receives only capabilities declared in its manifest and approved by the user.
- The module API is versioned independently of the rayslash app version.
- Existing users receive a safe one-time migration from virtual bundled modules; fresh users receive no optional modules.

## 2. Final architecture (already decided)

Use this architecture unless a step below discovers a hard platform restriction.

### 2.1 Repositories and hosting

- GitHub repositories are the source of truth.
- GitHub Release assets are the package source. Do not install a moving branch archive.
- A separate public registry repository contains submissions, generated static JSON, schemas, and the catalog website.
- GitHub Actions validates submissions, refreshes public GitHub metadata, generates the registry, signs it, and deploys GitHub Pages.
- The app reads the Pages URL first, a raw GitHub URL second, and its last verified local cache third.
- There is no required Cloudflare Worker, D1, KV, R2, paid database, custom ratings service, or custom package CDN.
- Community submission is a pull request to the registry. An issue form may help authors, but it is not the canonical submission record.
- GitHub stars are cached by the registry generator. The client does not call the GitHub API for every user.

This is free for public repositories using standard GitHub-hosted runners and public GitHub Pages. It also avoids making the launcher depend on the unauthenticated GitHub REST limit for browsing. GitHub Pages has usage limits, so the raw URL and verified local cache are required fallbacks.

### 2.2 Package format

- Release asset name: `<module-id>-<version>.tar.zst`.
- The archive contains one top-level directory and no links, devices, absolute paths, or parent traversal.
- Required package files: `module.toml`, `README.md`, `LICENSE`, and `icon.svg` or `icon.png`.
- Executable packages also contain `module.wasm`.
- The registry records the release URL, module version, API requirement, source commit, archive size, and SHA-256 digest.
- The client downloads to a temporary file, enforces size limits, verifies SHA-256, safely extracts to a staging directory, validates the manifest and files again, then atomically activates the directory.
- Installed paths are immutable and keyed by module ID, version, and package digest.

Do not rely on a hash of GitHub's generated source archive: GitHub documents that compression can change. A release asset built by the module workflow is the package artifact.

### 2.3 Module kinds

Version 1 supports `wasm` packages using the rayslash WIT API. They run in a separate `rayslash-module-host` process with no WASI access and no ambient operating-system access. The manifest parser reserves the `declarative` value for a possible later API, but API v1 validators, the registry, and the app reject it so authors cannot publish a package that the launcher cannot execute.

Do not add native dynamic libraries, Python processes, shell extensions, or arbitrary executable scripts to the default registry.

### 2.4 Required runtime host

`rayslash-module-host` is separately maintained and released, but it is required installation infrastructure:

- A supported rayslash package must require the matching host package or include the host executable, so a normal app installation can install modules immediately.
- The host is not a module and does not provide Calculator, Units, Currency, Time, Web Search, Timers, Aliases, or any community feature by itself.
- Native distro packages keep the host as a separate artifact and pull it in through a mandatory dependency.
- Local/development installs may use a verified official GitHub Release asset.
- Flatpak bundles the digest-pinned host release in the app build; it must not execute a newly downloaded native binary. This is not permission to bundle official modules.
- The host uses Wasmtime's component model and the rayslash WIT world, but does not link WASI command/filesystem/network interfaces.
- The launcher and host communicate using a versioned local IPC protocol.
- The launcher owns permissions, network requests, action execution, ranking, timeouts, and result caps.
- The host is disposable. A crash or timeout terminates/restarts it without terminating rayslash.

Keeping the host in a separate repository and native package preserves its security and release boundary. Delivering it automatically with rayslash ensures that module browsing and installation are complete app features rather than optional setup.

### 2.5 Trust model

- The app embeds trusted registry public keys, never private keys.
- The generated registry root is signed with an online registry signing key stored as a protected GitHub Actions secret/environment secret.
- Package SHA-256 digests are inside the signed registry.
- Official status is granted only by a signed registry record in the reserved `rayslash.*` namespace. Manifest text alone never grants official status.
- Community submissions cannot use `rayslash.*`, the `rayslash` author identity, official badges, or official key IDs.
- Registry key rotation is authorized by a release of rayslash that trusts both old and new public keys for an overlap period.
- The signing key is not the GitHub account password, SSH key, GPG key, or code-signing key. It is dedicated to the registry.
- If the signing key is exposed, stop registry publishing, revoke the workflow secret, remove the affected key in the next client release, and regenerate the catalog with a new key.

## 3. Historical pre-migration baseline

This was the starting state used to plan the migration; it is retained only to explain the version-1 compatibility path:

- An internal `Provider` trait and stable built-in `ProviderId` values exist.
- Provider outcomes, execution hints, diagnostics, permissions metadata, and typed activation actions exist.
- `modules.toml` exists and preserves unknown fields.
- The Settings Modules page lists seven official virtual modules and can enable/disable them.
- Legacy provider booleans are mirrored for compatibility.
- Core integration tests cover the current virtual-module state migration.

That transitional implementation has been replaced. Optional provider implementations now live in separate module repositories, fresh configurations are empty, and the UI uses the remote verified catalog lifecycle.

## 4. Work only the owner can do

### Step 1 — Choose the permanent GitHub owner

Choose one permanent public GitHub owner for the ecosystem.

Recommended: a GitHub organization controlled by at least two trusted maintainers. A personal account works for an initial release but creates a single-owner continuity risk.

The owner must be able to host these public repositories:

- `rayslash` — this app repository.
- `rayslash-registry` — catalog source, validation, generated index, and Pages site.
- `rayslash-module-sdk` — WIT contract, Rust SDK, validator CLI, templates, and author docs.
- `rayslash-module-host` — required sandbox host and IPC implementation, delivered automatically with the app package.
- `rayslash-module-calculator`.
- `rayslash-module-units`.
- `rayslash-module-currency`.
- `rayslash-module-time`.
- `rayslash-module-web-search`.
- `rayslash-module-timers`.
- `rayslash-module-aliases`.

Use one repository per official module. This tests the same workflow community authors use, permits independent updates, and keeps unrelated feature code out of the app and SDK repositories.

If a listed name is unavailable, choose the final replacement now and record it. Renaming after public releases complicates source matching and trust records.

### Step 2 — Create or transfer repositories

Create the repositories above as **public** repositories. Empty repositories are acceptable at this stage. Do not add generated code manually.

For each repository:

1. Set the default branch to `main`.
2. Enable issues.
3. Enable vulnerability reporting/private security advisories where available.
4. Disable wiki and projects unless you plan to maintain them.
5. Do not enable GitHub Packages; Release assets are sufficient.
6. Do not create a custom domain. The free `github.io` project URL is sufficient.

Transfer this app repository to the chosen owner if necessary. After transfer, update your local remote, but do not change Cargo metadata by hand; return the URL and Codex will update all source-of-truth files consistently.

Verify:

```sh
git remote -v
```

The current workspace has no configured Git remote, so adding the permanent remote is required before the final implementation can publish or test release links.

### Step 3 — Add trusted maintainers

For an organization:

1. Add at least one recovery maintainer besides yourself.
2. Require two-factor authentication for the organization.
3. Give ordinary contributors write access only where needed.
4. Restrict organization/repository administration and Actions secret access to trusted maintainers.

For a personal account, nominate a recovery maintainer and document how ownership will be transferred if the account becomes unavailable.

Return only GitHub usernames. Never return passwords, recovery codes, tokens, or private keys.

### Step 4 — Configure repository rules

Configure a ruleset for `main` in the app, registry, SDK, and host repositories:

- Require pull requests.
- Require at least one approving review for registry and host changes.
- Dismiss stale approvals after new commits.
- Require resolution of review conversations.
- Require status checks after workflows exist. This can be completed during final implementation because check names do not exist yet.
- Block force pushes and branch deletion.
- Allow repository administrators to bypass only for emergency recovery.

For official module repositories, require pull requests and passing package/API validation before merge.

Do not enable a rule that prevents the registry publishing workflow from updating its dedicated generated-output branch. Generated output should use a separate `pages` branch or a Pages artifact, never commits to protected `main`.

### Step 5 — Generate a dedicated registry signing key

Do this on a trusted local machine, not in a browser shell, Codespace, CI job, or shared computer.

The final implementation will include a repository script that generates the exact supported key format. Do not invent a key or GitHub secret yet. For now:

1. Decide which trusted maintainer machine will generate and retain the recovery copy.
2. Decide which second trusted maintainer will hold an encrypted recovery copy.
3. Prepare an encrypted offline storage location.
4. Choose a key ID such as `registry-2026-01` (the date is an identifier, not an expiration policy).

After Codex adds the key-generation script, you will run it once. It will output:

- a public key safe to commit and return;
- a private key to place in the protected registry GitHub Environment;
- a recovery copy that must remain encrypted and offline.

Never paste the private key into chat, an issue, a commit, a log, or the app repository. Return only the public key and key ID.

### Step 6 — Create the protected registry environment

In `rayslash-registry`, create a GitHub Actions environment named `registry-production`:

1. Restrict deployment branches to `main`.
2. Add required reviewers if your GitHub plan exposes that control. If it does not, rely on protected `main` and restricted repository administration; do not buy a plan solely for this control.
3. After the generation script exists, add the private key as `RAYSLASH_REGISTRY_SIGNING_KEY`.
4. Never add the key to repository-level variables, workflow files, test fixtures, or pull-request workflows.
5. The pull-request validator must not request this environment or receive this secret.
6. Only the post-merge publish workflow may read the secret.

The normal `GITHUB_TOKEN` should be used for public GitHub metadata. Do not create a personal access token unless testing proves the repository token cannot perform the required read operations.

### Step 7 — Enable GitHub Pages

Do this after Codex creates the registry publishing workflow:

1. Open `rayslash-registry` → Settings → Pages.
2. Select **GitHub Actions** as the source.
3. Run the publish workflow manually once.
4. Record the final HTTPS URL, normally `https://<owner>.github.io/rayslash-registry/`.
5. Confirm these URLs return HTTP 200:
   - `<pages-url>/v1/root.json`
   - `<pages-url>/v1/root.json.sig`
   - `<pages-url>/v1/index.json`
6. Confirm a raw fallback URL works from the generated output location.

Do not purchase a domain or hosting plan for this migration.

### Step 8 — Choose the moderation policy

Use this recommended initial policy:

- Every module enters through a pull request.
- Declarative modules require automated validation and one maintainer approval.
- WASM modules require automated validation, permission review, source/release reproducibility review, and one maintainer approval.
- New WASM modules are labeled `reviewed`, `limited-review`, or `blocked`; do not call a module secure.
- A module can be delisted immediately for malware, credential theft, undeclared behavior, impersonation, abandoned critical vulnerabilities, or legal takedown.
- Delisting prevents new installs/updates but does not remotely delete local files.
- A signed revocation record makes the app disable a known-malicious version and explain why. Re-enabling a revoked version is not supported from the normal UI.
- Maintainers do not promise a review deadline.
- Rejected submissions receive a documented reason and may resubmit.

If you want different rules, return the exact changes. Do not leave moderation undefined.

### Step 9 — Choose support and security contacts

Choose public destinations for:

- module author questions;
- registry submission problems;
- abuse reports;
- private security reports;
- general rayslash issues.

Recommended:

- GitHub Discussions in `rayslash-module-sdk` for author questions;
- issues in `rayslash-registry` for catalog problems;
- private vulnerability reporting/security advisories in the affected repository;
- a public `SECURITY.md` that explicitly says not to post exploitable reports publicly.

An email address is optional. Do not buy email service only for this migration.

### Step 10 — Confirm official module licenses and service policy

Default recommendation:

- App, SDK, host, registry code, and official modules: MIT.
- Each repository includes its own `LICENSE`; a license in this app does not cover a separate repository automatically.
- Third-party data/services remain attributed under their own terms.

Confirm whether the network modules may continue using:

- Frankfurter for currency rates;
- Open-Meteo for geocoding/timezone lookup;
- user-configured web search URLs.

The final implementation must keep providers replaceable and cache failures. No module may require a paid key. If a free public service changes its terms or limits, the affected module is updated independently without an app release.

### Step 11 — Preserve a migration test profile

Before installing a build that contains the final migration:

1. Stop rayslash.
2. Copy your current config and state to a private backup.
3. Do not commit or send the backup; it can contain local paths and commands.
4. Record only which virtual modules are currently enabled or disabled.

Typical paths:

```sh
mkdir -p "$HOME/rayslash-migration-backup"
config_dir="${XDG_CONFIG_HOME:-$HOME/.config}/rayslash"
state_dir="${XDG_STATE_HOME:-$HOME/.local/state}/rayslash"
test ! -d "$config_dir" || cp -a "$config_dir" "$HOME/rayslash-migration-backup/config"
test ! -d "$state_dir" || cp -a "$state_dir" "$HOME/rayslash-migration-backup/state"
```

If either source path does not exist, that is not an error. Keep the backup private.

### Step 12 — Do not manually move provider source files

Do not copy `calc`, `units.rs`, `currency.rs`, `time_lookup.rs`, `web_search.rs`, `utility_actions.rs`, or `aliases.rs` into new repositories yourself.

The extraction must happen together with:

- API adapters;
- behavior-preserving fixtures;
- package manifests;
- permission declarations;
- release workflows;
- migration compatibility tests;
- removal of built-in result/action variants;
- an app binary-size comparison.

Codex will perform that extraction after the repository URLs and public key are known.

## 5. Information to return

Return one message containing this completed block. Use `none` where optional. Do not include secrets.

```text
Permanent GitHub owner:
Owner type (organization or personal):
App repository URL:
Registry repository URL:
SDK repository URL:
Module host repository URL:
Calculator module repository URL:
Units module repository URL:
Currency module repository URL:
Time module repository URL:
Web Search module repository URL:
Timers module repository URL:
Aliases module repository URL:
GitHub Pages URL (or "pending workflow"):
Raw registry fallback URL (or "pending workflow"):
Registry signing key ID (or "pending script"):
Registry public key (or "pending script"):
Trusted maintainer GitHub usernames:
Module author support URL:
Registry support/abuse URL:
Security reporting URL:
License changes from MIT (or "none"):
Moderation policy changes from Step 8 (or "none"):
Continue Frankfurter (yes/no):
Continue Open-Meteo (yes/no):
Currently enabled virtual modules:
Currently disabled virtual modules:
Primary release architectures (recommended: x86_64 and aarch64):
Primary package targets to support immediately:
```

It is acceptable for the Pages URL and signing public key to say `pending`. Codex will first add the workflows/key script, then give you one short second owner-action checklist. All permanent repository URLs and policy choices are required before final implementation.

## 6. What Codex will implement after you return the information

The following is not manual work. It is the complete final implementation scope.

### Phase 1 — Freeze and specify API v1

- Add the WIT world, typed query/result/action/settings/diagnostic contracts, and lifecycle rules.
- Define API semantic-version compatibility and negotiation.
- Define stable provider/result/module ID grammar.
- Define trigger conflict rules and deterministic routing.
- Define declared and granted permissions.
- Define query cancellation, deadline, fuel, memory, output-size, and result-count limits.
- Define module state/config/cache scopes and quotas.
- Define typed host actions; no shell string action.
- Define error codes that remain stable across UI wording changes.
- Publish manifest and registry JSON Schemas.
- Add conformance fixtures and golden tests.

Exit criteria: the SDK, host, app, registry, and template use the same versioned contracts, and incompatible changes require API v2.

### Phase 2 — Build registry and author tooling

- Scaffold `rayslash-registry` with source submissions, generator, validator, signatures, shards, revocations, Pages site, issue forms, and workflows.
- Scaffold `rayslash-module-sdk` with WIT, Rust SDK, CLI validator, local test harness, starter templates, author guide, API reference, release guide, troubleshooting, security policy, contribution guide, and changelog policy.
- Provide one-command local validation and packaging.
- Make release workflows attach deterministic `.tar.zst` assets and checksums.
- Ensure pull-request workflows never receive the registry signing secret.
- Cache GitHub owner, stars, forks, archive status, topics, license, and update timestamp in generated metadata.
- Generate a small signed root plus search index and per-module shards.
- Add ETag/Last-Modified handling and documented stale-cache behavior.

Exit criteria: a sample third-party repository can be validated, packaged, submitted by pull request, indexed, signed, and browsed without a custom backend.

### Phase 3 — Implement the local package manager

- Replace the virtual-only descriptor list with installed, available, update, incompatible, revoked, broken, and disabled states.
- Add XDG paths for package data, config, state, cache, registry cache, downloads, staging, and rollback metadata.
- Validate IDs, URLs, manifests, archive paths, file types, sizes, duplicate paths, and digest.
- Use atomic staging/activation and retain the last working version until the new version starts successfully.
- Add install locks and recover abandoned staging directories safely.
- Never overwrite module config/state during install/update.
- Support uninstall with separate `keep data` and `remove data` choices.
- Keep installed modules usable offline.
- Add update permission diffs and explicit approval for expanded permissions.
- Block downgrades/replays unless the user deliberately chooses a developer override.
- Add registry key rotation and signed revocations.

Exit criteria: interruption, corrupt download, bad signature/hash, disk-full, invalid archive, or failed startup leaves the current version intact.

### Phase 4 — Implement the catalog UI

- Add Installed, Official, and Community sections with search.
- Show icon, module/author/source identity, version, compatibility, review status, permissions, GitHub stars, license, update state, and errors.
- Add install, cancel, retry, update, enable, disable, remove, keep/delete data, open source/star, and diagnostics actions.
- Add details and permission-diff views.
- Add offline/stale registry indicators without hiding installed modules.
- Add accessible keyboard behavior, focus management, progress, wrapping, and compact-layout tests.
- Rank exact/fuzzy relevance before logarithmic stars; do not let popularity defeat direct matches.
- Never send launcher queries, installed module IDs, paths, usernames, or hostnames to the registry.

Exit criteria: every lifecycle state is operable without editing files.

### Phase 5 — Build the WASM host

- Implement `rayslash-module-host` as a separate process and release artifact.
- Use the component model with only rayslash WIT imports; do not provide ambient WASI.
- Add process handshake, API negotiation, per-call IDs, cancellation, deadlines, fuel, memory limits, output limits, and crash recovery.
- Validate all guest strings, URLs, IDs, actions, and result counts at the host boundary and again in the launcher.
- Route approved network requests through the launcher with HTTPS-only origin/method/header/body/response caps and cache policy.
- Route filesystem, clipboard, notifications, and command actions through narrow typed host capabilities.
- Prohibit query-time process execution and shell evaluation.
- Add logs with secret/path redaction and user-visible diagnostics.
- Release signed/checksummed x86_64 and aarch64 binaries for selected package targets.

Exit criteria: malicious/looping/oversized test modules are terminated without freezing the launcher or escaping granted capabilities.

### Phase 6 — Extract all seven official modules

- Port Calculator and Units to WASM with no permissions.
- Port Currency to WASM with only the declared HTTPS service origin and cache permission.
- Port Time to WASM with only the declared HTTPS service origin and cache permission; package timezone data deliberately rather than assuming host files.
- Port Web Search to WASM with user-owned settings plus typed explicit-open actions.
- Port Aliases to WASM with user-owned settings plus typed actions; command aliases require a high-risk permission and never use a shell.
- Split Timers into safe notification/timer behavior and privileged power actions with explicit activation/permission messaging.
- Preserve current query syntax, ordering, stable IDs where possible, error behavior, caching, and settings.
- Publish each module through the same registry path as community modules.
- Delete the extracted implementations and module-specific dependencies from the app once parity tests pass.

Exit criteria: installing each official module restores its old behavior, and none of its provider implementation ships in a fresh app.

### Phase 7 — Migrate existing users safely

- Bump `modules.toml` to version 2 while preserving unknown data.
- Detect version-1 virtual built-ins and record their previous enabled state.
- Fresh install: no optional modules installed or enabled.
- Existing install: show one migration screen listing previously enabled modules and required downloads/permissions.
- Do not silently download executable code. A single confirmed action may install all selected official replacements.
- Preserve aliases, web-search templates, module enable choices, and provider state during confirmed migration.
- Keep a backup and a reversible migration marker until success.
- If offline, leave the migration pending and keep the app core usable.
- Remove legacy provider booleans only after at least one documented compatibility cycle.

Exit criteria: fresh, upgraded-online, upgraded-offline, partially migrated, cancelled, and corrupt-config cases all have tests and understandable UI.

### Phase 8 — Packaging and release integration

- Update Cargo metadata and all docs to permanent repository URLs.
- Make the module host a separate RPM/Arch artifact required by the app package.
- Bundle the pinned host artifact in Flatpak and test its execution boundary.
- Ensure no official module packages enter the app RPM, Arch package, Flatpak base app, or source install.
- Add reproducible release/build provenance where supported without making it the only trust mechanism.
- Publish separate x86_64/aarch64 Fedora host RPMs from checksum-pinned immutable host release inputs, and include the verified host RPM in the app's official architecture-matched Fedora package sets. CI must prove DNF resolves the dependency without bundling the host in the app RPM.
- Measure fresh installed size and release binary size before/after; document both the core and host cost.
- Add upgrade/rollback release notes and emergency registry-key rotation steps.

Exit criteria: package inventories prove that the fresh app contains no official optional module code/assets.

### Phase 9 — Complete verification

- Unit/integration/property tests for manifests, semver, IDs, URL normalization, signatures, digests, safe extraction, atomic installs, permission diffs, registry fallback, ranking, migrations, and IPC.
- Adversarial fixtures for traversal, symlinks, duplicate paths, decompression bombs, huge manifests, malformed UTF-8, forged official identity, stale/replayed registry, revoked packages, and hostile WASM.
- End-to-end tests using a local HTTP registry and package server; CI must not depend on live GitHub for correctness tests.
- UI tests for the complete lifecycle and offline/error states.
- Manual matrix for GNOME/KDE, Wayland/X11, Fedora, Arch, Ubuntu/Debian, openSUSE, native install, and Flatpak.
- Performance budgets for startup, catalog load, local queries, WASM cold/warm calls, memory, and package size.
- Documentation link and command checks.

Exit criteria: all automated checks pass, the manual matrix is recorded, and no unresolved critical/high security issue remains.

## 7. Required public documentation at completion

Codex will prepare these; do not write them manually:

- Module quickstart.
- Full module API v1 reference generated from WIT plus behavior notes.
- `module.toml` reference and JSON Schema.
- Declarative module guide and examples.
- Rust WASM module guide and template.
- Local validator/test harness guide.
- Permissions and security model.
- Packaging and GitHub Release guide.
- Compatibility and API versioning policy.
- Submission/review/delisting policy.
- Registry format and mirror/cache behavior.
- Settings/config/state/cache API and quotas.
- Typed actions and activation guide.
- Migration guide for former bundled providers.
- Troubleshooting and diagnostics.
- Maintainer release, signing, key rotation, incident, and revocation runbooks.

The docs must label API v1 as stable only after the conformance suite and host implementation pass. Until then, examples are development previews and must not promise compatibility.

## 8. Final acceptance checklist

Do not call the migration complete until every box is true:

- [x] Fresh install has zero optional modules.
- [x] Fresh app binary/package has no extracted official provider implementation or assets.
- [x] Apps and Folders work without registry or module host.
- [x] Supported app packages automatically install or include the module host.
- [x] Unsupported declarative packages are rejected consistently instead of appearing installable.
- [x] WASM modules require and use the separate sandbox host.
- [x] All seven official modules install on demand and match current behavior.
- [x] Community author quickstart works from an empty public repository.
- [x] Submission is validated and merged through a registry pull request.
- [x] Registry and package integrity are checked before activation.
- [x] Official identity cannot be forged by manifest fields.
- [x] Permission expansion requires approval.
- [x] Installed modules work when Pages/GitHub is unavailable.
- [x] Failed updates roll back without data loss.
- [x] Uninstall removes code and offers a separate data choice.
- [x] Revocation and signing-key rotation are tested.
- [x] Existing version-1 virtual-module users receive a safe opt-in migration.
- [ ] Fresh, upgrade, offline, malformed, interrupted, and hostile cases are tested.
- [x] Native and Flatpak packaging contain no default official modules.
- [x] Public author/API/security/submission documentation is complete.
- [ ] Community modules can declare user-editable settings rendered by a generic validated Settings form.
- [x] Binary and installed-size measurements are recorded.
- [ ] All CI and the manual Linux verification matrix pass.

## 9. Implementation handoff (2026-07-12)

Owner setup is complete. The production registry key is `registry-2026-01` with public key `JetgdjNVvSrVDWLYhY4D3fYAohnm6LiRtp+7rSQNJAo=`, Pages and raw fallbacks are live, and the protected signing workflow has published successfully.

Implemented artifacts:

- SDK API v1, manifest schema, validator/packager, author/API/release documentation, and immutable release tag.
- No-WASI host with bounded capabilities, persistent launcher IPC, x86_64/aarch64 releases, and separate Fedora/Arch recipes.
- Official reproducible Fedora 44 x86_64/aarch64 host RPMs and checksum sidecars are published on the host v0.1.2 release. App CI verifies those immutable assets, combines them with architecture-matched app RPMs, and exercises dependency resolution with a DNF dry run before app-release publication.
- Signed registry generator, protected publish workflow, public key, and seven live-fetched official submission records.
- Verified registry client/cache, digest-pinned safe atomic package installation, install/update/remove lifecycle, separate keep/delete-data removal, and permission display.
- Calculator, Units, Currency, Time, Web Search, Timers, and Aliases in separate repositories and successful GitHub Releases.
- Core reduced to Apps and Folders. Extracted Calculator, Units, Currency, Time, and Timers source implementations and module-specific dependencies are removed from the app.
- Fresh configurations create zero optional module entries. Existing version-1 entries become explicit `Restore` choices and never download silently.
- Live production signed registry → app installer → package verifier → persistent host probes pass for Calculator, Units, Currency, Time, Web Search, Timers, and Aliases. The probe exercises representative results, the Open-Meteo service boundary, Calculator's 250 ms warm-call budget, and complete removal in an isolated XDG profile.
- Interrupted install state is reconciled at startup. Removal uses recoverable same-filesystem staging, commits generated state atomically, stops stale host processes, and cleans superseded package versions without touching module-owned config/state during updates.
- Revoked installed versions are blocked at runtime and shown as revoked/removable in Settings. Signing-key overlap is tested with independently generated retiring/replacement keys.
- Registry and installed-state module IDs use the SDK's bounded reverse-DNS grammar at the client boundary; installed paths must be derivable from their verified ID, version, and digest before lifecycle cleanup can touch them.

The SDK, host, and all seven official-module pull requests were merged. Registry PR 4 was merged and its protected production deployment was approved. App PRs 3 and 4 were merged in dependency order; PR 5 contains the final runtime and lifecycle safeguards.

Do not delete module release tags or replace their assets. Do not place the registry private key in any repository. After the registry merge, the app will discover the seven records through its normal signed refresh; no URL or key edit is required.

## 10. Remaining release verification

No additional owner input or repository bootstrap is required. The following environment matrix remains release work rather than a claim that the module migration is already complete. Automate each check where a suitable runner exists; do not ask the owner to repeat source/signing setup. Before publishing the first end-user app release:

1. Build the Fedora RPM and Arch package on clean x86_64 builders; repeat on aarch64 hardware or builders.
2. Install the app package and confirm its transaction automatically installs `rayslash-module-host` (or, for Flatpak, that `/app/libexec/rayslash/rayslash-module-host` is included). Confirm Settings lists Installed, Official, and Community modules while no module package exists under the XDG data directory.
3. Install and exercise all seven official modules from Settings without any additional runtime setup. Confirm enable/disable, update, Remove, and Remove + data.
4. Repeat the launcher/window/shortcut checks on GNOME Wayland and KDE Plasma Wayland. Test X11 sessions where the distribution still provides them.
5. Disconnect networking after a successful catalog refresh. Confirm installed modules execute and the verified cached catalog remains visible.
6. Record the distribution, desktop/session, architecture, package versions, and pass/fail result in the release notes. Report any failure with the exact command, log, and environment; do not work around it by bundling modules into the app.
7. Verify that the direct Flatpak bundle contains `/app/libexec/rayslash/rayslash-module-host`, discovers host desktop entries, and starts selected host actions through `flatpak-spawn`. The Flatpak contains the runtime host but no official or community modules; its broad launcher permissions are documented and are not represented as Flathub-ready least privilege.

### Local signed-registry verification

Development builds can use an isolated registry only when `rayslash-core/registry-dev-override` is compiled in and debug assertions are enabled. Start a loopback HTTP server for a locally built and signed registry, then set `RAYSLASH_DEV_REGISTRY_ROOT`, `RAYSLASH_DEV_REGISTRY_KEY_ID`, and `RAYSLASH_DEV_REGISTRY_PUBLIC_KEY` before launching. The root, index, revocations, signature, digests, and package manifests are still verified normally. Loopback HTTP is accepted only in this gated mode.

For example, build the registry with a loopback base URL, sign `public/v1/root.json` with a disposable development key, serve `public/` on `127.0.0.1`, and run `cargo run -p rayslash --features rayslash-core/registry-dev-override`. Release builds and ordinary debug builds do not compile this override path; their URLs and trusted keys remain the production constants.

## 11. Current free-service verification

These assumptions were checked on 2026-07-11 and must be rechecked before a public launch:

- GitHub Pages is available for public repositories on GitHub Free.
- Standard GitHub-hosted Actions runners are free for public repositories.
- GitHub Pages documents a 1 GB site limit, a soft 100 GB/month bandwidth limit, and rate limiting; therefore Pages is not the only read path.
- Unauthenticated GitHub REST requests are limited to 60/hour per originating IP; therefore clients browse generated static metadata instead of crawling GitHub.
- GitHub Releases permit release assets below 2 GiB and document no total release-size or bandwidth limit; rayslash will impose much smaller package limits.
- GitHub recommends release assets rather than generated source archives when archive security matters.

Primary references:

- [GitHub Pages quickstart](https://docs.github.com/en/pages/quickstart)
- [GitHub Pages limits](https://docs.github.com/en/pages/getting-started-with-github-pages/github-pages-limits)
- [GitHub Actions billing](https://docs.github.com/en/billing/concepts/product-billing/github-actions)
- [GitHub REST API rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)
- [GitHub Releases](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases)
- [GitHub source archive stability](https://docs.github.com/en/repositories/working-with-files/using-files/downloading-source-code-archives#stability-of-source-code-archives)
