use std::{
    collections::HashSet,
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
    let mut dirs = Vec::new();

    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/share/applications"));
    }

    dirs.push(PathBuf::from("/usr/local/share/applications"));
    dirs.push(PathBuf::from("/usr/share/applications"));
    dirs
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
    fn discover_desktop_apps_in_dirs_reads_desktop_files_without_a_desktop_session() {
        let dir = unique_temp_dir("rayslash-applications");
        let icon = dir.join("app.svg");
        fs::write(
            dir.join("zeta.desktop"),
            format!(
                "[Desktop Entry]\nType=Application\nName=Zeta\nExec=zeta %U\nIcon={}\n",
                icon.display()
            ),
        )
        .expect("write zeta desktop file");
        fs::write(&icon, "<svg xmlns=\"http://www.w3.org/2000/svg\"/>").expect("write icon");
        fs::write(
            dir.join("alpha.desktop"),
            "[Desktop Entry]\nType=Application\nName=Alpha\nExec=alpha\n",
        )
        .expect("write alpha desktop file");
        fs::write(
            dir.join("hidden.desktop"),
            "[Desktop Entry]\nType=Application\nName=Hidden\nExec=hidden\nNoDisplay=true\n",
        )
        .expect("write hidden desktop file");

        let apps = discover_desktop_apps_in_dirs(std::slice::from_ref(&dir));

        assert_eq!(
            apps.iter().map(|app| app.name.as_str()).collect::<Vec<_>>(),
            vec!["Alpha", "Zeta"]
        );
        assert_eq!(apps[1].command.program, std::ffi::OsString::from("zeta"));
        assert!(apps[1].command.args.is_empty());
        assert_eq!(apps[1].icon_path.as_deref(), Some(icon.as_path()));

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
