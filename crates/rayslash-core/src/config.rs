use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::APP_NAME;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default, alias = "project_roots")]
    pub folder_sources: Vec<PathBuf>,
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
    #[serde(default = "default_max_results")]
    pub max_results: usize,
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
            Self::Serialize { source } => Some(source),
            Self::Write { source, .. } => Some(source),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            folder_sources: default_folder_sources(),
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
            max_results: default_max_results(),
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

pub fn save_config_to_path(path: &Path, config: &Config) -> Result<(), SaveConfigError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SaveConfigError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let contents =
        toml::to_string_pretty(config).map_err(|source| SaveConfigError::Serialize { source })?;

    fs::write(path, contents).map_err(|source| SaveConfigError::Write {
        path: path.to_path_buf(),
        source,
    })
}

fn default_folder_sources() -> Vec<PathBuf> {
    dirs::home_dir().into_iter().collect()
}

#[cfg(test)]
fn default_folder_sources_for_home(home: &Path) -> Vec<PathBuf> {
    vec![home.to_path_buf()]
}

fn normalize_folder_sources(sources: Vec<PathBuf>) -> Vec<PathBuf> {
    sources.into_iter().map(expand_home).collect()
}

fn default_true() -> bool {
    true
}

fn default_alternate_folder_opener_command() -> String {
    "code".to_owned()
}

fn default_max_results() -> usize {
    50
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_public_defaults() {
        let config = Config::default();

        assert!(config.folder_sources.len() <= 1);
        assert_eq!(config.providers, ProviderConfig::default());
        assert_eq!(config.actions, ActionConfig::default());
        assert_eq!(config.appearance, AppearanceConfig::default());
    }

    #[test]
    fn config_file_lives_under_rayslash_config_dir() {
        let Some(path) = config_file() else {
            return;
        };

        assert!(path.ends_with("rayslash/config.toml"));
    }

    #[test]
    fn missing_config_loads_defaults() {
        let path = std::env::temp_dir().join(format!(
            "rayslash-missing-config-{}.toml",
            std::process::id()
        ));

        let config = load_config_from_path(&path).expect("missing config should use defaults");

        assert_eq!(config, Config::default());
    }

    #[test]
    fn config_loads_folder_sources_from_toml() {
        let dir = unique_temp_dir("rayslash-config-test");
        let path = dir.join("config.toml");
        fs::write(
            &path,
            r#"
folder_sources = ["/tmp/alpha", "/tmp/beta"]
"#,
        )
        .expect("write config");

        let config = load_config_from_path(&path).expect("load config");

        assert_eq!(
            config.folder_sources,
            vec![PathBuf::from("/tmp/alpha"), PathBuf::from("/tmp/beta")]
        );

        fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

    #[test]
    fn legacy_project_roots_still_load_as_folder_sources() {
        let dir = unique_temp_dir("rayslash-config-legacy-roots-test");
        let path = dir.join("config.toml");
        fs::write(
            &path,
            r#"
project_roots = ["/tmp/alpha"]
"#,
        )
        .expect("write config");

        let config = load_config_from_path(&path).expect("load config");

        assert_eq!(config.folder_sources, vec![PathBuf::from("/tmp/alpha")]);

        fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

    #[test]
    fn config_loads_provider_action_and_appearance_settings_from_toml() {
        let dir = unique_temp_dir("rayslash-config-settings-test");
        let path = dir.join("config.toml");
        fs::write(
            &path,
            r#"
folder_sources = ["/tmp/alpha"]

[providers]
apps = false
folders = true
calculator = false

[actions]
alternate_folder_opener_enabled = false
alternate_folder_opener_command = "codium"

[appearance]
max_results = 20

[ranking]
learn_from_usage = false
"#,
        )
        .expect("write config");

        let config = load_config_from_path(&path).expect("load config");

        assert_eq!(config.folder_sources, vec![PathBuf::from("/tmp/alpha")]);
        assert_eq!(
            config.providers,
            ProviderConfig {
                apps: false,
                folders: true,
                calculator: false,
            }
        );
        assert!(!config.actions.alternate_folder_opener_enabled);
        assert_eq!(config.actions.alternate_folder_opener_command, "codium");
        assert_eq!(config.appearance, AppearanceConfig { max_results: 20 });
        assert_eq!(
            config.ranking,
            RankingConfig {
                learn_from_usage: false,
            }
        );

        fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

    #[test]
    fn legacy_provider_and_action_fields_still_load() {
        let dir = unique_temp_dir("rayslash-config-legacy-settings-test");
        let path = dir.join("config.toml");
        fs::write(
            &path,
            r#"
[providers]
projects = false

[actions]
project_editor_command = "codium"
"#,
        )
        .expect("write config");

        let config = load_config_from_path(&path).expect("load config");

        assert!(!config.providers.folders);
        assert_eq!(config.actions.alternate_folder_opener_command, "codium");

        fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

    #[test]
    fn missing_nested_config_fields_use_public_defaults() {
        let dir = unique_temp_dir("rayslash-config-default-settings-test");
        let path = dir.join("config.toml");
        fs::write(
            &path,
            r#"
[providers]
apps = false

[actions]
alternate_folder_opener_command = ""

[appearance]
max_results = 0
"#,
        )
        .expect("write config");

        let config = load_config_from_path(&path).expect("load config");

        assert_eq!(
            config.providers,
            ProviderConfig {
                apps: false,
                folders: true,
                calculator: true,
            }
        );
        assert_eq!(config.actions.alternate_folder_opener_command, "code");
        assert!(config.actions.alternate_folder_opener_enabled);
        assert_eq!(config.appearance.max_results, 50);
        assert!(config.ranking.learn_from_usage);

        fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

    #[test]
    fn default_folder_sources_use_home() {
        let home = unique_temp_dir("rayslash-home-test");

        let sources = default_folder_sources_for_home(&home);

        assert_eq!(sources, vec![home.clone()]);

        fs::remove_dir_all(home).expect("cleanup temp dir");
    }

    #[test]
    fn config_expands_tilde_folder_sources() {
        let dir = unique_temp_dir("rayslash-config-tilde-test");
        let path = dir.join("config.toml");

        fs::write(
            &path,
            r#"
folder_sources = ["~/Documents"]
"#,
        )
        .expect("write config");

        let config = load_config_from_path(&path).expect("load config");
        let home = dirs::home_dir().expect("home dir");

        assert_eq!(config.folder_sources, vec![home.join("Documents")]);

        fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

    #[test]
    fn config_can_be_saved_and_loaded_from_toml() {
        let dir = unique_temp_dir("rayslash-config-save-test");
        let path = dir.join("nested/config.toml");
        let config = Config {
            folder_sources: vec![PathBuf::from("~/Documents")],
            providers: ProviderConfig {
                apps: true,
                folders: false,
                calculator: true,
            },
            actions: ActionConfig {
                alternate_folder_opener_enabled: false,
                alternate_folder_opener_command: "codium".to_owned(),
            },
            appearance: AppearanceConfig { max_results: 25 },
            ranking: RankingConfig {
                learn_from_usage: false,
            },
        };

        save_config_to_path(&path, &config).expect("save config");
        let saved = fs::read_to_string(&path).expect("read saved config");
        let loaded = load_config_from_path(&path).expect("load saved config");
        let home = dirs::home_dir().expect("home dir");

        assert!(saved.contains("folder_sources"));
        assert!(saved.contains("folders = false"));
        assert!(saved.contains("alternate_folder_opener_enabled = false"));
        assert!(saved.contains("alternate_folder_opener_command = \"codium\""));
        assert!(saved.contains("learn_from_usage = false"));
        assert_eq!(loaded.folder_sources, vec![home.join("Documents")]);
        assert_eq!(loaded.providers, config.providers);
        assert_eq!(loaded.actions, config.actions);
        assert_eq!(loaded.appearance, config.appearance);
        assert_eq!(loaded.ranking, config.ranking);

        fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "{prefix}-{}-{:?}-{}",
            std::process::id(),
            std::thread::current().id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock before unix epoch")
                .as_nanos()
        ));
        fs::create_dir(&path).expect("create temp dir");
        path
    }
}
