use std::{
    collections::HashSet,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

use super::{
    DesktopApp,
    desktop_entry::parse_desktop_file_with_id,
    icon_lookup::{DesktopIconResolver, desktop_icon_dirs},
};

pub fn discover_desktop_apps() -> Vec<DesktopApp> {
    discover_desktop_apps_in_dirs(&desktop_application_dirs())
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

fn desktop_application_dirs() -> Vec<PathBuf> {
    desktop_application_dirs_from_env(
        std::env::var_os("XDG_DATA_HOME"),
        std::env::var_os("XDG_DATA_DIRS"),
        dirs::home_dir(),
    )
}

fn desktop_application_dirs_from_env(
    data_home: Option<OsString>,
    data_dirs: Option<OsString>,
    home: Option<PathBuf>,
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

    #[test]
    fn desktop_application_dirs_follow_xdg_base_directories() {
        assert_eq!(
            desktop_application_dirs_from_env(
                Some(OsString::from("/tmp/data-home")),
                Some(OsString::from("/tmp/flatpak:/tmp/system")),
                Some(PathBuf::from("/home/example")),
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
            desktop_application_dirs_from_env(None, None, Some(PathBuf::from("/home/example"))),
            vec![
                PathBuf::from("/home/example/.local/share/applications"),
                PathBuf::from("/usr/local/share/applications"),
                PathBuf::from("/usr/share/applications"),
            ]
        );
    }
}
