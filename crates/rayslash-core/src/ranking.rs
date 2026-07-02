use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{atomic_write, config};

pub const RANKING_STATE_VERSION: u32 = 1;
pub const RANKING_STATE_FILE_NAME: &str = "ranking.toml";
const MAX_QUERY_PREFIX_LEN: usize = 32;
const MAX_TOTAL_BOOST: u32 = 20;
const MAX_COUNT_BOOST: u32 = 8;
const MAX_PREFIX_BOOST: u32 = 16;
const MAX_QUERY_PREFIXES_PER_ENTRY: usize = 64;
const MAX_RANKING_ENTRIES: usize = 1000;
const ENTRY_RETENTION_SECONDS: u64 = 180 * 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankingState {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub entries: BTreeMap<String, RankingEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankingEntry {
    #[serde(default)]
    pub launch_count: u32,
    #[serde(default)]
    pub last_launched_unix: u64,
    #[serde(default)]
    pub query_prefixes: BTreeMap<String, u32>,
}

#[derive(Debug)]
pub enum LoadRankingStateError {
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
pub enum SaveRankingStateError {
    CreateDir { path: PathBuf, source: io::Error },
    Serialize { source: toml::ser::Error },
    Write { path: PathBuf, source: io::Error },
}

#[derive(Debug)]
pub enum ClearRankingStateError {
    Unavailable,
    Remove { path: PathBuf, source: io::Error },
}

impl Default for RankingState {
    fn default() -> Self {
        Self {
            version: RANKING_STATE_VERSION,
            entries: BTreeMap::new(),
        }
    }
}

impl fmt::Display for LoadRankingStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(
                    f,
                    "failed to read ranking state {}: {source}",
                    path.display()
                )
            }
            Self::Parse { path, source } => {
                write!(
                    f,
                    "failed to parse ranking state {}: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for LoadRankingStateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
        }
    }
}

impl fmt::Display for SaveRankingStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateDir { path, source } => {
                write!(
                    f,
                    "failed to create ranking state directory {}: {source}",
                    path.display()
                )
            }
            Self::Serialize { source } => write!(f, "failed to serialize ranking state: {source}"),
            Self::Write { path, source } => {
                write!(
                    f,
                    "failed to write ranking state {}: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for SaveRankingStateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CreateDir { source, .. } => Some(source),
            Self::Serialize { source } => Some(source),
            Self::Write { source, .. } => Some(source),
        }
    }
}

impl fmt::Display for ClearRankingStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => write!(f, "system state directory is unavailable"),
            Self::Remove { path, source } => {
                write!(
                    f,
                    "failed to remove ranking state {}: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for ClearRankingStateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable => None,
            Self::Remove { source, .. } => Some(source),
        }
    }
}

impl RankingState {
    pub fn boost_for(&self, result_id: &str, query: &str) -> u32 {
        let Some(entry) = self.entries.get(result_id) else {
            return 0;
        };

        entry.boost_for(query)
    }

    pub fn record_launch_at(&mut self, result_id: &str, query: &str, launched_at: SystemTime) {
        let launched_at = unix_seconds(launched_at);
        let entry = self.entries.entry(result_id.to_owned()).or_default();
        entry.launch_count = entry.launch_count.saturating_add(1);
        entry.last_launched_unix = launched_at;

        for prefix in query_prefixes(query) {
            let count = entry.query_prefixes.entry(prefix).or_default();
            *count = count.saturating_add(1);
        }
    }

    pub fn record_launch(&mut self, result_id: &str, query: &str) {
        self.record_launch_at(result_id, query, SystemTime::now());
    }

    pub fn prune<I>(&mut self, active_result_ids: I, now: SystemTime)
    where
        I: IntoIterator<Item = String>,
    {
        let active_result_ids = active_result_ids.into_iter().collect::<BTreeSet<_>>();
        let cutoff = unix_seconds(now).saturating_sub(ENTRY_RETENTION_SECONDS);

        self.entries.retain(|id, entry| {
            active_result_ids.contains(id) && entry.last_launched_unix >= cutoff
        });

        for entry in self.entries.values_mut() {
            entry.prune_query_prefixes();
        }

        if self.entries.len() > MAX_RANKING_ENTRIES {
            let keep_ids = self
                .entries
                .iter()
                .map(|(id, entry)| (id.clone(), entry.last_launched_unix))
                .collect::<BTreeMap<_, _>>();
            let mut keep_ids = keep_ids.into_iter().collect::<Vec<_>>();
            keep_ids.sort_by(|(a_id, a_time), (b_id, b_time)| {
                b_time.cmp(a_time).then_with(|| a_id.cmp(b_id))
            });
            keep_ids.truncate(MAX_RANKING_ENTRIES);
            let keep_ids = keep_ids
                .into_iter()
                .map(|(id, _time)| id)
                .collect::<BTreeSet<_>>();

            self.entries.retain(|id, _entry| keep_ids.contains(id));
        }
    }
}

impl RankingEntry {
    fn boost_for(&self, query: &str) -> u32 {
        let query = normalize_query(query);
        if query.is_empty() {
            return 0;
        }

        let count_boost = self.launch_count.saturating_mul(2).min(MAX_COUNT_BOOST);
        let prefix_boost = self
            .query_prefixes
            .get(&query)
            .copied()
            .unwrap_or_default()
            .saturating_mul(6)
            .min(MAX_PREFIX_BOOST);

        count_boost
            .saturating_add(prefix_boost)
            .min(MAX_TOTAL_BOOST)
    }

    fn prune_query_prefixes(&mut self) {
        if self.query_prefixes.len() <= MAX_QUERY_PREFIXES_PER_ENTRY {
            return;
        }

        let mut prefixes = self
            .query_prefixes
            .iter()
            .map(|(prefix, count)| (prefix.clone(), *count))
            .collect::<Vec<_>>();
        prefixes.sort_by(|(a_prefix, a_count), (b_prefix, b_count)| {
            b_count.cmp(a_count).then_with(|| a_prefix.cmp(b_prefix))
        });
        prefixes.truncate(MAX_QUERY_PREFIXES_PER_ENTRY);
        self.query_prefixes = prefixes.into_iter().collect();
    }
}

pub fn ranking_state_file() -> Option<PathBuf> {
    config::state_dir().map(|path| path.join(RANKING_STATE_FILE_NAME))
}

pub fn load_ranking_state() -> Result<RankingState, LoadRankingStateError> {
    let Some(path) = ranking_state_file() else {
        return Ok(RankingState::default());
    };

    load_ranking_state_from_path(&path)
}

pub fn load_ranking_state_from_path(path: &Path) -> Result<RankingState, LoadRankingStateError> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let state: RankingState =
                toml::from_str(&contents).map_err(|source| LoadRankingStateError::Parse {
                    path: path.to_path_buf(),
                    source,
                })?;

            if state.version == RANKING_STATE_VERSION {
                Ok(state)
            } else {
                Ok(RankingState::default())
            }
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(RankingState::default()),
        Err(source) => Err(LoadRankingStateError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

pub fn load_ranking_state_from_path_or_default(path: &Path) -> RankingState {
    load_ranking_state_from_path(path).unwrap_or_default()
}

pub fn save_ranking_state(state: &RankingState) -> Result<(), SaveRankingStateError> {
    let Some(path) = ranking_state_file() else {
        return Ok(());
    };

    save_ranking_state_to_path(&path, state)
}

pub fn save_ranking_state_to_path(
    path: &Path,
    state: &RankingState,
) -> Result<(), SaveRankingStateError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SaveRankingStateError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let contents = toml::to_string_pretty(state)
        .map_err(|source| SaveRankingStateError::Serialize { source })?;

    atomic_write::write(path, &contents).map_err(|source| SaveRankingStateError::Write {
        path: path.to_path_buf(),
        source,
    })
}

pub fn clear_ranking_state() -> Result<(), ClearRankingStateError> {
    let Some(path) = ranking_state_file() else {
        return Err(ClearRankingStateError::Unavailable);
    };

    clear_ranking_state_at_path(&path)
}

pub fn clear_ranking_state_at_path(path: &Path) -> Result<(), ClearRankingStateError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(ClearRankingStateError::Remove {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn default_version() -> u32 {
    RANKING_STATE_VERSION
}

fn query_prefixes(query: &str) -> Vec<String> {
    let query = normalize_query(query);
    if query.len() < 2 {
        return Vec::new();
    }

    let mut prefixes = Vec::new();
    for len in 2..=query.len().min(MAX_QUERY_PREFIX_LEN) {
        if query.is_char_boundary(len) {
            prefixes.push(query[..len].to_owned());
        }
    }

    prefixes
}

fn normalize_query(query: &str) -> String {
    query
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn unix_seconds(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
