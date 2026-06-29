use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::APP_NAME;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub project_roots: Vec<PathBuf>,
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

fn default_project_roots() -> Vec<PathBuf> {
    dirs::home_dir()
        .map(|home| vec![home.join("Projects")])
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_project_roots_field() {
        let config = Config::default();

        assert!(config.project_roots.len() <= 1);
    }

    #[test]
    fn config_file_lives_under_rayslash_config_dir() {
        let Some(path) = config_file() else {
            return;
        };

        assert!(path.ends_with("rayslash/config.toml"));
    }
}
