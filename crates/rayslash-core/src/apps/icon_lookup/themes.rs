use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub(in crate::apps) fn desktop_icon_dirs() -> Vec<PathBuf> {
    let base_dirs = desktop_icon_base_dirs();
    let mut dirs = Vec::new();
    let mut seen_paths = HashSet::new();

    for theme_name in preferred_icon_theme_names(&base_dirs) {
        add_icon_theme_dir(
            &theme_name,
            &base_dirs,
            &mut dirs,
            &mut seen_paths,
            &mut HashSet::new(),
        );
    }

    for base_dir in base_dirs {
        push_unique_path(&mut dirs, &mut seen_paths, base_dir);
    }

    push_unique_path(
        &mut dirs,
        &mut seen_paths,
        PathBuf::from("/usr/share/pixmaps"),
    );

    dirs
}

fn desktop_icon_base_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut seen_paths = HashSet::new();

    if let Some(home) = dirs::home_dir() {
        push_unique_path(&mut dirs, &mut seen_paths, home.join(".local/share/icons"));
        push_unique_path(&mut dirs, &mut seen_paths, home.join(".icons"));
    }

    let data_dirs = std::env::var_os("XDG_DATA_DIRS")
        .map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .unwrap_or_else(|| {
            vec![
                PathBuf::from("/usr/local/share"),
                PathBuf::from("/usr/share"),
            ]
        });

    for data_dir in data_dirs {
        push_unique_path(&mut dirs, &mut seen_paths, data_dir.join("icons"));
    }

    if std::env::var_os("FLATPAK_ID").is_some() {
        for path in [
            "/run/host/user-share/icons",
            "/run/host/share/icons",
            "/run/host/usr/local/share/icons",
            "/run/host/usr/share/icons",
        ] {
            push_unique_path(&mut dirs, &mut seen_paths, PathBuf::from(path));
        }
    }

    dirs
}

fn preferred_icon_theme_names(base_dirs: &[PathBuf]) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen_names = HashSet::new();

    for name in configured_icon_theme_names() {
        push_unique_name(&mut names, &mut seen_names, name);
    }

    for name in [
        "Papirus",
        "Papirus-Dark",
        "Papirus-Light",
        "breeze-dark",
        "breeze",
        "Yaru",
        "Adwaita",
        "hicolor",
    ] {
        if icon_theme_exists(name, base_dirs) {
            push_unique_name(&mut names, &mut seen_names, name.to_owned());
        }
    }

    let mut discovered = base_dirs
        .iter()
        .filter_map(|base_dir| fs::read_dir(base_dir).ok())
        .flat_map(|entries| entries.flatten())
        .filter_map(|entry| {
            let path = entry.path();
            path.join("index.theme")
                .is_file()
                .then(|| entry.file_name().to_string_lossy().into_owned())
        })
        .collect::<Vec<_>>();
    discovered.sort_by_key(|name| name.to_ascii_lowercase());

    for name in discovered {
        push_unique_name(&mut names, &mut seen_names, name);
    }

    names
}

fn configured_icon_theme_names() -> Vec<String> {
    let mut names = Vec::new();
    let mut seen_names = HashSet::new();

    if let Some(name) = icon_theme_from_gsettings() {
        push_unique_name(&mut names, &mut seen_names, name);
    }

    if let Some(home) = dirs::home_dir() {
        for path in [
            home.join(".config/gtk-4.0/settings.ini"),
            home.join(".config/gtk-3.0/settings.ini"),
        ] {
            if let Some(name) = icon_theme_from_ini(&path, "Settings", "gtk-icon-theme-name") {
                push_unique_name(&mut names, &mut seen_names, name);
            }
        }

        if let Some(name) = icon_theme_from_ini(&home.join(".config/kdeglobals"), "Icons", "Theme")
        {
            push_unique_name(&mut names, &mut seen_names, name);
        }
    }

    names
}

fn icon_theme_from_gsettings() -> Option<String> {
    let output = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "icon-theme"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    non_empty(unquote_config_value(
        String::from_utf8_lossy(&output.stdout).trim(),
    ))
}

fn icon_theme_from_ini(path: &Path, section: &str, key: &str) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    let value = ini_value(&contents, section, key)?;
    non_empty(unquote_config_value(value.trim()))
}

fn ini_value<'a>(contents: &'a str, section: &str, key: &str) -> Option<&'a str> {
    let mut in_section = false;

    for line in contents.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            in_section = &line[1..line.len() - 1] == section;
            continue;
        }

        if !in_section {
            continue;
        }

        let Some((candidate_key, value)) = line.split_once('=') else {
            continue;
        };

        if candidate_key.trim() == key {
            return Some(value);
        }
    }

    None
}

fn unquote_config_value(value: &str) -> String {
    value
        .trim()
        .trim_matches('\'')
        .trim_matches('"')
        .trim()
        .to_owned()
}

fn add_icon_theme_dir(
    theme_name: &str,
    base_dirs: &[PathBuf],
    dirs: &mut Vec<PathBuf>,
    seen_paths: &mut HashSet<PathBuf>,
    seen_names: &mut HashSet<String>,
) {
    if !seen_names.insert(theme_name.to_ascii_lowercase()) {
        return;
    }

    for theme_dir in icon_theme_paths(theme_name, base_dirs) {
        push_unique_path(dirs, seen_paths, theme_dir.clone());

        for inherited_theme in icon_theme_inherits(&theme_dir) {
            add_icon_theme_dir(&inherited_theme, base_dirs, dirs, seen_paths, seen_names);
        }
    }
}

fn icon_theme_paths(theme_name: &str, base_dirs: &[PathBuf]) -> Vec<PathBuf> {
    base_dirs
        .iter()
        .map(|base_dir| base_dir.join(theme_name))
        .filter(|path| path.join("index.theme").is_file())
        .collect()
}

fn icon_theme_exists(theme_name: &str, base_dirs: &[PathBuf]) -> bool {
    icon_theme_paths(theme_name, base_dirs)
        .into_iter()
        .next()
        .is_some()
}

fn icon_theme_inherits(theme_dir: &Path) -> Vec<String> {
    let Ok(contents) = fs::read_to_string(theme_dir.join("index.theme")) else {
        return Vec::new();
    };

    ini_value(&contents, "Icon Theme", "Inherits")
        .map(|value| {
            value
                .split(',')
                .filter_map(|name| non_empty(name.trim().to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

fn push_unique_path(paths: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: PathBuf) {
    if seen.insert(path.clone()) {
        paths.push(path);
    }
}

fn push_unique_name(names: &mut Vec<String>, seen: &mut HashSet<String>, name: String) {
    if name.is_empty() {
        return;
    }

    if seen.insert(name.to_ascii_lowercase()) {
        names.push(name);
    }
}

fn non_empty(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}
