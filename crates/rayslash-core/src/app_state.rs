use std::{
    collections::BTreeSet,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{atomic_write, config};

pub const APP_STATE_VERSION: u32 = 1;
pub const APP_STATE_FILE_NAME: &str = "apps.toml";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppInstallState {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub initialized: bool,
    #[serde(default)]
    pub known_app_ids: BTreeSet<String>,
    #[serde(default)]
    pub new_app_ids: BTreeSet<String>,
}

#[derive(Debug)]
pub enum LoadAppStateError {
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
pub enum SaveAppStateError {
    CreateDir { path: PathBuf, source: io::Error },
    Serialize { source: toml::ser::Error },
    Write { path: PathBuf, source: io::Error },
}

impl Default for AppInstallState {
    fn default() -> Self {
        Self {
            version: APP_STATE_VERSION,
            initialized: false,
            known_app_ids: BTreeSet::new(),
            new_app_ids: BTreeSet::new(),
        }
    }
}

impl fmt::Display for LoadAppStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(f, "failed to read app state {}: {source}", path.display())
            }
            Self::Parse { path, source } => {
                write!(f, "failed to parse app state {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for LoadAppStateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
        }
    }
}

impl fmt::Display for SaveAppStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateDir { path, source } => {
                write!(
                    f,
                    "failed to create app state directory {}: {source}",
                    path.display()
                )
            }
            Self::Serialize { source } => write!(f, "failed to serialize app state: {source}"),
            Self::Write { path, source } => {
                write!(f, "failed to write app state {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for SaveAppStateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CreateDir { source, .. } => Some(source),
            Self::Serialize { source } => Some(source),
            Self::Write { source, .. } => Some(source),
        }
    }
}

impl AppInstallState {
    pub fn mark_discovered_app_ids<I>(&mut self, app_ids: I) -> bool
    where
        I: IntoIterator<Item = String>,
    {
        let app_ids = app_ids.into_iter().collect::<BTreeSet<_>>();
        let mut changed = false;

        if !self.initialized {
            self.known_app_ids = app_ids;
            self.new_app_ids.clear();
            self.initialized = true;
            return true;
        }

        for app_id in &app_ids {
            if self.known_app_ids.insert(app_id.clone()) {
                self.new_app_ids.insert(app_id.clone());
                changed = true;
            }
        }

        let before = self.new_app_ids.len();
        self.new_app_ids.retain(|app_id| app_ids.contains(app_id));
        changed || self.new_app_ids.len() != before
    }

    pub fn mark_app_selected(&mut self, app_id: &str) -> bool {
        let known_changed = self.known_app_ids.insert(app_id.to_owned());
        let new_changed = self.new_app_ids.remove(app_id);
        known_changed || new_changed
    }

    pub fn is_new_app(&self, app_id: &str) -> bool {
        self.new_app_ids.contains(app_id)
    }
}

pub fn app_state_file() -> Option<PathBuf> {
    config::state_dir().map(|path| path.join(APP_STATE_FILE_NAME))
}

pub fn load_app_state() -> Result<AppInstallState, LoadAppStateError> {
    let Some(path) = app_state_file() else {
        return Ok(AppInstallState::default());
    };

    load_app_state_from_path(&path)
}

pub fn load_app_state_from_path(path: &Path) -> Result<AppInstallState, LoadAppStateError> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let state: AppInstallState =
                toml::from_str(&contents).map_err(|source| LoadAppStateError::Parse {
                    path: path.to_path_buf(),
                    source,
                })?;

            if state.version == APP_STATE_VERSION {
                Ok(state)
            } else {
                Ok(AppInstallState::default())
            }
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(AppInstallState::default()),
        Err(source) => Err(LoadAppStateError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

pub fn save_app_state(state: &AppInstallState) -> Result<(), SaveAppStateError> {
    let Some(path) = app_state_file() else {
        return Ok(());
    };

    save_app_state_to_path(&path, state)
}

pub fn save_app_state_to_path(
    path: &Path,
    state: &AppInstallState,
) -> Result<(), SaveAppStateError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SaveAppStateError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let contents =
        toml::to_string_pretty(state).map_err(|source| SaveAppStateError::Serialize { source })?;

    atomic_write::write(path, &contents).map_err(|source| SaveAppStateError::Write {
        path: path.to_path_buf(),
        source,
    })
}

fn default_version() -> u32 {
    APP_STATE_VERSION
}
