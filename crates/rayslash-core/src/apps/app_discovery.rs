use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde::{Deserialize, Serialize};

use super::{
    DesktopApp,
    desktop_entry::parse_desktop_file_with_id,
    icon_lookup::{DesktopIconResolver, desktop_icon_dirs},
};

const DESKTOP_CACHE_VERSION: u32 = 3;

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
    apps: Vec<DesktopApp>,
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
    let dirs = desktop_application_dirs();
    let environment = desktop_environment();
    let previous = desktop_apps_cache_file()
        .and_then(|path| fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<DesktopCatalogCache>(&bytes).ok())
        .filter(|cache| cache.version == DESKTOP_CACHE_VERSION && cache.environment == environment);
    let (apps, entries) = reconcile_desktop_apps(&dirs, previous);
    let sources = desktop_source_stamps(&dirs);
    let _ = save_desktop_apps_cache(entries, sources, &environment);
    apps
}

pub fn load_cached_desktop_apps() -> Option<Vec<DesktopApp>> {
    cached_desktop_apps_from_bytes(
        &fs::read(desktop_apps_cache_file()?).ok()?,
        &desktop_environment(),
    )
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
    let mut seen_ids = HashSet::new();
    let mut icon_resolver = DesktopIconResolver::new(desktop_icon_dirs());
    let mut apps = Vec::new();

    for dir in dirs {
        for path in desktop_files_in_dir(dir) {
            let id = desktop_app_id(dir, &path);

            if !seen_ids.insert(id.clone()) {
                continue;
            }

            match parse_desktop_file_with_id(&path, id) {
                Ok(Some(mut app)) => {
                    app.icon_path = app
                        .icon
                        .as_deref()
                        .and_then(|icon| icon_resolver.resolve(icon));
                    apps.push(app);
                }
                Ok(None) => {}
                Err(error) => {
                    eprintln!("failed to read desktop entry {}: {error}", path.display());
                }
            }
        }
    }

    apps.sort_by(app_order);
    apps
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
) -> std::io::Result<()> {
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
        // Schema 3 keeps the parsed app beside its source stamp. A duplicate
        // flat app list would nearly double large desktop-catalog caches.
        apps: Vec::new(),
    };
    let source_cache = DesktopSourceCache {
        version: DESKTOP_CACHE_VERSION,
        environment: environment.to_owned(),
        sources,
    };
    let contents = serde_json::to_vec(&cache).map_err(std::io::Error::other)?;
    crate::atomic_write::write_bytes(&path, &contents)?;
    if let Some(source_path) = desktop_sources_cache_file() {
        let sources = serde_json::to_vec(&source_cache).map_err(std::io::Error::other)?;
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
        DESKTOP_CACHE_VERSION => {
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
) -> (Vec<DesktopApp>, Vec<DesktopCatalogEntry>) {
    let previous = previous
        .into_iter()
        .flat_map(|cache| cache.entries)
        .map(|entry| (entry.source.path.clone(), entry))
        .collect::<HashMap<_, _>>();
    let mut seen_ids = HashSet::new();
    let mut icon_resolver = DesktopIconResolver::new(desktop_icon_dirs());
    let mut entries = Vec::new();
    let mut apps = Vec::new();

    for dir in dirs {
        for path in desktop_files_in_dir(dir) {
            let id = desktop_app_id(dir, &path);
            if !seen_ids.insert(id.clone()) {
                continue;
            }
            let Some(source) = desktop_source_stamp(path.clone()) else {
                continue;
            };
            let app = previous
                .get(&path)
                .filter(|entry| entry.source == source && entry.id == id)
                .map(|entry| entry.app.clone())
                .unwrap_or_else(|| match parse_desktop_file_with_id(&path, id.clone()) {
                    Ok(Some(mut app)) => {
                        app.icon_path = app
                            .icon
                            .as_deref()
                            .and_then(|icon| icon_resolver.resolve(icon));
                        Some(app)
                    }
                    Ok(None) => None,
                    Err(error) => {
                        eprintln!("failed to read desktop entry {}: {error}", path.display());
                        None
                    }
                });
            if let Some(app) = app.as_ref() {
                apps.push(app.clone());
            }
            entries.push(DesktopCatalogEntry { source, id, app });
        }
    }
    apps.sort_by(app_order);
    entries.sort_by(|a, b| a.source.path.cmp(&b.source.path));
    (apps, entries)
}

fn desktop_source_stamps(dirs: &[PathBuf]) -> Vec<DesktopSourceStamp> {
    let mut stamps = dirs
        .iter()
        .flat_map(|directory| desktop_files_in_dir(directory))
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

fn desktop_files_in_dir(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_desktop_files(dir, &mut files);
    files.sort();
    files
}

fn collect_desktop_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            collect_desktop_files(&path, files);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "desktop")
        {
            files.push(path);
        }
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
}
