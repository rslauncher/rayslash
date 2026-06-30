use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::APP_NAME;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub project_roots: Vec<PathBuf>,
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

impl Default for Config {
    fn default() -> Self {
        Self {
            project_roots: default_project_roots(),
        }
    }
}

pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|path| path.join(APP_NAME))
}

pub fn config_file() -> Option<PathBuf> {
    config_dir().map(|path| path.join("config.toml"))
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
            let mut config: Config =
                toml::from_str(&contents).map_err(|source| ConfigError::Parse {
                    path: path.to_path_buf(),
                    source,
                })?;

            config.project_roots = normalize_project_roots(config.project_roots);

            Ok(config)
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(Config::default()),
        Err(source) => Err(ConfigError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn default_project_roots() -> Vec<PathBuf> {
    dirs::home_dir()
        .map(|home| default_project_roots_for_home(&home))
        .unwrap_or_default()
}

fn default_project_roots_for_home(home: &Path) -> Vec<PathBuf> {
    ["Projects", "Code", "Documents/Projects"]
        .into_iter()
        .map(|relative| home.join(relative))
        .filter(|path| path.is_dir())
        .collect()
}

fn normalize_project_roots(roots: Vec<PathBuf>) -> Vec<PathBuf> {
    roots.into_iter().map(expand_home).collect()
}

fn expand_home(path: PathBuf) -> PathBuf {
    let Some(path_str) = path.to_str() else {
        return path;
    };

    if path_str == "~" {
        return dirs::home_dir().unwrap_or(path);
    }

    if let Some(rest) = path_str.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }

    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_project_roots_field() {
        let config = Config::default();

        assert!(config.project_roots.len() <= 3);
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
    fn config_loads_project_roots_from_toml() {
        let dir = unique_temp_dir("rayslash-config-test");
        let path = dir.join("config.toml");
        fs::write(
            &path,
            r#"
project_roots = ["/tmp/alpha", "/tmp/beta"]
"#,
        )
        .expect("write config");

        let config = load_config_from_path(&path).expect("load config");

        assert_eq!(
            config.project_roots,
            vec![PathBuf::from("/tmp/alpha"), PathBuf::from("/tmp/beta")]
        );

        fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

    #[test]
    fn default_project_roots_only_include_existing_common_dirs() {
        let home = unique_temp_dir("rayslash-home-test");
        fs::create_dir(home.join("Code")).expect("create Code");
        fs::create_dir_all(home.join("Documents/Projects")).expect("create Documents/Projects");

        let roots = default_project_roots_for_home(&home);

        assert_eq!(
            roots,
            vec![home.join("Code"), home.join("Documents/Projects")]
        );

        fs::remove_dir_all(home).expect("cleanup temp dir");
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

    #[test]
    fn config_expands_tilde_project_roots() {
        let dir = unique_temp_dir("rayslash-config-tilde-test");
        let path = dir.join("config.toml");

        fs::write(
            &path,
            r#"
project_roots = ["~/Documents/Projects"]
"#,
        )
        .expect("write config");

        let config = load_config_from_path(&path).expect("load config");
        let home = dirs::home_dir().expect("home dir");

        assert_eq!(config.project_roots, vec![home.join("Documents/Projects")]);

        fs::remove_dir_all(dir).expect("cleanup temp dir");
    }
}
