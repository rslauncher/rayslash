use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    io::{self, Cursor, Read},
    path::{Component, Path, PathBuf},
    process,
    time::Duration,
};

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{APP_NAME, atomic_write, config};

use super::{RegistryModule, RegistryVersion};

const INSTALLED_STATE_VERSION: u32 = 1;
const MAX_COMPRESSED_BYTES: u64 = 32 * 1024 * 1024;
const MAX_EXTRACTED_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ENTRY_BYTES: u64 = 16 * 1024 * 1024;
const MAX_ENTRIES: usize = 256;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackageKind {
    Declarative,
    Wasm,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PackagePermissions {
    pub network: Vec<String>,
    pub cache: bool,
    pub clipboard: bool,
    pub notifications: bool,
    pub commands: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageProvider {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub triggers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModulePackageManifest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub version: Version,
    pub api_version: VersionReq,
    pub license: String,
    pub source: String,
    #[serde(default)]
    pub homepage: Option<String>,
    pub icon: PathBuf,
    pub kind: PackageKind,
    #[serde(default)]
    pub permissions: PackagePermissions,
    pub providers: Vec<PackageProvider>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledModules {
    pub version: u32,
    #[serde(default)]
    pub modules: BTreeMap<String, InstalledModule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledModule {
    pub version: Version,
    pub digest: String,
    pub source: String,
    pub source_commit: String,
    pub install_path: PathBuf,
    pub enabled: bool,
}

impl Default for InstalledModules {
    fn default() -> Self {
        Self {
            version: INSTALLED_STATE_VERSION,
            modules: BTreeMap::new(),
        }
    }
}

#[derive(Debug)]
pub enum PackageError {
    DirectoryUnavailable,
    Network(String),
    Io { path: PathBuf, source: io::Error },
    Invalid(String),
    Parse(toml::de::Error),
    Serialize(toml::ser::Error),
}

impl fmt::Display for PackageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectoryUnavailable => {
                formatter.write_str("module data/state directory is unavailable")
            }
            Self::Network(message) => write!(formatter, "module download failed: {message}"),
            Self::Io { path, source } => write!(
                formatter,
                "module package I/O failed at {}: {source}",
                path.display()
            ),
            Self::Invalid(message) => write!(formatter, "invalid module package: {message}"),
            Self::Parse(error) => write!(formatter, "invalid module manifest/state TOML: {error}"),
            Self::Serialize(error) => write!(
                formatter,
                "could not serialize installed module state: {error}"
            ),
        }
    }
}

impl std::error::Error for PackageError {}

pub fn install_registry_version(
    module: &RegistryModule,
    version: &RegistryVersion,
) -> Result<InstalledModule, PackageError> {
    if module.review_status == super::ReviewStatus::Blocked {
        return Err(PackageError::Invalid(
            "this module is blocked by registry moderation".into(),
        ));
    }
    if super::load_cached_registry().ok().is_some_and(|registry| {
        super::installed_revocation(
            &registry.revocations,
            &module.id,
            &version.version,
            &version.sha256,
        )
        .is_some()
    }) {
        return Err(PackageError::Invalid(
            "this module version was revoked by the signed registry".into(),
        ));
    }
    if version.size == 0 || version.size > MAX_COMPRESSED_BYTES {
        return Err(PackageError::Invalid(
            "registry package size is outside limits".into(),
        ));
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .into();
    let bytes = agent
        .get(&version.asset_url)
        .header("User-Agent", "rayslash/0.1 module-installer")
        .call()
        .map_err(|error| PackageError::Network(error.to_string()))?
        .into_body()
        .with_config()
        .limit(MAX_COMPRESSED_BYTES)
        .read_to_vec()
        .map_err(|error| PackageError::Network(error.to_string()))?;
    if bytes.len() as u64 != version.size || sha256(&bytes) != version.sha256.to_ascii_lowercase() {
        return Err(PackageError::Invalid(
            "downloaded package size or digest does not match the signed registry".into(),
        ));
    }
    let modules_dir = modules_data_dir().ok_or(PackageError::DirectoryUnavailable)?;
    fs::create_dir_all(&modules_dir).map_err(|source| io_error(&modules_dir, source))?;
    let _install_lock = InstallLock::acquire(&modules_dir)?;
    remove_abandoned_staging(&modules_dir)?;
    let existing = load_installed_modules()?.modules.remove(&module.id);
    let mut repair_broken = false;
    if let Some(installed) = existing.as_ref() {
        if version.version < installed.version {
            return Err(PackageError::Invalid(format!(
                "refusing to downgrade {} from {} to {}",
                module.id, installed.version, version.version
            )));
        }
        if version.version == installed.version {
            if version.sha256.eq_ignore_ascii_case(&installed.digest) {
                let existing_manifest =
                    fs::read_to_string(installed.install_path.join("module.toml"))
                        .ok()
                        .and_then(|text| toml::from_str::<ModulePackageManifest>(&text).ok());
                if existing_manifest.as_ref().is_some_and(|manifest| {
                    manifest.id == module.id
                        && manifest.version == version.version
                        && installed.install_path.join("module.wasm").is_file()
                        && super::runtime::probe_wasm_module(
                            &module.id,
                            &installed.install_path,
                            manifest,
                        )
                        .is_ok()
                }) {
                    return Ok(installed.clone());
                }
                repair_broken = true;
            } else {
                return Err(PackageError::Invalid(format!(
                    "version {} was already installed with a different digest",
                    version.version
                )));
            }
        }
    }
    let staging = modules_dir.join(format!(
        ".staging-{}-{}",
        safe_filename(&module.id),
        process::id()
    ));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|source| io_error(&staging, source))?;
    }
    fs::create_dir(&staging).map_err(|source| io_error(&staging, source))?;

    let result = (|| {
        let manifest = extract_package(&bytes, &staging)?;
        validate_manifest(&manifest, module, version, &staging)?;
        let digest = sha256(&bytes);
        let destination =
            modules_dir
                .join(&module.id)
                .join(format!("{}-{}", version.version, &digest[..16]));
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
        }
        if repair_broken && destination.exists() {
            fs::remove_dir_all(&destination).map_err(|source| io_error(&destination, source))?;
        }
        let destination_existed = destination.exists();
        if destination_existed {
            fs::remove_dir_all(&staging).map_err(|source| io_error(&staging, source))?;
        } else {
            fs::rename(&staging, &destination).map_err(|source| io_error(&destination, source))?;
        }
        if manifest.kind == PackageKind::Wasm
            && let Err(error) =
                super::runtime::probe_wasm_module(&module.id, &destination, &manifest)
        {
            if !destination_existed {
                let _ = fs::remove_dir_all(&destination);
            }
            return Err(PackageError::Invalid(format!(
                "module failed its startup probe: {error}"
            )));
        }
        let installed = InstalledModule {
            version: version.version.clone(),
            digest,
            source: module.repository.clone(),
            source_commit: version.source_commit.clone(),
            install_path: destination,
            enabled: true,
        };
        let mut state = load_installed_modules()?;
        state.modules.insert(module.id.clone(), installed.clone());
        save_installed_modules(&state)?;
        Ok(installed)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

struct InstallLock {
    path: PathBuf,
}

impl InstallLock {
    fn acquire(modules_dir: &Path) -> Result<Self, PackageError> {
        let path = modules_dir.join(".install.lock");
        for attempt in 0..2 {
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            match options.open(&path) {
                Ok(mut file) => {
                    use std::io::Write;
                    writeln!(file, "{}", process::id())
                        .map_err(|source| io_error(&path, source))?;
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists && attempt == 0 => {
                    if lock_owner_is_alive(&path) {
                        return Err(PackageError::Invalid(
                            "another module install or update is already running".into(),
                        ));
                    }
                    fs::remove_file(&path).map_err(|source| io_error(&path, source))?;
                }
                Err(source) => return Err(io_error(&path, source)),
            }
        }
        Err(PackageError::Invalid(
            "could not acquire the module install lock".into(),
        ))
    }
}

impl Drop for InstallLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn lock_owner_is_alive(path: &Path) -> bool {
    let Ok(value) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(pid) = value.trim().parse::<u32>() else {
        return false;
    };
    Path::new("/proc").join(pid.to_string()).exists()
}

fn remove_abandoned_staging(modules_dir: &Path) -> Result<(), PackageError> {
    for entry in fs::read_dir(modules_dir).map_err(|source| io_error(modules_dir, source))? {
        let entry = entry.map_err(|source| io_error(modules_dir, source))?;
        if entry.file_name().to_string_lossy().starts_with(".staging-") {
            let path = entry.path();
            if path.is_dir() {
                fs::remove_dir_all(&path).map_err(|source| io_error(&path, source))?;
            } else {
                fs::remove_file(&path).map_err(|source| io_error(&path, source))?;
            }
        }
    }
    Ok(())
}

pub fn load_installed_modules() -> Result<InstalledModules, PackageError> {
    let path = installed_modules_file().ok_or(PackageError::DirectoryUnavailable)?;
    match fs::read_to_string(&path) {
        Ok(contents) => {
            let state: InstalledModules = toml::from_str(&contents).map_err(PackageError::Parse)?;
            if state.version != INSTALLED_STATE_VERSION {
                return Err(PackageError::Invalid(format!(
                    "unsupported installed state version {}",
                    state.version
                )));
            }
            Ok(state)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(InstalledModules::default()),
        Err(source) => Err(io_error(&path, source)),
    }
}

pub fn remove_installed_module(module_id: &str, remove_data: bool) -> Result<bool, PackageError> {
    let modules_dir = modules_data_dir().ok_or(PackageError::DirectoryUnavailable)?;
    fs::create_dir_all(&modules_dir).map_err(|source| io_error(&modules_dir, source))?;
    let _install_lock = InstallLock::acquire(&modules_dir)?;
    let mut state = load_installed_modules()?;
    let Some(installed) = state.modules.remove(module_id) else {
        return Ok(false);
    };
    if installed.install_path.exists() {
        fs::remove_dir_all(&installed.install_path)
            .map_err(|source| io_error(&installed.install_path, source))?;
    }
    if remove_data {
        for directory in [
            module_config_dir(module_id),
            module_state_dir(module_id),
            module_cache_dir(module_id),
        ] {
            if directory.exists() {
                fs::remove_dir_all(&directory).map_err(|source| io_error(&directory, source))?;
            }
        }
    }
    save_installed_modules(&state)?;
    Ok(true)
}

pub fn installed_modules_file() -> Option<PathBuf> {
    config::state_dir().map(|path| path.join("modules/installed.toml"))
}

fn save_installed_modules(state: &InstalledModules) -> Result<(), PackageError> {
    let path = installed_modules_file().ok_or(PackageError::DirectoryUnavailable)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
    }
    let contents = toml::to_string_pretty(state).map_err(PackageError::Serialize)?;
    atomic_write::write(&path, &contents).map_err(|source| io_error(&path, source))
}

fn extract_package(bytes: &[u8], staging: &Path) -> Result<ModulePackageManifest, PackageError> {
    if bytes.len() as u64 > MAX_COMPRESSED_BYTES {
        return Err(PackageError::Invalid(
            "compressed package exceeds limit".into(),
        ));
    }
    let decoder = zstd::Decoder::new(Cursor::new(bytes))
        .map_err(|error| PackageError::Invalid(format!("zstd stream: {error}")))?;
    let mut archive = tar::Archive::new(decoder);
    let mut paths = BTreeSet::new();
    let mut root = None;
    let mut extracted = 0_u64;
    let mut manifest_text = None;
    for (index, entry) in archive
        .entries()
        .map_err(|error| PackageError::Invalid(format!("tar stream: {error}")))?
        .enumerate()
    {
        if index >= MAX_ENTRIES {
            return Err(PackageError::Invalid("package has too many entries".into()));
        }
        let mut entry =
            entry.map_err(|error| PackageError::Invalid(format!("tar entry: {error}")))?;
        if !entry.header().entry_type().is_file() {
            return Err(PackageError::Invalid(
                "only regular files are allowed".into(),
            ));
        }
        let path = entry
            .path()
            .map_err(|error| PackageError::Invalid(format!("entry path: {error}")))?
            .into_owned();
        if !safe_archive_path(&path) {
            return Err(PackageError::Invalid("unsafe package path".into()));
        }
        let mut components = path.components();
        let first = components
            .next()
            .and_then(|value| match value {
                Component::Normal(value) => Some(value.to_owned()),
                _ => None,
            })
            .ok_or_else(|| PackageError::Invalid("missing package root".into()))?;
        if root.as_ref().is_some_and(|known| known != &first) {
            return Err(PackageError::Invalid(
                "package must have one top-level directory".into(),
            ));
        }
        root.get_or_insert(first);
        let relative = components.as_path();
        if relative.as_os_str().is_empty() || !paths.insert(relative.to_path_buf()) {
            return Err(PackageError::Invalid(
                "empty or duplicate package path".into(),
            ));
        }
        let size = entry
            .header()
            .size()
            .map_err(|error| PackageError::Invalid(format!("entry size: {error}")))?;
        if size > MAX_ENTRY_BYTES || extracted.saturating_add(size) > MAX_EXTRACTED_BYTES {
            return Err(PackageError::Invalid(
                "extracted package exceeds limits".into(),
            ));
        }
        extracted += size;
        let target = staging.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
        }
        let mut data = Vec::with_capacity(size as usize);
        entry
            .read_to_end(&mut data)
            .map_err(|source| io_error(&target, source))?;
        if relative == Path::new("module.toml") {
            if size > MAX_MANIFEST_BYTES {
                return Err(PackageError::Invalid("manifest exceeds limit".into()));
            }
            manifest_text = Some(
                String::from_utf8(data.clone())
                    .map_err(|_| PackageError::Invalid("manifest is not UTF-8".into()))?,
            );
        }
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        use std::io::Write;
        options
            .open(&target)
            .and_then(|mut file| file.write_all(&data))
            .map_err(|source| io_error(&target, source))?;
    }
    toml::from_str(
        &manifest_text.ok_or_else(|| PackageError::Invalid("package has no module.toml".into()))?,
    )
    .map_err(PackageError::Parse)
}

fn validate_manifest(
    manifest: &ModulePackageManifest,
    module: &RegistryModule,
    version: &RegistryVersion,
    root: &Path,
) -> Result<(), PackageError> {
    if manifest.kind == PackageKind::Declarative {
        return Err(PackageError::Invalid(
            "declarative packages are reserved for a future API; use a WASM package with API v1"
                .into(),
        ));
    }
    if manifest.id != module.id
        || manifest.name != module.name
        || manifest.description != module.description
        || manifest.author != module.author
        || manifest.license != module.license
        || manifest.kind != module.kind
        || manifest.permissions != module.permissions
        || manifest.version != version.version
        || manifest.api_version != version.api_version
        || manifest.source != module.repository
    {
        return Err(PackageError::Invalid(
            "manifest identity does not match signed registry metadata".into(),
        ));
    }
    if manifest.providers.is_empty()
        || manifest.icon.is_absolute()
        || manifest
            .icon
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(PackageError::Invalid(
            "manifest providers or icon path are invalid".into(),
        ));
    }
    if !root.join(&manifest.icon).is_file() {
        return Err(PackageError::Invalid("manifest icon is missing".into()));
    }
    if manifest.kind == PackageKind::Wasm && !root.join("module.wasm").is_file() {
        return Err(PackageError::Invalid(
            "WASM module has no module.wasm".into(),
        ));
    }
    Ok(())
}

fn modules_data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|path| path.join(APP_NAME).join("modules"))
}
fn module_config_dir(id: &str) -> PathBuf {
    config::config_dir()
        .unwrap_or_default()
        .join("modules")
        .join(id)
}
fn module_state_dir(id: &str) -> PathBuf {
    config::state_dir()
        .unwrap_or_default()
        .join("modules")
        .join(id)
}
fn module_cache_dir(id: &str) -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_default()
        .join(APP_NAME)
        .join("modules")
        .join(id)
}
fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn safe_filename(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
fn safe_archive_path(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}
fn io_error(path: &Path, source: io::Error) -> PackageError {
    PackageError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_extractor_rejects_parent_traversal() {
        assert!(!safe_archive_path(Path::new("root/../escape")));
        assert!(!safe_archive_path(Path::new("/absolute")));
        assert!(safe_archive_path(Path::new("root/module.toml")));
    }
}
