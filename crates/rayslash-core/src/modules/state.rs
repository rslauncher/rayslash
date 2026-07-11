use std::{
    collections::BTreeMap,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    atomic_write,
    config::{self, ProviderConfig},
};

use super::{
    ALIASES_MODULE_ID, CALCULATOR_MODULE_ID, CURRENCY_MODULE_ID, TIME_MODULE_ID, TIMERS_MODULE_ID,
    UNITS_MODULE_ID, WEB_SEARCH_MODULE_ID, official_module_descriptors,
};

pub const MODULES_CONFIG_VERSION: u32 = 1;
const STABLE_CHANNEL: &str = "stable";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModulesConfig {
    pub version: u32,
    #[serde(default)]
    pub modules: BTreeMap<String, ModuleEntryConfig>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModuleEntryConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownModuleError {
    pub module_id: String,
}

impl fmt::Display for UnknownModuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown official module: {}", self.module_id)
    }
}

impl std::error::Error for UnknownModuleError {}

#[derive(Debug)]
pub enum LoadModulesConfigError {
    Read {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    UnsupportedVersion {
        path: PathBuf,
        version: u32,
    },
}

#[derive(Debug)]
pub enum SaveModulesConfigError {
    CreateDir { path: PathBuf, source: io::Error },
    UnsupportedVersion { version: u32 },
    Serialize { source: toml::ser::Error },
    Write { path: PathBuf, source: io::Error },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModulesConfigLoadOutcome {
    Loaded(ModulesConfig),
    Created(ModulesConfig),
}

#[derive(Debug)]
pub enum InitializeModulesConfigError {
    ConfigDirUnavailable,
    Load(LoadModulesConfigError),
    Save(SaveModulesConfigError),
}

impl fmt::Display for LoadModulesConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(
                    f,
                    "failed to read module config {}: {source}",
                    path.display()
                )
            }
            Self::Parse { path, source } => {
                write!(
                    f,
                    "failed to parse module config {}: {source}",
                    path.display()
                )
            }
            Self::UnsupportedVersion { path, version } => write!(
                f,
                "unsupported module config version {version} in {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for LoadModulesConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::UnsupportedVersion { .. } => None,
        }
    }
}

impl fmt::Display for SaveModulesConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateDir { path, source } => write!(
                f,
                "failed to create module config directory {}: {source}",
                path.display()
            ),
            Self::UnsupportedVersion { version } => {
                write!(f, "cannot save unsupported module config version {version}")
            }
            Self::Serialize { source } => {
                write!(f, "failed to serialize module config: {source}")
            }
            Self::Write { path, source } => {
                write!(
                    f,
                    "failed to write module config {}: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for SaveModulesConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CreateDir { source, .. } => Some(source),
            Self::UnsupportedVersion { .. } => None,
            Self::Serialize { source } => Some(source),
            Self::Write { source, .. } => Some(source),
        }
    }
}

impl ModulesConfigLoadOutcome {
    pub fn config(&self) -> &ModulesConfig {
        match self {
            Self::Loaded(config) | Self::Created(config) => config,
        }
    }

    pub fn into_config(self) -> ModulesConfig {
        match self {
            Self::Loaded(config) | Self::Created(config) => config,
        }
    }

    pub fn was_created(&self) -> bool {
        matches!(self, Self::Created(_))
    }
}

impl fmt::Display for InitializeModulesConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfigDirUnavailable => {
                write!(f, "system config directory is unavailable")
            }
            Self::Load(error) => error.fmt(f),
            Self::Save(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for InitializeModulesConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ConfigDirUnavailable => None,
            Self::Load(error) => Some(error),
            Self::Save(error) => Some(error),
        }
    }
}

impl ModuleEntryConfig {
    fn built_in(version: &str, enabled: bool) -> Self {
        Self {
            enabled,
            version: Some(version.to_owned()),
            channel: Some(STABLE_CHANNEL.to_owned()),
            extra: BTreeMap::new(),
        }
    }
}

impl ModulesConfig {
    pub fn from_legacy_provider_config(providers: &ProviderConfig) -> Self {
        let mut config = Self {
            version: MODULES_CONFIG_VERSION,
            modules: BTreeMap::new(),
            extra: BTreeMap::new(),
        };
        config.seed_missing_official_modules(providers);
        config
    }

    pub fn seed_missing_official_modules(&mut self, providers: &ProviderConfig) {
        for descriptor in official_module_descriptors() {
            let enabled = legacy_enabled(descriptor.id, providers).unwrap_or(true);
            self.modules
                .entry(descriptor.id.to_owned())
                .or_insert_with(|| ModuleEntryConfig::built_in(descriptor.version, enabled));
        }
    }

    pub fn is_enabled(&self, module_id: &str) -> Option<bool> {
        self.modules.get(module_id).map(|entry| entry.enabled)
    }

    pub fn is_official_enabled(&self, module_id: &str) -> Option<bool> {
        super::official_module_descriptor(module_id)?;
        self.is_enabled(module_id)
    }

    pub fn set_enabled(
        &mut self,
        module_id: &str,
        enabled: bool,
    ) -> Result<bool, UnknownModuleError> {
        let descriptor =
            super::official_module_descriptor(module_id).ok_or_else(|| UnknownModuleError {
                module_id: module_id.to_owned(),
            })?;
        let entry = self
            .modules
            .entry(module_id.to_owned())
            .or_insert_with(|| ModuleEntryConfig::built_in(descriptor.version, true));
        let changed = entry.enabled != enabled;
        entry.enabled = enabled;
        Ok(changed)
    }

    pub fn enable(&mut self, module_id: &str) -> Result<bool, UnknownModuleError> {
        self.set_enabled(module_id, true)
    }

    pub fn disable(&mut self, module_id: &str) -> Result<bool, UnknownModuleError> {
        self.set_enabled(module_id, false)
    }

    pub fn apply_to_provider_config(&self, providers: &mut ProviderConfig) {
        apply_if_present(self, CALCULATOR_MODULE_ID, &mut providers.calculator);
        apply_if_present(self, ALIASES_MODULE_ID, &mut providers.aliases);
        apply_if_present(self, WEB_SEARCH_MODULE_ID, &mut providers.web_search);
        apply_if_present(self, UNITS_MODULE_ID, &mut providers.unit_conversion);
        apply_if_present(self, CURRENCY_MODULE_ID, &mut providers.currency_conversion);
        apply_if_present(self, TIME_MODULE_ID, &mut providers.time_lookup);
        apply_if_present(self, TIMERS_MODULE_ID, &mut providers.utility_actions);
    }

    pub fn applied_provider_config(&self, providers: &ProviderConfig) -> ProviderConfig {
        let mut applied = providers.clone();
        self.apply_to_provider_config(&mut applied);
        applied
    }

    pub fn mirror_from_provider_config(&mut self, providers: &ProviderConfig) {
        mirror_entry(self, CALCULATOR_MODULE_ID, providers.calculator);
        mirror_entry(self, ALIASES_MODULE_ID, providers.aliases);
        mirror_entry(self, WEB_SEARCH_MODULE_ID, providers.web_search);
        mirror_entry(self, UNITS_MODULE_ID, providers.unit_conversion);
        mirror_entry(self, CURRENCY_MODULE_ID, providers.currency_conversion);
        mirror_entry(self, TIME_MODULE_ID, providers.time_lookup);
        mirror_entry(self, TIMERS_MODULE_ID, providers.utility_actions);
    }
}

impl Default for ModulesConfig {
    fn default() -> Self {
        Self::from_legacy_provider_config(&ProviderConfig::default())
    }
}

pub fn modules_config_file() -> Option<PathBuf> {
    config::config_dir().map(|path| path.join("modules.toml"))
}

pub fn load_modules_config(
    legacy_providers: &ProviderConfig,
) -> Result<ModulesConfig, LoadModulesConfigError> {
    let Some(path) = modules_config_file() else {
        return Ok(ModulesConfig::from_legacy_provider_config(legacy_providers));
    };
    load_modules_config_from_path(&path, legacy_providers)
}

pub fn load_or_create_modules_config(
    legacy_providers: &ProviderConfig,
) -> Result<ModulesConfigLoadOutcome, InitializeModulesConfigError> {
    let path = modules_config_file().ok_or(InitializeModulesConfigError::ConfigDirUnavailable)?;
    load_or_create_modules_config_from_path(&path, legacy_providers)
}

pub fn load_or_create_modules_config_from_path(
    path: &Path,
    legacy_providers: &ProviderConfig,
) -> Result<ModulesConfigLoadOutcome, InitializeModulesConfigError> {
    match fs::read_to_string(path) {
        Ok(contents) => parse_modules_config(path, &contents, legacy_providers)
            .map(ModulesConfigLoadOutcome::Loaded)
            .map_err(InitializeModulesConfigError::Load),
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            let config = ModulesConfig::from_legacy_provider_config(legacy_providers);
            save_modules_config_to_path(path, &config)
                .map_err(InitializeModulesConfigError::Save)?;
            Ok(ModulesConfigLoadOutcome::Created(config))
        }
        Err(source) => Err(InitializeModulesConfigError::Load(
            LoadModulesConfigError::Read {
                path: path.to_path_buf(),
                source,
            },
        )),
    }
}

pub fn load_modules_config_from_path(
    path: &Path,
    legacy_providers: &ProviderConfig,
) -> Result<ModulesConfig, LoadModulesConfigError> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Ok(ModulesConfig::from_legacy_provider_config(legacy_providers));
        }
        Err(source) => {
            return Err(LoadModulesConfigError::Read {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    parse_modules_config(path, &contents, legacy_providers)
}

pub fn save_modules_config(config: &ModulesConfig) -> Result<(), SaveModulesConfigError> {
    let Some(path) = modules_config_file() else {
        return Err(SaveModulesConfigError::Write {
            path: PathBuf::from("modules.toml"),
            source: io::Error::new(
                io::ErrorKind::NotFound,
                "system config directory is unavailable",
            ),
        });
    };
    save_modules_config_to_path(&path, config)
}

pub fn save_modules_config_to_path(
    path: &Path,
    config: &ModulesConfig,
) -> Result<(), SaveModulesConfigError> {
    if config.version != MODULES_CONFIG_VERSION {
        return Err(SaveModulesConfigError::UnsupportedVersion {
            version: config.version,
        });
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SaveModulesConfigError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let contents = toml::to_string_pretty(config)
        .map_err(|source| SaveModulesConfigError::Serialize { source })?;
    atomic_write::write(path, &contents).map_err(|source| SaveModulesConfigError::Write {
        path: path.to_path_buf(),
        source,
    })
}

fn validate_version_for_load(path: &Path, version: u32) -> Result<(), LoadModulesConfigError> {
    if version == MODULES_CONFIG_VERSION {
        Ok(())
    } else {
        Err(LoadModulesConfigError::UnsupportedVersion {
            path: path.to_path_buf(),
            version,
        })
    }
}

fn parse_modules_config(
    path: &Path,
    contents: &str,
    legacy_providers: &ProviderConfig,
) -> Result<ModulesConfig, LoadModulesConfigError> {
    let mut config: ModulesConfig =
        toml::from_str(contents).map_err(|source| LoadModulesConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
    validate_version_for_load(path, config.version)?;
    config.seed_missing_official_modules(legacy_providers);
    Ok(config)
}

fn default_enabled() -> bool {
    true
}

fn legacy_enabled(module_id: &str, providers: &ProviderConfig) -> Option<bool> {
    match module_id {
        CALCULATOR_MODULE_ID => Some(providers.calculator),
        ALIASES_MODULE_ID => Some(providers.aliases),
        WEB_SEARCH_MODULE_ID => Some(providers.web_search),
        UNITS_MODULE_ID => Some(providers.unit_conversion),
        CURRENCY_MODULE_ID => Some(providers.currency_conversion),
        TIME_MODULE_ID => Some(providers.time_lookup),
        TIMERS_MODULE_ID => Some(providers.utility_actions),
        _ => None,
    }
}

fn apply_if_present(config: &ModulesConfig, module_id: &str, target: &mut bool) {
    if let Some(enabled) = config.is_enabled(module_id) {
        *target = enabled;
    }
}

fn mirror_entry(config: &mut ModulesConfig, module_id: &str, enabled: bool) {
    if let Some(entry) = config.modules.get_mut(module_id) {
        entry.enabled = enabled;
    } else if let Some(descriptor) = super::official_module_descriptor(module_id) {
        config.modules.insert(
            module_id.to_owned(),
            ModuleEntryConfig::built_in(descriptor.version, enabled),
        );
    }
}
