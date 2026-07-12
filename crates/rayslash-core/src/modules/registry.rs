use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{APP_NAME, atomic_write};

use super::{PackageKind, PackagePermissions};

pub const DEFAULT_REGISTRY_ROOT_URL: &str =
    "https://rslauncher.github.io/rayslash-registry/v1/root.json";
pub const RAW_REGISTRY_ROOT_URL: &str =
    "https://raw.githubusercontent.com/rslauncher/rayslash-registry/registry/v1/root.json";
// Keep the retiring and replacement keys here together for one launcher release during a
// rotation. Remove the retiring key only in a later release after the new root is deployed.
const TRUSTED_REGISTRY_KEYS: &[(&str, &str)] = &[(
    "registry-2026-01",
    "JetgdjNVvSrVDWLYhY4D3fYAohnm6LiRtp+7rSQNJAo=",
)];
const MAX_ROOT_BYTES: u64 = 64 * 1024;
const MAX_INDEX_BYTES: u64 = 8 * 1024 * 1024;
const MAX_REVOCATIONS_BYTES: u64 = 1024 * 1024;
const MAX_SIGNATURE_BYTES: u64 = 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryRoot {
    pub schema_version: u32,
    pub generated_at: String,
    pub key_id: String,
    pub index_url: String,
    pub index_sha256: String,
    pub revocations_url: String,
    pub revocations_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryIndex {
    pub schema_version: u32,
    pub generated_at: String,
    pub modules: Vec<RegistryModule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryRevocations {
    pub schema_version: u32,
    pub generated_at: String,
    pub revoked: Vec<RegistryRevocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryRevocation {
    pub module_id: String,
    pub version: Version,
    pub sha256: String,
    pub reason: String,
    pub revoked_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryModule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub license: String,
    pub kind: PackageKind,
    pub permissions: PackagePermissions,
    pub repository: String,
    pub official: bool,
    pub review_status: ReviewStatus,
    pub github_stars: u64,
    pub updated_at: String,
    pub versions: Vec<RegistryVersion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewStatus {
    Reviewed,
    LimitedReview,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryVersion {
    pub version: Version,
    pub api_version: VersionReq,
    pub source_commit: String,
    pub asset_url: String,
    pub sha256: String,
    pub size: u64,
    pub yanked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryRefresh {
    pub root: RegistryRoot,
    pub index: RegistryIndex,
    pub revocations: RegistryRevocations,
    pub source_url: String,
    pub from_cache: bool,
}

struct FetchedRegistry {
    root_bytes: Vec<u8>,
    signature_bytes: Vec<u8>,
    index_bytes: Vec<u8>,
    revocations_bytes: Vec<u8>,
    root: RegistryRoot,
    index: RegistryIndex,
    revocations: RegistryRevocations,
}

#[derive(Debug)]
pub enum RegistryError {
    CacheUnavailable,
    Read { path: PathBuf, source: io::Error },
    Write { path: PathBuf, source: io::Error },
    Network { url: String, message: String },
    Invalid(String),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CacheUnavailable => {
                formatter.write_str("module registry cache directory is unavailable")
            }
            Self::Read { path, source } => {
                write!(formatter, "failed to read {}: {source}", path.display())
            }
            Self::Write { path, source } => {
                write!(formatter, "failed to write {}: {source}", path.display())
            }
            Self::Network { url, message } => write!(formatter, "failed to fetch {url}: {message}"),
            Self::Invalid(message) => write!(formatter, "invalid module registry: {message}"),
        }
    }
}

impl std::error::Error for RegistryError {}

pub fn refresh_registry() -> Result<RegistryRefresh, RegistryError> {
    let mut errors = Vec::new();
    for root_url in [DEFAULT_REGISTRY_ROOT_URL, RAW_REGISTRY_ROOT_URL] {
        match fetch_registry(root_url) {
            Ok(fetched) => {
                reject_registry_rollback(&fetched.root)?;
                save_cache(
                    &fetched.root_bytes,
                    &fetched.signature_bytes,
                    &fetched.index_bytes,
                    &fetched.revocations_bytes,
                )?;
                return Ok(RegistryRefresh {
                    root: fetched.root,
                    index: fetched.index,
                    revocations: fetched.revocations,
                    source_url: root_url.to_owned(),
                    from_cache: false,
                });
            }
            Err(error) => errors.push(error.to_string()),
        }
    }
    load_cached_registry().map_err(|cache_error| RegistryError::Network {
        url: format!("{DEFAULT_REGISTRY_ROOT_URL}, {RAW_REGISTRY_ROOT_URL}"),
        message: format!(
            "{}; cached fallback failed: {cache_error}",
            errors.join("; ")
        ),
    })
}

fn reject_registry_rollback(new_root: &RegistryRoot) -> Result<(), RegistryError> {
    let Ok(cached) = load_cached_registry() else {
        return Ok(());
    };
    let parse = |value: &str| {
        value
            .parse::<DateTime<Utc>>()
            .map_err(|error| RegistryError::Invalid(format!("generated_at timestamp: {error}")))
    };
    let new_time = parse(&new_root.generated_at)?;
    let cached_time = parse(&cached.root.generated_at)?;
    if new_time < cached_time {
        return Err(RegistryError::Invalid(
            "registry root is older than the last verified root".into(),
        ));
    }
    if new_time == cached_time
        && (new_root.index_sha256 != cached.root.index_sha256
            || new_root.revocations_sha256 != cached.root.revocations_sha256
            || new_root.key_id != cached.root.key_id)
    {
        return Err(RegistryError::Invalid(
            "registry root conflicts with the last verified root timestamp".into(),
        ));
    }
    Ok(())
}

pub fn load_cached_registry() -> Result<RegistryRefresh, RegistryError> {
    let directory = registry_cache_dir().ok_or(RegistryError::CacheUnavailable)?;
    let snapshot = match fs::read_to_string(directory.join("CURRENT")) {
        Ok(value) => {
            let id = value.trim();
            if id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(RegistryError::Invalid(
                    "registry cache pointer is invalid".into(),
                ));
            }
            directory.join("snapshots").join(id)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => directory.clone(),
        Err(source) => {
            return Err(RegistryError::Read {
                path: directory.join("CURRENT"),
                source,
            });
        }
    };
    let root_bytes = read_limited(&snapshot.join("root.json"), MAX_ROOT_BYTES)?;
    let signature_bytes = read_limited(&snapshot.join("root.json.sig"), MAX_SIGNATURE_BYTES)?;
    let index_bytes = read_limited(&snapshot.join("index.json"), MAX_INDEX_BYTES)?;
    let revocations_bytes =
        read_limited(&snapshot.join("revocations.json"), MAX_REVOCATIONS_BYTES)?;
    let (root, mut index, revocations) = verify_registry_bytes(
        &root_bytes,
        &signature_bytes,
        &index_bytes,
        &revocations_bytes,
    )?;
    apply_revocations(&mut index, &revocations);
    Ok(RegistryRefresh {
        root,
        index,
        revocations,
        source_url: directory.display().to_string(),
        from_cache: true,
    })
}

pub fn verify_registry_bytes(
    root_bytes: &[u8],
    signature_bytes: &[u8],
    index_bytes: &[u8],
    revocations_bytes: &[u8],
) -> Result<(RegistryRoot, RegistryIndex, RegistryRevocations), RegistryError> {
    if root_bytes.len() as u64 > MAX_ROOT_BYTES
        || signature_bytes.len() as u64 > MAX_SIGNATURE_BYTES
        || index_bytes.len() as u64 > MAX_INDEX_BYTES
        || revocations_bytes.len() as u64 > MAX_REVOCATIONS_BYTES
    {
        return Err(RegistryError::Invalid(
            "registry response exceeds its size limit".into(),
        ));
    }
    let root: RegistryRoot = serde_json::from_slice(root_bytes)
        .map_err(|error| RegistryError::Invalid(format!("root JSON: {error}")))?;
    if root.schema_version != 1 {
        return Err(RegistryError::Invalid("unsupported root schema".into()));
    }
    let encoded_key = TRUSTED_REGISTRY_KEYS
        .iter()
        .find_map(|(id, key)| (*id == root.key_id).then_some(*key))
        .ok_or_else(|| RegistryError::Invalid("untrusted registry signing key".into()))?;
    let key: [u8; 32] = STANDARD
        .decode(encoded_key)
        .map_err(|error| RegistryError::Invalid(format!("embedded public key: {error}")))?
        .try_into()
        .map_err(|_| RegistryError::Invalid("embedded public key length".into()))?;
    let signature: [u8; 64] = STANDARD
        .decode(String::from_utf8_lossy(signature_bytes).trim())
        .map_err(|error| RegistryError::Invalid(format!("root signature encoding: {error}")))?
        .try_into()
        .map_err(|_| RegistryError::Invalid("root signature length".into()))?;
    VerifyingKey::from_bytes(&key)
        .map_err(|error| RegistryError::Invalid(format!("public key: {error}")))?
        .verify(root_bytes, &Signature::from_bytes(&signature))
        .map_err(|_| RegistryError::Invalid("root signature verification failed".into()))?;
    if sha256(index_bytes) != root.index_sha256.to_ascii_lowercase() {
        return Err(RegistryError::Invalid("index digest mismatch".into()));
    }
    if sha256(revocations_bytes) != root.revocations_sha256.to_ascii_lowercase() {
        return Err(RegistryError::Invalid("revocations digest mismatch".into()));
    }
    let index: RegistryIndex = serde_json::from_slice(index_bytes)
        .map_err(|error| RegistryError::Invalid(format!("index JSON: {error}")))?;
    if index.schema_version != 1 {
        return Err(RegistryError::Invalid("unsupported index schema".into()));
    }
    validate_index(&index)?;
    let revocations: RegistryRevocations = serde_json::from_slice(revocations_bytes)
        .map_err(|error| RegistryError::Invalid(format!("revocations JSON: {error}")))?;
    validate_revocations(&revocations)?;
    Ok((root, index, revocations))
}

fn fetch_registry(root_url: &str) -> Result<FetchedRegistry, RegistryError> {
    let signature_url = format!("{root_url}.sig");
    let root_bytes = fetch_limited(root_url, MAX_ROOT_BYTES)?;
    let signature_bytes = fetch_limited(&signature_url, MAX_SIGNATURE_BYTES)?;
    let unsigned_root: RegistryRoot = serde_json::from_slice(&root_bytes)
        .map_err(|error| RegistryError::Invalid(format!("root JSON: {error}")))?;
    if !unsigned_root.index_url.starts_with("https://") {
        return Err(RegistryError::Invalid("index URL must use HTTPS".into()));
    }
    if !unsigned_root.revocations_url.starts_with("https://") {
        return Err(RegistryError::Invalid(
            "revocations URL must use HTTPS".into(),
        ));
    }
    let index_bytes = fetch_limited(&unsigned_root.index_url, MAX_INDEX_BYTES)?;
    let revocations_bytes = fetch_limited(&unsigned_root.revocations_url, MAX_REVOCATIONS_BYTES)?;
    let (root, mut index, revocations) = verify_registry_bytes(
        &root_bytes,
        &signature_bytes,
        &index_bytes,
        &revocations_bytes,
    )?;
    apply_revocations(&mut index, &revocations);
    Ok(FetchedRegistry {
        root_bytes,
        signature_bytes,
        index_bytes,
        revocations_bytes,
        root,
        index,
        revocations,
    })
}

fn fetch_limited(url: &str, limit: u64) -> Result<Vec<u8>, RegistryError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .into();
    agent
        .get(url)
        .header("User-Agent", "rayslash/0.1 module-registry")
        .call()
        .map_err(|error| RegistryError::Network {
            url: url.to_owned(),
            message: error.to_string(),
        })?
        .into_body()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .map_err(|error| RegistryError::Network {
            url: url.to_owned(),
            message: error.to_string(),
        })
}

fn validate_index(index: &RegistryIndex) -> Result<(), RegistryError> {
    let mut previous = None;
    for module in &index.modules {
        if module.id.is_empty() || previous.is_some_and(|value: &str| value >= module.id.as_str()) {
            return Err(RegistryError::Invalid(
                "module IDs must be non-empty, unique, and sorted".into(),
            ));
        }
        if module.official != module.id.starts_with("rayslash.") {
            return Err(RegistryError::Invalid(format!(
                "official identity mismatch for {}",
                module.id
            )));
        }
        if module.kind != PackageKind::Wasm {
            return Err(RegistryError::Invalid(format!(
                "{} uses a package kind unsupported by API v1",
                module.id
            )));
        }
        if module.official != module.author.eq_ignore_ascii_case("rayslash") {
            return Err(RegistryError::Invalid(format!(
                "official author mismatch for {}",
                module.id
            )));
        }
        if module.name.is_empty()
            || module.description.is_empty()
            || module.author.is_empty()
            || module.license.is_empty()
            || !module.repository.starts_with("https://github.com/")
            || module.versions.is_empty()
        {
            return Err(RegistryError::Invalid(format!(
                "invalid source or empty versions for {}",
                module.id
            )));
        }
        for origin in &module.permissions.network {
            let Some(authority) = origin.strip_prefix("https://") else {
                return Err(RegistryError::Invalid(format!(
                    "invalid network origin for {}",
                    module.id
                )));
            };
            if authority.is_empty() || authority.contains(['/', '?', '#', '@']) {
                return Err(RegistryError::Invalid(format!(
                    "invalid network origin for {}",
                    module.id
                )));
            }
        }
        let mut versions = std::collections::BTreeSet::new();
        for version in &module.versions {
            if !versions.insert(&version.version)
                || version.source_commit.len() != 40
                || !version
                    .source_commit
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())
                || !version
                    .asset_url
                    .starts_with(&format!("{}/releases/download/", module.repository))
                || version.sha256.len() != 64
                || !version.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
                || version.size == 0
                || version.size > 32 * 1024 * 1024
            {
                return Err(RegistryError::Invalid(format!(
                    "invalid version record for {}",
                    module.id
                )));
            }
        }
        previous = Some(module.id.as_str());
    }
    Ok(())
}

fn validate_revocations(revocations: &RegistryRevocations) -> Result<(), RegistryError> {
    if revocations.schema_version != 1 {
        return Err(RegistryError::Invalid(
            "unsupported revocations schema".into(),
        ));
    }
    let mut previous = None;
    for revocation in &revocations.revoked {
        let key = (
            &revocation.module_id,
            &revocation.version,
            &revocation.sha256,
        );
        if revocation.module_id.is_empty()
            || revocation.reason.is_empty()
            || revocation.revoked_at.is_empty()
            || revocation.sha256.len() != 64
            || !revocation
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || previous.as_ref().is_some_and(|value| value >= &key)
        {
            return Err(RegistryError::Invalid(
                "revocations must be complete, unique, and sorted".into(),
            ));
        }
        previous = Some(key);
    }
    Ok(())
}

pub fn installed_revocation<'a>(
    revocations: &'a RegistryRevocations,
    module_id: &str,
    version: &Version,
    digest: &str,
) -> Option<&'a RegistryRevocation> {
    revocations.revoked.iter().find(|revocation| {
        revocation.module_id == module_id
            && &revocation.version == version
            && revocation.sha256.eq_ignore_ascii_case(digest)
    })
}

fn apply_revocations(index: &mut RegistryIndex, revocations: &RegistryRevocations) {
    for module in &mut index.modules {
        for version in &mut module.versions {
            if installed_revocation(revocations, &module.id, &version.version, &version.sha256)
                .is_some()
            {
                version.yanked = true;
            }
        }
    }
}

fn save_cache(
    root: &[u8],
    signature: &[u8],
    index: &[u8],
    revocations: &[u8],
) -> Result<(), RegistryError> {
    let directory = registry_cache_dir().ok_or(RegistryError::CacheUnavailable)?;
    let snapshots = directory.join("snapshots");
    fs::create_dir_all(&snapshots).map_err(|source| RegistryError::Write {
        path: snapshots.clone(),
        source,
    })?;
    let previous_id = fs::read_to_string(directory.join("CURRENT"))
        .ok()
        .map(|value| value.trim().to_owned());
    let id = sha256(root);
    let destination = snapshots.join(&id);
    let staging = snapshots.join(format!(".{id}.tmp-{}", std::process::id()));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|source| RegistryError::Write {
            path: staging.clone(),
            source,
        })?;
    }
    fs::create_dir(&staging).map_err(|source| RegistryError::Write {
        path: staging.clone(),
        source,
    })?;
    for (name, contents) in [
        ("root.json", root),
        ("root.json.sig", signature),
        ("index.json", index),
        ("revocations.json", revocations),
    ] {
        let path = staging.join(name);
        fs::write(&path, contents).map_err(|source| RegistryError::Write { path, source })?;
    }
    if destination.exists() {
        fs::remove_dir_all(&staging).map_err(|source| RegistryError::Write {
            path: staging,
            source,
        })?;
    } else {
        fs::rename(&staging, &destination).map_err(|source| RegistryError::Write {
            path: destination.clone(),
            source,
        })?;
    }
    let pointer = directory.join("CURRENT");
    atomic_write::write_bytes(&pointer, format!("{id}\n").as_bytes()).map_err(|source| {
        RegistryError::Write {
            path: pointer,
            source,
        }
    })?;
    if let Ok(entries) = fs::read_dir(&snapshots) {
        for entry in entries.flatten() {
            if entry.file_name() != id.as_str()
                && previous_id
                    .as_deref()
                    .is_none_or(|previous| entry.file_name() != previous)
                && entry.path().is_dir()
            {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }
    Ok(())
}

fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, RegistryError> {
    let metadata = fs::metadata(path).map_err(|source| RegistryError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.len() > limit {
        return Err(RegistryError::Invalid(format!(
            "{} exceeds its size limit",
            path.display()
        )));
    }
    fs::read(path).map_err(|source| RegistryError::Read {
        path: path.to_path_buf(),
        source,
    })
}

fn registry_cache_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|path| path.join(APP_NAME).join("module-registry"))
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_signatures_before_index_use() {
        let root = br#"{"schema_version":1,"generated_at":"now","key_id":"registry-2026-01","index_url":"https://example.test/index.json","index_sha256":"00","revocations_url":"https://example.test/revocations.json","revocations_sha256":"00"}"#;
        assert!(matches!(
            verify_registry_bytes(root, b"invalid", b"{}", b"{}"),
            Err(RegistryError::Invalid(_))
        ));
    }

    #[test]
    fn live_registry_fixture_verifies_with_production_key() {
        let root = include_bytes!("../../tests/fixtures/module-registry/root.json");
        let signature = include_bytes!("../../tests/fixtures/module-registry/root.json.sig");
        let index = include_bytes!("../../tests/fixtures/module-registry/index.json");
        let revocations = include_bytes!("../../tests/fixtures/module-registry/revocations.json");
        let (_, index, revocations) =
            verify_registry_bytes(root, signature, index, revocations).expect("verified fixture");
        assert!(index.modules.is_empty());
        assert!(revocations.revoked.is_empty());
    }

    #[test]
    fn signed_root_rejects_modified_revocations() {
        let root = include_bytes!("../../tests/fixtures/module-registry/root.json");
        let signature = include_bytes!("../../tests/fixtures/module-registry/root.json.sig");
        let index = include_bytes!("../../tests/fixtures/module-registry/index.json");
        assert!(matches!(
            verify_registry_bytes(root, signature, index, br#"{"schema_version":1,"generated_at":"now","revoked":[]}"#),
            Err(RegistryError::Invalid(message)) if message == "revocations digest mismatch"
        ));
    }

    #[test]
    fn revocation_yanks_only_the_exact_package_digest() {
        let mut index = RegistryIndex {
            schema_version: 1,
            generated_at: "2026-01-01T00:00:00Z".into(),
            modules: vec![RegistryModule {
                id: "io.github.example.module".into(),
                name: "Example".into(),
                description: "Example module".into(),
                author: "example".into(),
                license: "MIT".into(),
                kind: PackageKind::Wasm,
                permissions: PackagePermissions::default(),
                repository: "https://github.com/example/module".into(),
                official: false,
                review_status: ReviewStatus::Reviewed,
                github_stars: 0,
                updated_at: "2026-01-01T00:00:00Z".into(),
                versions: vec![RegistryVersion {
                    version: Version::new(1, 0, 0),
                    api_version: VersionReq::parse("^1.0").unwrap(),
                    source_commit: "0".repeat(40),
                    asset_url:
                        "https://github.com/example/module/releases/download/v1/module.tar.zst"
                            .into(),
                    sha256: "a".repeat(64),
                    size: 1,
                    yanked: false,
                }],
            }],
        };
        let revocations = RegistryRevocations {
            schema_version: 1,
            generated_at: "2026-01-01T00:00:00Z".into(),
            revoked: vec![RegistryRevocation {
                module_id: "io.github.example.module".into(),
                version: Version::new(1, 0, 0),
                sha256: "a".repeat(64),
                reason: "security issue".into(),
                revoked_at: "2026-01-01T00:00:00Z".into(),
            }],
        };
        apply_revocations(&mut index, &revocations);
        assert!(index.modules[0].versions[0].yanked);
    }
}
