use std::{
    collections::{BTreeMap, HashMap, HashSet},
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde::{Deserialize, Serialize};

use super::{
    DesktopApp, DesktopCandidateOutcome,
    desktop_entry::parse_desktop_file_with_id,
    icon_lookup::{DesktopIconResolver, desktop_icon_dirs},
};

const DESKTOP_CACHE_VERSION: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationSource {
    UserXdg,
    SystemXdg,
    UserFlatpak,
    SystemFlatpak,
    Snap,
    HostXdg,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SourceScanStatistics {
    pub candidates: u64,
    pub source_errors: u64,
    pub outcomes: BTreeMap<DesktopCandidateOutcome, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ApplicationScanStatistics {
    pub candidates: u64,
    pub source_errors: u64,
    pub outcomes: BTreeMap<DesktopCandidateOutcome, u64>,
    pub sources: BTreeMap<ApplicationSource, SourceScanStatistics>,
}

impl ApplicationScanStatistics {
    pub fn outcome_count(&self, outcome: DesktopCandidateOutcome) -> u64 {
        self.outcomes.get(&outcome).copied().unwrap_or(0)
    }

    pub fn indexed(&self) -> u64 {
        self.outcome_count(DesktopCandidateOutcome::Indexed)
    }

    pub fn accounted_candidates(&self) -> u64 {
        self.outcomes.values().sum()
    }

    pub fn is_consistent(&self) -> bool {
        self.candidates == self.accounted_candidates()
            && self
                .sources
                .values()
                .map(|source| source.candidates)
                .sum::<u64>()
                == self.candidates
            && self
                .sources
                .values()
                .map(|source| source.source_errors)
                .sum::<u64>()
                == self.source_errors
            && self
                .sources
                .values()
                .all(|source| source.candidates == source.outcomes.values().sum::<u64>())
    }

    pub fn successfully_read(&self) -> u64 {
        self.candidates.saturating_sub(
            self.outcome_count(DesktopCandidateOutcome::Duplicate)
                + self.outcome_count(DesktopCandidateOutcome::ReadFailure)
                + self.outcome_count(DesktopCandidateOutcome::InvalidEncoding)
                + self.outcome_count(DesktopCandidateOutcome::MetadataFailure),
        )
    }

    pub fn successfully_parsed(&self) -> u64 {
        self.successfully_read()
            .saturating_sub(self.outcome_count(DesktopCandidateOutcome::MalformedDesktopEntry))
    }

    pub fn local_summary(&self) -> String {
        let mut parts = vec![
            format!("{} candidates", self.candidates),
            format!("{} indexed", self.indexed()),
        ];
        for (outcome, count) in &self.outcomes {
            if *outcome != DesktopCandidateOutcome::Indexed && *count > 0 {
                parts.push(format!("{count} {}", outcome_label(*outcome)));
            }
        }
        if self.source_errors > 0 {
            parts.push(format!("{} source errors", self.source_errors));
        }
        parts.join(" · ")
    }

    fn record_candidate(&mut self, source: ApplicationSource, outcome: DesktopCandidateOutcome) {
        self.candidates += 1;
        *self.outcomes.entry(outcome).or_default() += 1;
        let source = self.sources.entry(source).or_default();
        source.candidates += 1;
        *source.outcomes.entry(outcome).or_default() += 1;
    }

    fn record_source_errors(&mut self, source: ApplicationSource, count: u64) {
        if count == 0 {
            return;
        }
        self.source_errors += count;
        self.sources.entry(source).or_default().source_errors += count;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationScan {
    pub apps: Vec<DesktopApp>,
    pub statistics: ApplicationScanStatistics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DesktopSourceStamp {
    path: PathBuf,
    len: u64,
    modified_seconds: u64,
    modified_nanoseconds: u32,
}

#[derive(Serialize, Deserialize)]
struct DesktopCatalogEntry {
    source: DesktopSourceStamp,
    id: String,
    #[serde(default)]
    outcome: Option<DesktopCandidateOutcome>,
    app: Option<DesktopApp>,
}

#[derive(Serialize, Deserialize)]
struct DesktopCatalogCache {
    version: u32,
    environment: String,
    #[serde(default)]
    sources: Vec<DesktopSourceStamp>,
    #[serde(default)]
    entries: Vec<DesktopCatalogEntry>,
    #[serde(default)]
    apps: Vec<DesktopApp>,
    #[serde(default)]
    statistics: Option<ApplicationScanStatistics>,
}

#[derive(Serialize, Deserialize)]
struct DesktopSourceCache {
    version: u32,
    environment: String,
    sources: Vec<DesktopSourceStamp>,
}

pub fn discover_desktop_apps() -> Vec<DesktopApp> {
    discover_desktop_apps_in_dirs(&desktop_application_dirs())
}

pub fn discover_and_cache_desktop_apps() -> Vec<DesktopApp> {
    discover_and_cache_desktop_apps_with_diagnostics().apps
}

pub fn discover_and_cache_desktop_apps_with_diagnostics() -> ApplicationScan {
    let dirs = desktop_application_dirs();
    let environment = desktop_environment();
    let previous = desktop_apps_cache_file()
        .and_then(|path| fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<DesktopCatalogCache>(&bytes).ok())
        .filter(|cache| cache.version == DESKTOP_CACHE_VERSION && cache.environment == environment);
    let (scan, entries) = reconcile_desktop_apps(&dirs, previous);
    let sources = desktop_source_stamps(&dirs);
    if let Err(error) =
        save_desktop_apps_cache(entries, sources, &environment, scan.statistics.clone())
    {
        eprintln!("failed to save the desktop application cache: {error}");
    }
    scan
}

pub fn load_cached_desktop_apps() -> Option<Vec<DesktopApp>> {
    cached_desktop_apps_from_bytes(
        &fs::read(desktop_apps_cache_file()?).ok()?,
        &desktop_environment(),
    )
}

pub fn load_cached_desktop_scan() -> Option<ApplicationScan> {
    let contents = fs::read(desktop_apps_cache_file()?).ok()?;
    let cache: DesktopCatalogCache = serde_json::from_slice(&contents).ok()?;
    if cache.version != DESKTOP_CACHE_VERSION || cache.environment != desktop_environment() {
        return None;
    }
    let statistics = cache.statistics?;
    if !statistics.is_consistent() {
        return None;
    }
    let mut apps = cache
        .entries
        .into_iter()
        .filter_map(|entry| entry.app)
        .collect::<Vec<_>>();
    apps.sort_by(app_order);
    if apps.len() as u64 != statistics.indexed() {
        return None;
    }
    Some(ApplicationScan { apps, statistics })
}

/// Check whether the source desktop files still match the cached catalog.
///
/// This deliberately compares cheap file metadata and avoids parsing desktop entries or
/// resolving icons. Callers can run it off the UI thread before scheduling reconciliation.
pub fn desktop_apps_cache_is_current() -> bool {
    let Some(path) = desktop_sources_cache_file() else {
        return false;
    };
    let Ok(contents) = fs::read(path) else {
        return false;
    };
    let Ok(cache) = serde_json::from_slice::<DesktopSourceCache>(&contents) else {
        return false;
    };
    cache.version == DESKTOP_CACHE_VERSION
        && cache.environment == desktop_environment()
        && cache.sources == desktop_source_stamps(&desktop_application_dirs())
}

pub fn discover_desktop_apps_in_dirs(dirs: &[PathBuf]) -> Vec<DesktopApp> {
    discover_desktop_apps_in_dirs_with_diagnostics(dirs).apps
}

pub fn discover_desktop_apps_in_dirs_with_diagnostics(dirs: &[PathBuf]) -> ApplicationScan {
    reconcile_desktop_apps(dirs, None).0
}

pub fn desktop_application_dirs() -> Vec<PathBuf> {
    desktop_application_dirs_from_env(
        std::env::var_os("XDG_DATA_HOME"),
        std::env::var_os("XDG_DATA_DIRS"),
        dirs::home_dir(),
        std::env::var_os("FLATPAK_ID").is_some(),
    )
}

fn desktop_apps_cache_file() -> Option<PathBuf> {
    dirs::cache_dir().map(|path| path.join("rayslash/desktop-apps-v1.json"))
}

fn desktop_sources_cache_file() -> Option<PathBuf> {
    dirs::cache_dir().map(|path| path.join("rayslash/desktop-apps-v1.sources.json"))
}

fn save_desktop_apps_cache(
    entries: Vec<DesktopCatalogEntry>,
    sources: Vec<DesktopSourceStamp>,
    environment: &str,
    statistics: ApplicationScanStatistics,
) -> io::Result<()> {
    let Some(path) = desktop_apps_cache_file() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let cache = DesktopCatalogCache {
        version: DESKTOP_CACHE_VERSION,
        environment: environment.to_owned(),
        sources: Vec::new(),
        entries,
        apps: Vec::new(),
        statistics: Some(statistics),
    };
    let source_cache = DesktopSourceCache {
        version: DESKTOP_CACHE_VERSION,
        environment: environment.to_owned(),
        sources,
    };
    let contents = serde_json::to_vec(&cache).map_err(io::Error::other)?;
    crate::atomic_write::write_bytes(&path, &contents)?;
    if let Some(source_path) = desktop_sources_cache_file() {
        let sources = serde_json::to_vec(&source_cache).map_err(io::Error::other)?;
        crate::atomic_write::write_bytes(&source_path, &sources)?;
    }
    Ok(())
}

fn cached_desktop_apps_from_bytes(contents: &[u8], environment: &str) -> Option<Vec<DesktopApp>> {
    let cache: DesktopCatalogCache = serde_json::from_slice(contents).ok()?;
    if cache.environment != environment {
        return None;
    }
    match cache.version {
        1 | 2 => Some(cache.apps),
        3 | DESKTOP_CACHE_VERSION => {
            let mut apps = cache
                .entries
                .into_iter()
                .filter_map(|entry| entry.app)
                .collect::<Vec<_>>();
            apps.sort_by(app_order);
            Some(apps)
        }
        _ => None,
    }
}

fn reconcile_desktop_apps(
    dirs: &[PathBuf],
    previous: Option<DesktopCatalogCache>,
) -> (ApplicationScan, Vec<DesktopCatalogEntry>) {
    let previous = previous
        .into_iter()
        .flat_map(|cache| cache.entries)
        .filter(|entry| entry.outcome.is_some())
        .map(|entry| (entry.source.path.clone(), entry))
        .collect::<HashMap<_, _>>();
    let mut seen_ids = HashSet::new();
    let mut icon_resolver = DesktopIconResolver::new(desktop_icon_dirs());
    let mut entries = Vec::new();
    let mut apps = Vec::new();
    let mut statistics = ApplicationScanStatistics::default();

    for dir in dirs {
        let source_kind = classify_application_source(dir);
        let (paths, source_errors) = desktop_files_in_dir(dir);
        statistics.record_source_errors(source_kind, source_errors);
        for path in paths {
            let id = desktop_app_id(dir, &path);
            if !seen_ids.insert(id.clone()) {
                statistics.record_candidate(source_kind, DesktopCandidateOutcome::Duplicate);
                continue;
            }
            let Some(source) = desktop_source_stamp(path.clone()) else {
                statistics.record_candidate(source_kind, DesktopCandidateOutcome::MetadataFailure);
                continue;
            };

            let (outcome, app) = if let Some(entry) = previous
                .get(&path)
                .filter(|entry| entry.source == source && entry.id == id)
            {
                (
                    entry
                        .outcome
                        .unwrap_or(DesktopCandidateOutcome::ReadFailure),
                    entry.app.clone(),
                )
            } else {
                match parse_desktop_file_with_id(&path, id.clone()) {
                    Ok(Ok(mut app)) => {
                        app.icon_path = app
                            .icon
                            .as_deref()
                            .and_then(|icon| icon_resolver.resolve(icon));
                        (DesktopCandidateOutcome::Indexed, Some(app))
                    }
                    Ok(Err(outcome)) => (outcome, None),
                    Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                        (DesktopCandidateOutcome::InvalidEncoding, None)
                    }
                    Err(_) => (DesktopCandidateOutcome::ReadFailure, None),
                }
            };

            statistics.record_candidate(source_kind, outcome);
            if let Some(app) = app.as_ref() {
                apps.push(app.clone());
            }
            entries.push(DesktopCatalogEntry {
                source,
                id,
                outcome: Some(outcome),
                app,
            });
        }
    }

    apps.sort_by(app_order);
    entries.sort_by(|a, b| a.source.path.cmp(&b.source.path));
    debug_assert!(statistics.is_consistent());
    debug_assert_eq!(apps.len() as u64, statistics.indexed());
    (ApplicationScan { apps, statistics }, entries)
}

fn desktop_source_stamps(dirs: &[PathBuf]) -> Vec<DesktopSourceStamp> {
    let mut stamps = dirs
        .iter()
        .flat_map(|directory| desktop_files_in_dir(directory).0)
        .filter_map(desktop_source_stamp)
        .collect::<Vec<_>>();
    stamps.sort_by(|a, b| a.path.cmp(&b.path));
    stamps
}

fn desktop_source_stamp(path: PathBuf) -> Option<DesktopSourceStamp> {
    let metadata = fs::metadata(&path).ok()?;
    let modified = metadata.modified().ok()?;
    let duration = modified.duration_since(SystemTime::UNIX_EPOCH).ok()?;
    Some(DesktopSourceStamp {
        path,
        len: metadata.len(),
        modified_seconds: duration.as_secs(),
        modified_nanoseconds: duration.subsec_nanos(),
    })
}

fn desktop_environment() -> String {
    [
        "XDG_DATA_HOME",
        "XDG_DATA_DIRS",
        "PATH",
        "XDG_CURRENT_DESKTOP",
        "DESKTOP_SESSION",
        "LANG",
        "LC_ALL",
        "LC_MESSAGES",
        "FLATPAK_ID",
    ]
    .into_iter()
    .map(|name| {
        format!(
            "{name}={}",
            std::env::var_os(name)
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_default()
        )
    })
    .collect::<Vec<_>>()
    .join("\n")
}

fn desktop_application_dirs_from_env(
    data_home: Option<OsString>,
    data_dirs: Option<OsString>,
    home: Option<PathBuf>,
    flatpak: bool,
) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut seen_paths = HashSet::new();

    if let Some(data_home) = data_home.filter(|value| !value.is_empty()) {
        push_unique_path(
            &mut dirs,
            &mut seen_paths,
            PathBuf::from(data_home).join("applications"),
        );
    } else if let Some(home) = home {
        push_unique_path(
            &mut dirs,
            &mut seen_paths,
            home.join(".local/share/applications"),
        );
    }

    let data_dirs = data_dirs
        .filter(|value| !value.is_empty())
        .map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .unwrap_or_else(|| {
            vec![
                PathBuf::from("/usr/local/share"),
                PathBuf::from("/usr/share"),
            ]
        });

    for data_dir in data_dirs {
        push_unique_path(&mut dirs, &mut seen_paths, data_dir.join("applications"));
    }

    if flatpak {
        for path in [
            "/run/host/usr/local/share/applications",
            "/run/host/usr/share/applications",
        ] {
            push_unique_path(&mut dirs, &mut seen_paths, PathBuf::from(path));
        }
    }

    dirs
}

fn push_unique_path(paths: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: PathBuf) {
    if seen.insert(path.clone()) {
        paths.push(path);
    }
}

fn desktop_files_in_dir(dir: &Path) -> (Vec<PathBuf>, u64) {
    let mut files = Vec::new();
    let mut source_errors = 0;
    collect_desktop_files(dir, &mut files, &mut source_errors);
    files.sort();
    (files, source_errors)
}

fn collect_desktop_files(dir: &Path, files: &mut Vec<PathBuf>, source_errors: &mut u64) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) => {
            if error.kind() != io::ErrorKind::NotFound {
                *source_errors += 1;
            }
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                *source_errors += 1;
                continue;
            }
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => {
                *source_errors += 1;
                continue;
            }
        };

        if file_type.is_dir() {
            collect_desktop_files(&path, files, source_errors);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "desktop")
        {
            files.push(path);
        }
    }
}

fn classify_application_source(dir: &Path) -> ApplicationSource {
    let path = dir.to_string_lossy().to_ascii_lowercase();
    if path.contains("/.local/share/flatpak/exports/share/applications") {
        ApplicationSource::UserFlatpak
    } else if path.contains("/var/lib/flatpak/exports/share/applications") {
        ApplicationSource::SystemFlatpak
    } else if path.contains("/snap/") || path.contains("/var/lib/snapd/desktop/applications") {
        ApplicationSource::Snap
    } else if path.starts_with("/run/host/") {
        ApplicationSource::HostXdg
    } else if dirs::home_dir().is_some_and(|home| dir.starts_with(home))
        || path.ends_with("/.local/share/applications")
    {
        ApplicationSource::UserXdg
    } else if path == "/usr/share/applications"
        || path == "/usr/local/share/applications"
        || path == "/app/share/applications"
    {
        ApplicationSource::SystemXdg
    } else {
        ApplicationSource::Other
    }
}

fn desktop_app_id(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "-")
}

fn app_order(a: &DesktopApp, b: &DesktopApp) -> std::cmp::Ordering {
    a.name
        .to_lowercase()
        .cmp(&b.name.to_lowercase())
        .then_with(|| a.id.cmp(&b.id))
        .then_with(|| a.desktop_file.cmp(&b.desktop_file))
}

fn outcome_label(outcome: DesktopCandidateOutcome) -> &'static str {
    match outcome {
        DesktopCandidateOutcome::Indexed => "indexed",
        DesktopCandidateOutcome::Duplicate => "duplicates",
        DesktopCandidateOutcome::Hidden => "hidden",
        DesktopCandidateOutcome::NoDisplay => "NoDisplay",
        DesktopCandidateOutcome::UnsupportedType => "unsupported type",
        DesktopCandidateOutcome::MissingName => "missing name",
        DesktopCandidateOutcome::MissingExec => "missing Exec",
        DesktopCandidateOutcome::MalformedDesktopEntry => "malformed",
        DesktopCandidateOutcome::DesktopEnvironmentFiltered => "desktop filtered",
        DesktopCandidateOutcome::InvalidTryExec => "invalid TryExec",
        DesktopCandidateOutcome::MissingExecutable => "missing executable",
        DesktopCandidateOutcome::InvalidEncoding => "invalid encoding",
        DesktopCandidateOutcome::ReadFailure => "read failures",
        DesktopCandidateOutcome::MetadataFailure => "metadata failures",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache_bytes(version: u32, environment: &str) -> Vec<u8> {
        serde_json::to_vec(&DesktopCatalogCache {
            version,
            environment: environment.to_owned(),
            sources: Vec::new(),
            entries: Vec::new(),
            apps: Vec::new(),
            statistics: Some(ApplicationScanStatistics::default()),
        })
        .expect("cache fixture should serialize")
    }

    #[test]
    fn desktop_application_dirs_follow_xdg_base_directories() {
        assert_eq!(
            desktop_application_dirs_from_env(
                Some(OsString::from("/tmp/data-home")),
                Some(OsString::from("/tmp/flatpak:/tmp/system")),
                Some(PathBuf::from("/home/example")),
                false,
            ),
            vec![
                PathBuf::from("/tmp/data-home/applications"),
                PathBuf::from("/tmp/flatpak/applications"),
                PathBuf::from("/tmp/system/applications"),
            ]
        );
    }

    #[test]
    fn desktop_application_dirs_use_default_xdg_locations() {
        assert_eq!(
            desktop_application_dirs_from_env(
                None,
                None,
                Some(PathBuf::from("/home/example")),
                false,
            ),
            vec![
                PathBuf::from("/home/example/.local/share/applications"),
                PathBuf::from("/usr/local/share/applications"),
                PathBuf::from("/usr/share/applications"),
            ]
        );
    }

    #[test]
    fn flatpak_discovery_includes_host_desktop_entry_exports() {
        let dirs = desktop_application_dirs_from_env(
            Some(OsString::from("/tmp/app-data")),
            Some(OsString::from("/app/share:/usr/share")),
            Some(PathBuf::from("/home/example")),
            true,
        );

        assert!(dirs.contains(&PathBuf::from("/run/host/usr/share/applications")));
    }

    #[test]
    fn desktop_cache_requires_matching_version_and_environment() {
        assert_eq!(
            cached_desktop_apps_from_bytes(
                &cache_bytes(DESKTOP_CACHE_VERSION, "current"),
                "current"
            ),
            Some(Vec::new())
        );
        assert!(
            cached_desktop_apps_from_bytes(
                &cache_bytes(DESKTOP_CACHE_VERSION + 1, "current"),
                "current"
            )
            .is_none()
        );
        assert!(
            cached_desktop_apps_from_bytes(
                &cache_bytes(DESKTOP_CACHE_VERSION, "different"),
                "current"
            )
            .is_none()
        );
        assert!(cached_desktop_apps_from_bytes(b"not json", "current").is_none());
    }

    #[test]
    fn desktop_cache_version_one_remains_a_fast_migration_source() {
        assert_eq!(
            cached_desktop_apps_from_bytes(
                br#"{"version":1,"environment":"current","apps":[]}"#,
                "current"
            ),
            Some(Vec::new())
        );
    }

    #[test]
    fn accounting_requires_one_final_outcome_per_candidate() {
        let mut statistics = ApplicationScanStatistics::default();
        statistics.record_candidate(ApplicationSource::UserXdg, DesktopCandidateOutcome::Indexed);
        statistics.record_candidate(ApplicationSource::UserXdg, DesktopCandidateOutcome::Hidden);
        statistics.record_candidate(
            ApplicationSource::SystemFlatpak,
            DesktopCandidateOutcome::ReadFailure,
        );
        assert!(statistics.is_consistent());
        assert_eq!(statistics.candidates, 3);
        assert_eq!(statistics.accounted_candidates(), 3);

        statistics.candidates += 1;
        assert!(!statistics.is_consistent());
    }

    #[test]
    fn application_sources_are_classified_without_retaining_path_details() {
        assert_eq!(
            classify_application_source(Path::new(
                "/home/example/.local/share/flatpak/exports/share/applications"
            )),
            ApplicationSource::UserFlatpak
        );
        assert_eq!(
            classify_application_source(Path::new("/var/lib/flatpak/exports/share/applications")),
            ApplicationSource::SystemFlatpak
        );
        assert_eq!(
            classify_application_source(Path::new("/var/lib/snapd/desktop/applications")),
            ApplicationSource::Snap
        );
        assert_eq!(
            classify_application_source(Path::new("/run/host/usr/share/applications")),
            ApplicationSource::HostXdg
        );
        assert_eq!(
            classify_application_source(Path::new("/usr/share/applications")),
            ApplicationSource::SystemXdg
        );
        assert_eq!(
            classify_application_source(Path::new("/opt/vendor/applications")),
            ApplicationSource::Other
        );
    }
}
