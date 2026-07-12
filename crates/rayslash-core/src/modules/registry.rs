use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{APP_NAME, atomic_write};

pub const DEFAULT_REGISTRY_ROOT_URL: &str =
    "https://rslauncher.github.io/rayslash-registry/v1/root.json";
pub const RAW_REGISTRY_ROOT_URL: &str =
    "https://raw.githubusercontent.com/rslauncher/rayslash-registry/registry/v1/root.json";
const REGISTRY_KEY_ID: &str = "registry-2026-01";
const REGISTRY_PUBLIC_KEY: &str = "JetgdjNVvSrVDWLYhY4D3fYAohnm6LiRtp+7rSQNJAo=";
const MAX_ROOT_BYTES: u64 = 64 * 1024;
const MAX_INDEX_BYTES: u64 = 8 * 1024 * 1024;
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
pub struct RegistryModule {
    pub id: String,
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
    pub source_url: String,
    pub from_cache: bool,
}

struct FetchedRegistry {
    root_bytes: Vec<u8>,
    signature_bytes: Vec<u8>,
    index_bytes: Vec<u8>,
    root: RegistryRoot,
    index: RegistryIndex,
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
                save_cache(
                    &fetched.root_bytes,
                    &fetched.signature_bytes,
                    &fetched.index_bytes,
                )?;
                return Ok(RegistryRefresh {
                    root: fetched.root,
                    index: fetched.index,
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

pub fn load_cached_registry() -> Result<RegistryRefresh, RegistryError> {
    let directory = registry_cache_dir().ok_or(RegistryError::CacheUnavailable)?;
    let root_bytes = read_limited(&directory.join("root.json"), MAX_ROOT_BYTES)?;
    let signature_bytes = read_limited(&directory.join("root.json.sig"), MAX_SIGNATURE_BYTES)?;
    let index_bytes = read_limited(&directory.join("index.json"), MAX_INDEX_BYTES)?;
    let (root, index) = verify_registry_bytes(&root_bytes, &signature_bytes, &index_bytes)?;
    Ok(RegistryRefresh {
        root,
        index,
        source_url: directory.display().to_string(),
        from_cache: true,
    })
}

pub fn verify_registry_bytes(
    root_bytes: &[u8],
    signature_bytes: &[u8],
    index_bytes: &[u8],
) -> Result<(RegistryRoot, RegistryIndex), RegistryError> {
    if root_bytes.len() as u64 > MAX_ROOT_BYTES
        || signature_bytes.len() as u64 > MAX_SIGNATURE_BYTES
        || index_bytes.len() as u64 > MAX_INDEX_BYTES
    {
        return Err(RegistryError::Invalid(
            "registry response exceeds its size limit".into(),
        ));
    }
    let root: RegistryRoot = serde_json::from_slice(root_bytes)
        .map_err(|error| RegistryError::Invalid(format!("root JSON: {error}")))?;
    if root.schema_version != 1 || root.key_id != REGISTRY_KEY_ID {
        return Err(RegistryError::Invalid(
            "unsupported schema or signing key".into(),
        ));
    }
    let key: [u8; 32] = STANDARD
        .decode(REGISTRY_PUBLIC_KEY)
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
    let index: RegistryIndex = serde_json::from_slice(index_bytes)
        .map_err(|error| RegistryError::Invalid(format!("index JSON: {error}")))?;
    if index.schema_version != 1 {
        return Err(RegistryError::Invalid("unsupported index schema".into()));
    }
    validate_index(&index)?;
    Ok((root, index))
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
    let index_bytes = fetch_limited(&unsigned_root.index_url, MAX_INDEX_BYTES)?;
    let (root, index) = verify_registry_bytes(&root_bytes, &signature_bytes, &index_bytes)?;
    Ok(FetchedRegistry {
        root_bytes,
        signature_bytes,
        index_bytes,
        root,
        index,
    })
}

fn fetch_limited(url: &str, limit: u64) -> Result<Vec<u8>, RegistryError> {
    ureq::get(url)
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
        if !module.repository.starts_with("https://github.com/") || module.versions.is_empty() {
            return Err(RegistryError::Invalid(format!(
                "invalid source or empty versions for {}",
                module.id
            )));
        }
        previous = Some(module.id.as_str());
    }
    Ok(())
}

fn save_cache(root: &[u8], signature: &[u8], index: &[u8]) -> Result<(), RegistryError> {
    let directory = registry_cache_dir().ok_or(RegistryError::CacheUnavailable)?;
    fs::create_dir_all(&directory).map_err(|source| RegistryError::Write {
        path: directory.clone(),
        source,
    })?;
    for (name, contents) in [
        ("root.json", root),
        ("root.json.sig", signature),
        ("index.json", index),
    ] {
        let path = directory.join(name);
        atomic_write::write_bytes(&path, contents)
            .map_err(|source| RegistryError::Write { path, source })?;
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
            verify_registry_bytes(root, b"invalid", b"{}"),
            Err(RegistryError::Invalid(_))
        ));
    }

    #[test]
    fn live_registry_fixture_verifies_with_production_key() {
        let root = include_bytes!("../../tests/fixtures/module-registry/root.json");
        let signature = include_bytes!("../../tests/fixtures/module-registry/root.json.sig");
        let index = include_bytes!("../../tests/fixtures/module-registry/index.json");
        let (_, index) = verify_registry_bytes(root, signature, index).expect("verified fixture");
        assert!(index.modules.is_empty());
    }
}
