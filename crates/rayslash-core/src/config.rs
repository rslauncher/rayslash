use std::{
    env, fmt, fs, io,
    io::Write,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{APP_NAME, atomic_write};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default, alias = "project_roots")]
    pub folder_sources: Vec<PathBuf>,
    #[serde(default)]
    pub aliases: Vec<AliasConfig>,
    #[serde(default)]
    pub providers: ProviderConfig,
    #[serde(default)]
    pub actions: ActionConfig,
    #[serde(default)]
    pub appearance: AppearanceConfig,
    #[serde(default)]
    pub ranking: RankingConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConfig {
    #[serde(default = "default_true")]
    pub apps: bool,
    #[serde(default = "default_true", alias = "projects")]
    pub folders: bool,
    #[serde(default = "default_true")]
    pub calculator: bool,
    #[serde(default = "default_true")]
    pub aliases: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AliasConfig {
    pub name: String,
    pub query: String,
    pub target: String,
    #[serde(default)]
    pub kind: Option<AliasKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AliasKind {
    Url,
    File,
    Folder,
    Command,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionConfig {
    #[serde(default = "default_true")]
    pub alternate_folder_opener_enabled: bool,
    #[serde(
        default = "default_alternate_folder_opener_command",
        alias = "project_editor_command"
    )]
    pub alternate_folder_opener_command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppearanceConfig {
    #[serde(default)]
    pub theme: AppearanceTheme,
    #[serde(default)]
    pub density: AppearanceDensity,
    #[serde(default = "default_max_results")]
    pub max_results: usize,
    #[serde(default = "default_true")]
    pub show_tooltips: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppearanceTheme {
    #[default]
    Dark,
    Dim,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppearanceDensity {
    Compact,
    #[default]
    Comfortable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankingConfig {
    #[serde(default = "default_true")]
    pub learn_from_usage: bool,
}

#[derive(Debug)]
pub enum ConfigError {
    Read {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
}

#[derive(Debug)]
pub enum SaveConfigError {
    CreateDir { path: PathBuf, source: io::Error },
    Backup { path: PathBuf, source: io::Error },
    Serialize { source: toml::ser::Error },
    Write { path: PathBuf, source: io::Error },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(f, "failed to read config {}: {source}", path.display())
            }
            Self::Parse { path, source } => {
                write!(f, "failed to parse config {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
        }
    }
}

impl fmt::Display for SaveConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateDir { path, source } => {
                write!(
                    f,
                    "failed to create config directory {}: {source}",
                    path.display()
                )
            }
            Self::Backup { path, source } => {
                write!(f, "failed to back up config {}: {source}", path.display())
            }
            Self::Serialize { source } => write!(f, "failed to serialize config: {source}"),
            Self::Write { path, source } => {
                write!(f, "failed to write config {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for SaveConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CreateDir { source, .. } => Some(source),
            Self::Backup { source, .. } => Some(source),
            Self::Serialize { source } => Some(source),
            Self::Write { source, .. } => Some(source),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            folder_sources: default_folder_sources(),
            aliases: Vec::new(),
            providers: ProviderConfig::default(),
            actions: ActionConfig::default(),
            appearance: AppearanceConfig::default(),
            ranking: RankingConfig::default(),
        }
    }
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            apps: true,
            folders: true,
            calculator: true,
            aliases: true,
        }
    }
}

impl Default for ActionConfig {
    fn default() -> Self {
        Self {
            alternate_folder_opener_enabled: true,
            alternate_folder_opener_command: default_alternate_folder_opener_command(),
        }
    }
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme: AppearanceTheme::default(),
            density: AppearanceDensity::default(),
            max_results: default_max_results(),
            show_tooltips: true,
        }
    }
}

impl Default for RankingConfig {
    fn default() -> Self {
        Self {
            learn_from_usage: true,
        }
    }
}

impl Config {
    pub fn normalized(mut self) -> Self {
        self.folder_sources = normalize_folder_sources(self.folder_sources);
        self.aliases = crate::aliases::normalize_aliases(self.aliases);
        self.actions.alternate_folder_opener_command =
            normalize_command(self.actions.alternate_folder_opener_command);
        if self.appearance.max_results == 0 {
            self.appearance.max_results = default_max_results();
        }
        self
    }
}

pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|path| path.join(APP_NAME))
}

pub fn config_file() -> Option<PathBuf> {
    config_dir().map(|path| path.join("config.toml"))
}

pub fn state_dir() -> Option<PathBuf> {
    dirs::state_dir().map(|path| path.join(APP_NAME))
}

pub fn load_config() -> Result<Config, ConfigError> {
    let Some(path) = config_file() else {
        return Ok(Config::default());
    };

    load_config_from_path(&path)
}

pub fn load_config_from_path(path: &Path) -> Result<Config, ConfigError> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let config: Config =
                toml::from_str(&contents).map_err(|source| ConfigError::Parse {
                    path: path.to_path_buf(),
                    source,
                })?;

            Ok(config.normalized())
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(Config::default()),
        Err(source) => Err(ConfigError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

pub fn save_config(config: &Config) -> Result<(), SaveConfigError> {
    let Some(path) = config_file() else {
        return Err(SaveConfigError::Write {
            path: PathBuf::from("config.toml"),
            source: io::Error::new(
                io::ErrorKind::NotFound,
                "system config directory is unavailable",
            ),
        });
    };

    save_config_to_path(&path, config)
}

pub fn save_config_with_backup(config: &Config) -> Result<(), SaveConfigError> {
    let Some(path) = config_file() else {
        return Err(SaveConfigError::Write {
            path: PathBuf::from("config.toml"),
            source: io::Error::new(
                io::ErrorKind::NotFound,
                "system config directory is unavailable",
            ),
        });
    };

    save_config_to_path_with_backup(&path, config)
}

pub fn save_config_to_path(path: &Path, config: &Config) -> Result<(), SaveConfigError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SaveConfigError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let contents = toml::to_string_pretty(&config.clone().normalized())
        .map_err(|source| SaveConfigError::Serialize { source })?;

    atomic_write::write(path, &contents).map_err(|source| SaveConfigError::Write {
        path: path.to_path_buf(),
        source,
    })
}

pub fn save_config_to_path_with_backup(
    path: &Path,
    config: &Config,
) -> Result<(), SaveConfigError> {
    backup_existing_config(path).map_err(|source| SaveConfigError::Backup {
        path: path.to_path_buf(),
        source,
    })?;

    save_config_to_path(path, config)
}

fn default_folder_sources() -> Vec<PathBuf> {
    dirs::home_dir().into_iter().collect()
}

fn normalize_folder_sources(sources: Vec<PathBuf>) -> Vec<PathBuf> {
    sources
        .into_iter()
        .map(expand_home)
        .map(absolute_path)
        .collect()
}

fn default_true() -> bool {
    true
}

fn default_alternate_folder_opener_command() -> String {
    "xdg-terminal-exec".to_owned()
}

fn default_max_results() -> usize {
    36
}

fn normalize_command(command: String) -> String {
    match command.trim() {
        "" => default_alternate_folder_opener_command(),
        command => command.to_owned(),
    }
}

fn expand_home(path: PathBuf) -> PathBuf {
    let Some(path_str) = path.to_str() else {
        return path;
    };

    if path_str == "~" {
        return dirs::home_dir().unwrap_or(path);
    }

    if let Some(rest) = path_str.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }

    path
}

fn absolute_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        return path;
    }

    env::current_dir()
        .map(|current_dir| current_dir.join(&path))
        .unwrap_or(path)
}

fn backup_existing_config(path: &Path) -> io::Result<()> {
    let contents = match fs::read(path) {
        Ok(contents) => contents,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => return Err(source),
    };

    let backup_path = backup_path(path);
    let mut backup_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(backup_path)?;
    backup_file.write_all(&contents)?;
    backup_file.sync_all()
}

fn backup_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.toml");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let backup_file_name = format!("{file_name}.backup-{}-{timestamp}", process::id());

    path.with_file_name(backup_file_name)
}
